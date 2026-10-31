#[cfg(target_os = "linux")]
#[test]
fn real_devices_command_emits_a_bounded_versioned_snapshot() {
    use std::{
        process::Command,
        time::{Duration, Instant},
    };
    let started = Instant::now();
    let output = Command::new(env!("CARGO_BIN_EXE_opencontinuity"))
        .args(["devices", "list", "--json"])
        .output()
        .expect("run real CLI command");
    assert!(started.elapsed() < Duration::from_secs(5));
    let value: serde_json::Value = serde_json::from_slice(&output.stdout).expect("JSON output");
    assert_eq!(value["schema_version"], 1);
    assert_eq!(value["source"], "bluez_known_device_snapshot");
    assert_eq!(value["scan_performed"], false);
    assert_eq!(value["authorization"], "never_evaluated");
    assert!(value["adapters"].is_array());
    assert!(value["devices"].is_array());
    let statuses = [
        "ok",
        "no_adapter",
        "bluez_unavailable",
        "system_bus_unavailable",
        "access_denied",
        "timeout",
        "malformed_data",
    ];
    assert!(statuses.contains(&value["status"].as_str().unwrap_or("")));
    assert_eq!(output.status.success(), value["status"] == "ok");
    let serialized = String::from_utf8_lossy(&output.stdout);
    assert!(!serialized.contains("/org/bluez/"));
}
