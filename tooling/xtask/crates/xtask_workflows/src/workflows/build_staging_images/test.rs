use std::process::Command;

#[test]
fn batch_and_individual_selections_share_the_publication_lock() {
    let workflow = serde_json::to_value(super::build_staging_images()).unwrap();
    assert_eq!(
        workflow["concurrency"]["group"],
        "staging-image-${{ github.repository }}"
    );
    assert_eq!(workflow["concurrency"]["cancel-in-progress"], false);
}

#[test]
fn rejects_untrusted_selections_before_any_build() {
    let step = serde_json::to_value(super::resolve_component()).unwrap();
    let script = step["run"].as_str().unwrap();
    let dir = tempfile::tempdir().unwrap();
    for selection in [
        "",
        "../../anything",
        "authentication-service; touch escaped",
        "authentication-service unknown",
        "$(touch escaped)",
        "task-services\nunfurl-service",
    ] {
        let output = Command::new("bash")
            .args(["-c", script])
            .current_dir(dir.path())
            .env_remove("BASH_ENV")
            .env("COMPONENT", selection)
            .output()
            .unwrap();
        assert!(!output.status.success(), "accepted {selection:?}");
    }
    assert!(!dir.path().join("escaped").exists());
}

#[test]
fn selected_binaries_exist_in_the_service_contract() {
    let config: serde_json::Value = serde_json::from_str(
        &std::fs::read_to_string(xtask_paths::repo_root().join(".github/services-config.json"))
            .unwrap(),
    )
    .unwrap();
    assert_eq!(
        super::SERVICES
            .iter()
            .filter(|(_, _, selected)| *selected)
            .count(),
        8
    );
    for &(component, binary, _) in super::SERVICES {
        assert!(
            config["services"][component]["deploy_binaries"]
                .as_array()
                .unwrap()
                .iter()
                .any(|value| value == binary),
            "missing binary {binary} for {component}"
        );
    }
}
