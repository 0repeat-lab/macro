//! Build synthetic-pilot clients without deployment credentials.

use std::collections::HashMap;

use gh_workflow::{
    Concurrency, Event, Expression, Job, Level, Permissions, Step, Workflow, WorkflowDispatch,
    WorkflowDispatchInput,
};

use crate::workflows::{runners, steps};

/// Build clients independently of the long-running Rust backend images.
pub fn build_staging_clients() -> Workflow {
    let mut inputs = HashMap::new();
    inputs.insert(
        "component".into(),
        WorkflowDispatchInput {
            description: "web, lexical-service, sync-service, or all (private synthetic pilot)"
                .into(),
            required: true,
            input_type: "string".into(),
            default: Some("all".into()),
        },
    );
    let validation = Job::default()
        .runs_on(runners::Runner::GitHubLinux.to_string())
        .timeout_minutes(20u32)
        .add_step(
            Step::new("Validate client selection")
                .run("set -euo pipefail\ncase \"$COMPONENT\" in web|lexical-service|sync-service|all) ;; *) exit 1 ;; esac\n")
                                .shell("bash")
                .add_env(("COMPONENT", "${{ inputs.component }}")),
        )
        .add_step(steps::checkout_v4())
        .add_step(steps::setup_bun_pinned())
        .add_step(
            Step::new("Validate workflows on Linux")
                                .run(indoc::indoc! {r#"
                    set -euo pipefail
                    rustup toolchain install 1.94.0 --profile minimal
                    cargo +1.94.0 run --locked -p xtask_workflows -- --check
                    cargo +1.94.0 test --locked -p xtask_workflows
                "#}).shell("bash"),
        );
    let mut workflow = Workflow::new("Build staging clients")
        .permissions(Permissions {
            contents: Some(Level::Read),
            packages: Some(Level::Write),
            ..Default::default()
        })
        .on(Event::default().workflow_dispatch(WorkflowDispatch { inputs }))
        .concurrency(
            Concurrency::new(Expression::new("staging-clients-${{ github.repository }}"))
                .cancel_in_progress(false),
        )
        .add_job("validate", validation);

    for (component, dockerfile) in [
        ("web", "docker/Dockerfile.staging-web"),
        ("lexical-service", "docker/lexical-service.Dockerfile"),
        ("sync-service", "docker/sync-service.Dockerfile"),
    ] {
        let mut job = Job::default()
            .name(format!("Build {component}"))
            .runs_on(runners::Runner::GitHubLinux.to_string())
            .timeout_minutes(180u32)
            .needs(vec!["validate".into()])
            .cond(Expression::new(format!(
                "${{{{ inputs.component == 'all' || inputs.component == '{component}' }}}}"
            )))
            .add_step(steps::checkout_v4());
        if component == "web" {
            job = job
                .add_step(steps::setup_nix())
                .add_step(steps::setup_reqs_web("Prepare web build tools", false))
                .add_step(
                    Step::new("Build same-origin private pilot web app")
                        .run(indoc::indoc! {r#"
                            set -euo pipefail
                            # dev:prepare builds the required WASM packages sequentially.
                            bun run dev:prepare
                            bun run --bun build
                            test -s dist/index.html
                        "#})
                        .shell("bash")
                        .working_directory(xtask_paths::repo_dir!("apps/web"))
                        .add_env(("CARGO_BUILD_JOBS", "2"))
                        .add_env(("NODE_OPTIONS", "--max-old-space-size=4096"))
                        .add_env(("MODE", "development"))
                        .add_env(("NODE_ENV", "production"))
                        .add_env(("VITE_LOCAL_SERVERS", "ALL"))
                        .add_env(("VITE_LOCAL_BACKEND_ORIGIN", "same-origin"))
                        .add_env(("VITE_LOCAL_ONLY", "true"))
                        .add_env(("VITE_LOCAL_STACK", "true"))
                        .add_env(("VITE_AI_EDITING_WORKER_URL", "/ai-editing"))
                        .add_env(("VITE_SEGMENT_WRITE_KEY", ""))
                        .add_env(("VITE_POSTHOG_API_KEY", ""))
                        .add_env(("VITE_OTEL_EXPORTER_URL", "")),
                );
        }
        job = job.add_step(
            Step::new("Build client image")
                                .run(indoc::indoc! {r#"
                    set -euo pipefail
                    owner="${GITHUB_REPOSITORY_OWNER,,}"
                    image="ghcr.io/${owner}/macro-${COMPONENT}:${GITHUB_SHA}"
                    docker build -f "$DOCKERFILE" \
                      --label "org.opencontainers.image.source=https://github.com/${GITHUB_REPOSITORY}" \
                      --label "org.opencontainers.image.revision=${GITHUB_SHA}" -t "$image" .
                "#})
                .shell("bash")
                .add_env(("COMPONENT", component))
                .add_env(("DOCKERFILE", dockerfile)),
        );
        job = job.add_step(
            Step::new("Publish client image")
                                .run(indoc::indoc! {r#"
                    set -euo pipefail
                    owner="${GITHUB_REPOSITORY_OWNER,,}"
                    image="ghcr.io/${owner}/macro-${COMPONENT}:${GITHUB_SHA}"
                    trap 'docker logout ghcr.io >/dev/null 2>&1 || true' EXIT
                    printf '%s' "$REGISTRY_TOKEN" | docker login ghcr.io -u "$GITHUB_ACTOR" --password-stdin
                    docker push "$image"
                    docker inspect --format '{{join .RepoDigests "\n"}}' "$image" >> "$GITHUB_STEP_SUMMARY"
                "#})
                .shell("bash")
                .add_env(("COMPONENT", component))
                .add_env(("REGISTRY_TOKEN", "${{ github.token }}")),
        );
        workflow = workflow.add_job(component, job);
    }
    workflow
}
