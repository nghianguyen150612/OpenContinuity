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
