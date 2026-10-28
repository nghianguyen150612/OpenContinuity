//! Read-only BlueZ capability inspection. This module never registers GATT
//! objects, advertises, powers an adapter, or changes BlueZ state.

use serde::Serialize;
use std::process::{Command, Stdio};

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Evidence {
    Verified,
    Unknown,
    Blocked,
}

#[derive(Debug, Clone, Serialize)]
pub struct AdapterCapability {
    pub object_path: String,
    pub powered: Option<bool>,
    pub gatt_manager: Evidence,
    pub le_advertising_manager: Evidence,
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
            experimental_gatt_transport: "not_implemented",
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
    let Some(tree) = tree.filter(|output| output.status.success()) else {
        return BluezCapabilityReport {
            schema_version: 1,
            source: "bluez_adapter_capability_discovery",
            discovery: "bluez_service_or_system_bus_unavailable",
            mock_simulation: false,
            bluez_capability_discovery: "attempted_but_unavailable",
            experimental_gatt_transport: "not_implemented",
            controller_le_peripheral_capability: "unknown_no_adapter_inspected",
            bluez_version,
            adapters: Vec::new(),
            hardware_watch_observations: "not_run",
            authentication_test_outcome: "not_run",
            authorization: "never_evaluated",
        };
    };

    let paths = String::from_utf8_lossy(&tree.stdout);
    let adapters: Vec<_> = paths
        .lines()
        .filter_map(|line| {
            let path = line.split_whitespace().find_map(|part| {
                let start = part.find("/org/bluez/hci")?;
                Some(&part[start..])
            })?;
            let suffix = path.strip_prefix("/org/bluez/")?;
            (!suffix.contains('/')).then_some(path)
        })
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
            let text = introspection
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
                gatt_manager: interface_evidence(text.as_deref(), "org.bluez.GattManager1"),
                le_advertising_manager: interface_evidence(
                    text.as_deref(),
                    "org.bluez.LEAdvertisingManager1",
                ),
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
        experimental_gatt_transport: "not_implemented",
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

fn interface_evidence(introspection: Option<&str>, interface: &str) -> Evidence {
    match introspection {
        Some(text) if text.contains(interface) => Evidence::Verified,
        Some(_) => Evidence::Blocked,
        None => Evidence::Unknown,
    }
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
            interface_evidence(Some("org.bluez.GattManager1"), "org.bluez.GattManager1"),
            Evidence::Verified
        ));
        assert!(matches!(
            interface_evidence(Some("org.bluez.Adapter1"), "org.bluez.GattManager1"),
            Evidence::Blocked
        ));
        assert!(matches!(
            interface_evidence(None, "org.bluez.GattManager1"),
            Evidence::Unknown
        ));
        assert_eq!(parse_bool("b true"), Some(true));
        assert_eq!(parse_bool("b false"), Some(false));
        assert_eq!(parse_bool("invalid"), None);
    }

    #[test]
    fn report_is_non_authorizing_and_does_not_claim_watch_observations() {
        let report = inspect();
        let value = serde_json::to_value(report).unwrap();
        assert_eq!(value["authorization"], "never_evaluated");
        assert_eq!(value["mock_simulation"], false);
        assert_eq!(value["experimental_gatt_transport"], "not_implemented");
        assert_eq!(
            value["controller_le_peripheral_capability"],
            "unknown_no_adapter_inspected"
        );
        assert_eq!(value["hardware_watch_observations"], "not_run");
        assert_eq!(value["authentication_test_outcome"], "not_run");
    }
}
