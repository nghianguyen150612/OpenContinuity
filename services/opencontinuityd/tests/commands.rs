use std::process::Command;

#[test]
fn help_and_version_work() {
    let help = Command::new(env!("CARGO_BIN_EXE_opencontinuityd"))
        .arg("--help")
        .output()
        .unwrap();
    assert!(help.status.success());
    assert!(String::from_utf8_lossy(&help.stdout).contains("foreground"));

    let version = Command::new(env!("CARGO_BIN_EXE_opencontinuityd"))
        .arg("--version")
        .output()
        .unwrap();
    assert!(version.status.success());
    assert!(String::from_utf8_lossy(&version.stdout).contains(env!("CARGO_PKG_VERSION")));
}
