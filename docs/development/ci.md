# CI on pull requests

## Standalone staging service builds

`Build staging service image` is a manual-only workflow for a fork without
the private Namespace runners or cloud deployment credentials. It runs on a
standard GitHub-hosted Linux runner, builds one allowlisted service through
its existing Nix deploy package, and publishes the binary plus its runtime
closure to `ghcr.io/<repository-owner>/macro-<service>:<commit-sha>`.

Choose the service key in the `component` input, starting with
`authentication-service`. It verifies workflow generation and runs the workflow
package's tests on Linux before building. This workflow only builds an image: it does not
run migrations, start a server, or deploy infrastructure. Deployments should
use the resulting registry digest and independent environment configuration.
New GHCR packages may require package visibility or pull-access configuration
before a remote host can retrieve them.

Use `task-services` after verifying authentication to build the remaining eight
core Rust task services sequentially in one runner. They reuse the dependency
build rather than starting a cold runner for every image. Email and calendar
remain explicit individual selections; this batch does not deploy services or
build the JavaScript frontend/workers. Search uses the existing local target
without PDF's platform-specific runtime library.
The staging workflow serializes runs across selections, so a batch cannot race
an individual build that publishes the same service tag. Wait for the active
run before dispatching further selections.

The runner has no private build cache, so a cold build can be expensive in
time and disk space. It is bounded to one Nix build job with two cores and
has a three-hour timeout. Verify the first build before queuing other services.

The workflow is generated from
`tooling/xtask/crates/xtask_workflows/src/workflows/build_staging_images.rs`.
Regenerate it with `cargo x workflows`; do not edit its YAML directly.

## Pull request checks

CI runs on a pull request only when someone adds the `run-ci` label. The `PR CI`
workflow then runs every check on the PR's head commit, removes the label, and
reports the result as the `CI gate` commit status on that commit. Each push
leaves the new commit at `Not run on this commit`, so a PR can merge only after
a passing run on its latest commit.

Run CI deliberately: at a checkpoint, a coherent locally tested chunk you want
real CI on before building further, or when you think the PR is done. Never
after every push, and never instead of local checks.

If the label starts nothing and `CI gate` says `PR CI can't start`, the PR
conflicts with `main` or was last pushed before `PR CI` existed: merge `main`
in, push, and add the label again.

The [`run-ci` skill](../../.agents/skills/run-ci/SKILL.md) has the commands,
what each `CI gate` state means, re-runs, and fork PRs. The workflows are
generated from Rust; see the
[workflows rule](../../.agents/rules/infra/github-workflows.md).

## RustSec advisories

`cargo deny` reads the live RustSec advisory database. On a PR it fails only
on advisories against packages the PR adds to `Cargo.lock`: a name and
version the base's lockfile doesn't have. An advisory published against a
package already on `main` doesn't fail PRs. The daily `cargo deny advisories`
workflow fails on it instead, and a failed run posts to the Macro Alerts
channel. The CI owner fixes it on `main`: update the crate, or ignore the
advisory in `deny.toml` with a reason.

Locally, `cargo deny check advisories` reports every advisory, as the daily
run does.
