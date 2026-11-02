use std::process::Command;
#[test]
fn discovery_requires_an_explicit_subcommand_and_valid_arguments_before_bus_access() {
    for args in [
        vec!["devices", "discover"],
        vec!["devices", "discover", "--seconds", "0", "--json"],
        vec!["devices", "discover", "--seconds", "31", "--json"],
        vec![
            "devices",
            "discover",
            "--adapter-index",
            "/org/bluez/hci0",
            "--json",
        ],
        vec!["devices", "--json"],
        vec!["devices", "discover", "--events-jsonl", "--json"],
        vec!["devices", "discover", "--events-jsonl", "--events-jsonl"],
    ] {
        let result = Command::new(env!("CARGO_BIN_EXE_opencontinuity"))
            .args(args)
            .output()
            .unwrap();
        assert_eq!(result.status.code(), Some(2));
        assert!(result.stdout.is_empty());
    }
}

#[cfg(target_os = "linux")]
#[test]
fn jsonl_preflight_failure_has_one_session_and_terminal_event() {
    let output = Command::new(env!("CARGO_BIN_EXE_opencontinuity"))
        .args(["devices", "discover", "--seconds", "1", "--events-jsonl"])
        .env(
            "DBUS_SYSTEM_BUS_ADDRESS",
            "unix:path=/nonexistent/p014-private-bus",
        )
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(1));
    assert!(output.stderr.is_empty());
    let text = String::from_utf8(output.stdout).unwrap();
    let events: Vec<serde_json::Value> = text
        .lines()
        .map(|line| serde_json::from_str(line).unwrap())
        .collect();
    assert_eq!(events.len(), 2);
    assert_eq!(events[0]["type"], "session");
    assert_eq!(events[1]["type"], "terminal");
    assert_eq!(events[0]["sequence"], 1);
    assert_eq!(events[1]["sequence"], 2);
    assert_eq!(events[0]["scan_session_id"], events[1]["scan_session_id"]);
    assert_eq!(events[0]["source"], "bluez_le_discovery_events");
    assert_eq!(events[0]["authorization"], "never_evaluated");
    assert_eq!(events[0]["status"], "system_bus_unavailable");
    assert_eq!(events[1]["status"], "system_bus_unavailable");
    assert_eq!(events[1]["cleanup_status"], "not_needed");
    assert!(!text.contains("p014-private-bus"));
}
#[cfg(target_os = "linux")]
#[test]
fn unavailable_bus_is_sanitized_and_nonzero() {
    let output = Command::new(env!("CARGO_BIN_EXE_opencontinuity"))
        .args(["devices", "discover", "--seconds", "1", "--json"])
        .env(
            "DBUS_SYSTEM_BUS_ADDRESS",
            "unix:path=/nonexistent/p013-private-bus",
        )
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(1));
    assert!(output.stderr.is_empty());
    let value: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(value["status"], "system_bus_unavailable");
    assert_eq!(value["scan_started"], false);
    assert_eq!(value["cleanup_status"], "not_needed");
    assert!(
        !String::from_utf8(output.stdout)
            .unwrap()
            .contains("p013-private-bus")
    );
}
