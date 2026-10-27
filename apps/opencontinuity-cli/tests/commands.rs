use std::process::Command;

#[test]
fn help_and_version_work() {
    let help = Command::new(env!("CARGO_BIN_EXE_opencontinuity"))
        .arg("--help")
        .output()
        .unwrap();
    assert!(help.status.success());
    assert!(String::from_utf8_lossy(&help.stdout).contains("doctor"));

    let version = Command::new(env!("CARGO_BIN_EXE_opencontinuity"))
        .arg("--version")
        .output()
        .unwrap();
    assert!(version.status.success());
    assert!(String::from_utf8_lossy(&version.stdout).contains(env!("CARGO_PKG_VERSION")));
}

#[test]
fn doctor_json_command_emits_valid_unknown_and_unsupported_states() {
    let result = Command::new(env!("CARGO_BIN_EXE_opencontinuity"))
        .args(["doctor", "--json"])
        .output()
        .unwrap();
    assert!(result.status.success());
    let report: serde_json::Value = serde_json::from_slice(&result.stdout).unwrap();
    assert_eq!(report["project"], "OpenContinuity");
    assert_eq!(
        report["watch_auto_unlock_trust"]["device_detection"],
        "unknown"
    );
    assert_eq!(
        report["watch_auto_unlock_trust"]["operational_readiness"],
        "unknown"
    );
    assert_eq!(report["capabilities"][0]["state"], "unsupported");
}

#[test]
fn research_watch_fixture_is_read_only_and_deterministic() {
    let path = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../tests/fixtures/watch-discovery.json"
    );
    let run = || {
        Command::new(env!("CARGO_BIN_EXE_opencontinuity"))
            .args(["research", "watch", "--fixture", path, "--json"])
            .output()
            .unwrap()
    };
    let first = run();
    let second = run();
    assert!(first.status.success());
    assert!(second.status.success());
    assert_eq!(first.stdout, second.stdout);
    let report: serde_json::Value = serde_json::from_slice(&first.stdout).unwrap();
    assert_eq!(report["source"], "mock_fixture");
    assert_eq!(report["authorization"], "never_evaluated");
    assert_eq!(report["observations"].as_array().unwrap().len(), 1);
}

#[test]
fn research_watch_fixture_rejects_invalid_input() {
    let path = std::env::temp_dir().join(format!(
        "opencontinuity-invalid-fixture-{}.json",
        std::process::id()
    ));
    std::fs::write(&path, "{}").unwrap();
    let result = Command::new(env!("CARGO_BIN_EXE_opencontinuity"))
        .args(["research", "watch", "--fixture"])
        .arg(&path)
        .output()
        .unwrap();
    let _ = std::fs::remove_file(path);
    assert!(!result.status.success());
    assert!(String::from_utf8_lossy(&result.stderr).contains("invalid research fixture"));
}
