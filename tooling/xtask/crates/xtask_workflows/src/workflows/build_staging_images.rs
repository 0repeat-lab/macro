//! Build staging images without private runners or deployment access.

use std::collections::HashMap;

use gh_workflow::{
    Concurrency, Event, Expression, Job, Level, Permissions, Run, Step, Workflow, WorkflowDispatch,
    WorkflowDispatchInput,
};

use crate::workflows::{runners, steps};

#[cfg(test)]
mod test;

// Authentication stays independently deployable. The batch reuses one runner's
// cold dependency build for the remaining core task backends.
const SERVICES: &[(&str, &str, bool)] = &[
    ("authentication-service", "authentication_service", false),
    ("connection-gateway", "connection_gateway_service", true),
    ("contacts-service", "contacts_service", true),
    ("document-storage-service", "document_storage_service", true),
    ("notification-service", "notification_service", true),
    (
        "search-processing-service",
        "search_processing_service",
        true,
    ),
    ("email-service", "email_service", false),
    ("calendar-service", "calendar_service", false),
    ("static-file-service", "static_file_service", true),
    ("image-proxy-service", "image_proxy_service", true),
    ("unfurl-service", "unfurl_service", true),
];

/// Build selected Linux images without changing a running environment.
pub fn build_staging_images() -> Workflow {
    let mut inputs = HashMap::new();
    inputs.insert(
        "component".into(),
        WorkflowDispatchInput {
            description: "A service key, or task-services for the remaining core task backends"
                .into(),
            required: true,
            input_type: "string".into(),
            default: Some("authentication-service".into()),
        },
    );

    let mut job = Job::default()
        .name("Build and publish selected Linux service images")
        .runs_on(runners::Runner::GitHubLinux.to_string())
        .timeout_minutes(180u32)
        .add_step(steps::checkout_v4())
        .add_step(resolve_component())
        .add_step(steps::setup_bun_pinned())
        .add_step(
            Step::new("Validate workflows on Linux")
                .run(indoc::indoc! {r#"
                    set -euo pipefail
                    rustup toolchain install 1.94.0 --profile minimal
                    cargo +1.94.0 run --locked -p xtask_workflows -- --check
                    cargo +1.94.0 test --locked -p xtask_workflows
                "#})
                .shell("bash"),
        )
        .add_step(steps::setup_nix());

    for &(component, binary, task_service) in SERVICES {
        let condition = Expression::new(format!(
            "${{{{ inputs.component == '{component}' || (inputs.component == 'task-services' && {task_service}) }}}}"
        ));
        // The existing local search target omits PDF's platform library.
        let target = if component == "search-processing-service" {
            "local-stack-binaries-local-search-processing".to_owned()
        } else {
            format!("deploy-service-binaries-{component}")
        };
        job = job
            .add_step(build_component(component, &target).if_condition(condition.clone()))
            .add_step(publish_component(component, binary).if_condition(condition));
    }

    Workflow::new("Build staging service image")
        .permissions(Permissions {
            contents: Some(Level::Read),
            packages: Some(Level::Write),
            ..Default::default()
        })
        .on(Event::default().workflow_dispatch(WorkflowDispatch { inputs }))
        .concurrency(
            Concurrency::new(Expression::new("staging-image-${{ github.repository }}"))
                .cancel_in_progress(false),
        )
        .add_job("build", job)
}

fn resolve_component() -> Step<Run> {
    let allowed = SERVICES
        .iter()
        .map(|(name, _, _)| *name)
        .collect::<Vec<_>>()
        .join("|");
    Step::new("Validate service selection")
        .run(format!(
            "set -euo pipefail\ncase \"$COMPONENT\" in\n  {allowed}|task-services) ;;\n  *) echo 'Unsupported staging service' >&2; exit 1 ;;\nesac\n"
        ))
        .id("component")
        .shell("bash")
        .add_env(("COMPONENT", "${{ inputs.component }}"))
}

fn build_component(component: &str, target: &str) -> Step<Run> {
    Step::new(format!("Build {component}"))
        .run(indoc::indoc! {r#"
            set -euo pipefail
            test ! -e prebuilt
            nix build --max-jobs 1 --cores 2 --print-build-logs ".#${NIX_TARGET}"
            mkdir -p prebuilt/nix-store
            cp -r result/bin/* prebuilt/
            nix-store -qR result > runtime-closure.txt
            while IFS= read -r store_path; do
              cp -a "$store_path" prebuilt/nix-store/
            done < runtime-closure.txt
        "#})
        .shell("bash")
        .add_env(("NIX_TARGET", target))
}

fn publish_component(component: &str, binary: &str) -> Step<Run> {
    Step::new(format!("Publish {component}"))
        .run(indoc::indoc! {r#"
            set -euo pipefail
            owner="${GITHUB_REPOSITORY_OWNER,,}"
            image="ghcr.io/${owner}/macro-${COMPONENT}:${GITHUB_SHA}"
            trap 'docker logout ghcr.io >/dev/null 2>&1 || true' EXIT
            printf '%s' "$REGISTRY_TOKEN" | docker login ghcr.io -u "$GITHUB_ACTOR" --password-stdin
            docker build -f docker/Dockerfile.prebuilt --build-arg "SERVICE_NAME=$BINARY" \
              --label "org.opencontainers.image.source=https://github.com/${GITHUB_REPOSITORY}" \
              --label "org.opencontainers.image.revision=${GITHUB_SHA}" -t "$image" .
            docker push "$image"
            docker inspect --format '{{join .RepoDigests "\n"}}' "$image" >> "$GITHUB_STEP_SUMMARY"
            chmod -R u+w prebuilt
            rm -rf -- prebuilt runtime-closure.txt
        "#})
        .shell("bash")
        .add_env(("COMPONENT", component))
        .add_env(("BINARY", binary))
        .add_env(("REGISTRY_TOKEN", "${{ github.token }}"))
}
