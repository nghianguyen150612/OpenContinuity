//! Read-only BlueZ capability inspection. This module never registers GATT
//! objects, advertises, powers an adapter, or changes BlueZ state.

use serde::Serialize;
use std::process::{Command, Stdio};

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Evidence {
    Supported,
    Unsupported,
    Unknown,
    Blocked,
    NotTested,
}

#[derive(Debug, Clone, Serialize)]
pub struct AdapterCapability {
    pub object_path: String,
    pub powered: Option<bool>,
    pub gatt_manager: Evidence,
    pub le_advertising_manager: Evidence,
    pub controller_le_peripheral_capability: Evidence,
}

#[derive(Debug, Clone, Serialize)]
pub struct BluezCapabilityReport {
    pub schema_version: u8,
    pub source: &'static str,
    pub discovery: &'static str,
    pub mock_simulation: bool,
    pub bluez_capability_discovery: &'static str,
    pub experimental_gatt_transport: &'static str,
    pub controller_le_peripheral_capability: &'static str,
    pub bluez_version: Option<String>,
    pub adapters: Vec<AdapterCapability>,
    pub hardware_watch_observations: &'static str,
    pub authentication_test_outcome: &'static str,
    pub authorization: &'static str,
}

pub fn inspect() -> BluezCapabilityReport {
    #[cfg(target_os = "linux")]
    {
        inspect_linux()
    }
    #[cfg(not(target_os = "linux"))]
    {
        BluezCapabilityReport {
            schema_version: 1,
            source: "bluez_adapter_capability_discovery",
            discovery: "blocked_non_linux_target",
            mock_simulation: false,
            bluez_capability_discovery: "blocked_non_linux_target",
            experimental_gatt_transport: "implemented_experimental",
            controller_le_peripheral_capability: "unknown_not_read_by_probe",
            bluez_version: None,
            adapters: Vec::new(),
            hardware_watch_observations: "not_run",
            authentication_test_outcome: "not_run",
            authorization: "never_evaluated",
        }
    }
}

#[cfg(target_os = "linux")]
fn inspect_linux() -> BluezCapabilityReport {
    let bluez_version = run(&["bluetoothd", "--version"])
        .filter(|output| output.status.success())
        .map(|output| String::from_utf8_lossy(&output.stdout).trim().to_owned())
        .filter(|version| !version.is_empty());
    let tree = run(&[
        "busctl",
        "--system",
        "--no-pager",
        "--timeout=2s",
        "tree",
        "org.bluez",
    ]);
    let Some(tree_output) = tree else {
        return BluezCapabilityReport {
            schema_version: 1,
            source: "bluez_adapter_capability_discovery",
            discovery: "system_bus_probe_unavailable",
            mock_simulation: false,
            bluez_capability_discovery: "attempted_but_unavailable",
            experimental_gatt_transport: "implemented_experimental",
            controller_le_peripheral_capability: "unknown_no_adapter_inspected",
            bluez_version,
            adapters: Vec::new(),
            hardware_watch_observations: "not_run",
            authentication_test_outcome: "not_run",
            authorization: "never_evaluated",
        };
    };
    if !tree_output.status.success() {
        let discovery = classify_tree_error(&String::from_utf8_lossy(&tree_output.stderr));
        let denied = discovery == "blocked_dbus_access";
        return BluezCapabilityReport {
            schema_version: 1,
            source: "bluez_adapter_capability_discovery",
            discovery,
            mock_simulation: false,
            bluez_capability_discovery: if denied {
                "blocked"
            } else {
                "attempted_but_unavailable"
            },
            experimental_gatt_transport: "implemented_experimental",
            controller_le_peripheral_capability: "unknown_no_adapter_inspected",
            bluez_version,
            adapters: Vec::new(),
            hardware_watch_observations: "not_run",
            authentication_test_outcome: "not_run",
            authorization: "never_evaluated",
        };
    }
    let tree = tree_output;

    let paths = String::from_utf8_lossy(&tree.stdout);
    let adapters: Vec<_> = adapter_paths(&paths)
        .iter()
        .map(|path| path.as_str())
        .map(|path| {
            let introspection = run(&[
                "busctl",
                "--system",
                "--no-pager",
                "--timeout=2s",
                "introspect",
                "org.bluez",
                path,
            ]);
            let denied = introspection.as_ref().is_some_and(|output| {
                let error = String::from_utf8_lossy(&output.stderr).to_lowercase();
                !output.status.success()
                    && (error.contains("accessdenied")
                        || error.contains("access denied")
                        || error.contains("permission denied"))
            });
            let text = introspection
                .as_ref()
                .filter(|output| output.status.success())
                .map(|output| String::from_utf8_lossy(&output.stdout).into_owned());
            let powered_output = run(&[
                "busctl",
                "--system",
                "--no-pager",
                "--timeout=2s",
                "get-property",
                "org.bluez",
                path,
                "org.bluez.Adapter1",
                "Powered",
            ]);
            let powered = powered_output
                .filter(|output| output.status.success())
                .and_then(|output| parse_bool(&String::from_utf8_lossy(&output.stdout)));
            AdapterCapability {
                object_path: path.to_owned(),
                powered,
                gatt_manager: interface_evidence(text.as_deref(), "org.bluez.GattManager1", denied),
                le_advertising_manager: interface_evidence(
                    text.as_deref(),
                    "org.bluez.LEAdvertisingManager1",
                    denied,
                ),
                controller_le_peripheral_capability: Evidence::NotTested,
            }
        })
        .collect();
    BluezCapabilityReport {
        schema_version: 1,
        source: "bluez_adapter_capability_discovery",
        discovery: if adapters.is_empty() {
            "bluez_service_found_no_adapter_objects"
        } else {
            "bluez_dbus_objects_inspected"
        },
        mock_simulation: false,
        bluez_capability_discovery: "completed",
        experimental_gatt_transport: "implemented_experimental",
        controller_le_peripheral_capability: "unknown_not_read_by_probe",
        bluez_version,
        adapters,
        hardware_watch_observations: "not_run",
        authentication_test_outcome: "not_run",
        authorization: "never_evaluated",
    }
}

