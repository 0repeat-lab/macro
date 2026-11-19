//! Manually build a staging service image without private runners or deployment access.

use std::collections::HashMap;

use gh_workflow::{
    Concurrency, Event, Expression, Job, Level, Permissions, Step, Workflow, WorkflowDispatch,
    WorkflowDispatchInput,
};

use crate::workflows::{runners, steps};

/// Build a Linux image and publish it without changing a running environment.
pub fn build_staging_images() -> Workflow {
    let mut inputs = HashMap::new();
    inputs.insert(
        "component".into(),
        WorkflowDispatchInput {
            description: "Service key from the staging build allowlist".into(),
            required: true,
            input_type: "string".into(),
            default: Some("authentication-service".into()),
        },
    );

    Workflow::new("Build staging service image")
        .permissions(Permissions {
            contents: Some(Level::Read),
            packages: Some(Level::Write),
            ..Default::default()
        })
        .on(Event::default().workflow_dispatch(WorkflowDispatch { inputs }))
        .concurrency(
            Concurrency::new(Expression::new("staging-image-${{ inputs.component }}"))
                .cancel_in_progress(false),
        )
        .add_job(
            "build",
            Job::default()
                .name("Build and publish Linux service image")
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
                .add_step(steps::setup_nix())
                .add_step(
                    Step::new("Build Linux binary and runtime closure")
                        .run(indoc::indoc! {r#"
                            set -euo pipefail
                            nix build --max-jobs 1 --cores 2 --print-build-logs ".#deploy-service-binaries-${COMPONENT}"
                            mkdir -p prebuilt/nix-store
                            cp -r result/bin/* prebuilt/
                            nix-store -qR result > runtime-closure.txt
                            while IFS= read -r store_path; do
                              cp -a "$store_path" prebuilt/nix-store/
                            done < runtime-closure.txt
                        "#})
                        .shell("bash")
                        .add_env(("COMPONENT", "${{ inputs.component }}")),
                )
                .add_step(
                    Step::new("Publish immutable commit image")
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
                        "#})
                        .shell("bash")
                        .add_env(("COMPONENT", "${{ inputs.component }}"))
                        .add_env(("BINARY", "${{ steps.component.outputs.binary }}"))
                        .add_env(("REGISTRY_TOKEN", "${{ github.token }}")),
                ),
        )
}

fn resolve_component() -> Step<gh_workflow::Run> {
    Step::new("Validate service selection")
        .run(indoc::indoc! {r#"
            set -euo pipefail
            case "$COMPONENT" in
              authentication-service) binary=authentication_service ;;
              connection-gateway) binary=connection_gateway_service ;;
              contacts-service) binary=contacts_service ;;
              document-storage-service) binary=document_storage_service ;;
              notification-service) binary=notification_service ;;
              search-processing-service) binary=search_processing_service ;;
              email-service) binary=email_service ;;
              calendar-service) binary=calendar_service ;;
              static-file-service) binary=static_file_service ;;
              image-proxy-service) binary=image_proxy_service ;;
              unfurl-service) binary=unfurl_service ;;
              *) echo 'Unsupported staging service' >&2; exit 1 ;;
            esac
            printf 'binary=%s\n' "$binary" >> "$GITHUB_OUTPUT"
        "#})
        .id("component")
        .shell("bash")
        .add_env(("COMPONENT", "${{ inputs.component }}"))
}