#[cfg(target_os = "linux")]
fn run(args: &[&str]) -> Option<std::process::Output> {
    let (program, rest) = args.split_first()?;
    Command::new(program)
        .args(rest)
        .stdin(Stdio::null())
        .output()
        .ok()
}

fn interface_evidence(introspection: Option<&str>, interface: &str, denied: bool) -> Evidence {
    match introspection {
        Some(text) if text.contains(interface) => Evidence::Supported,
        Some(_) => Evidence::Unsupported,
        None if denied => Evidence::Blocked,
        None => Evidence::Unknown,
    }
}

fn classify_tree_error(stderr: &str) -> &'static str {
    let error = stderr.to_lowercase();
    if error.contains("accessdenied")
        || error.contains("access denied")
        || error.contains("permission denied")
    {
        "blocked_dbus_access"
    } else if error.contains("failed to connect") || error.contains("no such file") {
        "system_bus_unavailable"
    } else if error.contains("serviceunknown")
        || error.contains("not provided by any .service files")
    {
        "bluez_service_unavailable"
    } else {
        "bluez_probe_failed_unknown_reason"
    }
}

fn adapter_paths(tree: &str) -> Vec<String> {
    let mut paths = tree
        .lines()
        .filter_map(|line| {
            let path = line
                .split_whitespace()
                .find(|part| part.starts_with("/org/bluez/"))?;
            let adapter = path.strip_prefix("/org/bluez/")?;
            (adapter.starts_with("hci")
                && adapter[3..].bytes().all(|byte| byte.is_ascii_digit())
                && adapter.len() > 3
                && !adapter.contains('/'))
            .then(|| path.to_owned())
        })
        .collect::<Vec<_>>();
    paths.sort();
    paths.dedup();
    paths
}

fn parse_bool(value: &str) -> Option<bool> {
    match value.split_whitespace().last()? {
        "true" => Some(true),
        "false" => Some(false),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn capability_parser_distinguishes_present_missing_and_unknown() {
        assert!(matches!(
            interface_evidence(
                Some("org.bluez.GattManager1"),
                "org.bluez.GattManager1",
                false
            ),
            Evidence::Supported
        ));
        assert!(matches!(
            interface_evidence(Some("org.bluez.Adapter1"), "org.bluez.GattManager1", false),
            Evidence::Unsupported
        ));
        assert!(matches!(
            interface_evidence(None, "org.bluez.GattManager1", false),
            Evidence::Unknown
        ));
        assert!(matches!(
            interface_evidence(None, "org.bluez.GattManager1", true),
            Evidence::Blocked
        ));
        assert_eq!(
            classify_tree_error("Failed to connect to system scope bus: No such file or directory"),
            "system_bus_unavailable"
        );
        assert_eq!(
            classify_tree_error(
                "org.freedesktop.DBus.Error.ServiceUnknown: org.bluez is not provided by any .service files"
            ),
            "bluez_service_unavailable"
        );
        assert_eq!(
            classify_tree_error("org.freedesktop.DBus.Error.AccessDenied: denied"),
            "blocked_dbus_access"
        );
        assert_eq!(parse_bool("b true"), Some(true));
        assert_eq!(parse_bool("b false"), Some(false));
        assert_eq!(parse_bool("invalid"), None);
    }

    #[test]
    fn classifications_are_environment_independent() {
        for status in [
            Evidence::Supported,
            Evidence::Unsupported,
            Evidence::Unknown,
            Evidence::Blocked,
            Evidence::NotTested,
        ] {
            assert!(serde_json::to_string(&status).is_ok());
        }
        assert!(adapter_paths("").is_empty());
        assert!(adapter_paths("garbled text /not-bluez/hci0 /org/bluez/hci0/dev_aa").is_empty());
        assert_eq!(
            adapter_paths("/org/bluez/hci1\n/org/bluez/hci0\n/org/bluez/hci1"),
            ["/org/bluez/hci0", "/org/bluez/hci1"]
        );
    }
}
