//! One-shot, read-only inventory of BlueZ's already-known Device1 objects.
//! This module never starts discovery or calls a mutating BlueZ method.

use serde::Serialize;
use std::time::Duration;

pub const MAX_ADAPTERS: usize = 16;
pub const MAX_DEVICES: usize = 128;
pub const MAX_NAME_CHARS: usize = 80;
pub const MAX_OUTPUT_BYTES: usize = 64 * 1024;
pub const SNAPSHOT_TIMEOUT: Duration = Duration::from_secs(3);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum InventoryStatus {
    Ok,
    NoAdapter,
    BluezUnavailable,
    SystemBusUnavailable,
    AccessDenied,
    Timeout,
    MalformedData,
    #[cfg(not(target_os = "linux"))]
    UnsupportedPlatform,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Adapter {
    pub id: String,
    pub powered: Option<bool>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Device {
    pub id: String,
    pub adapter_id: String,
    pub display_name: String,
    pub connected: Option<bool>,
    pub paired: Option<bool>,
    pub trusted: Option<bool>,
    pub services_resolved: Option<bool>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct DeviceInventory {
    pub schema_version: u8,
    pub source: &'static str,
    pub scan_performed: bool,
    pub status: InventoryStatus,
    pub adapters: Vec<Adapter>,
    pub devices: Vec<Device>,
    pub authorization: &'static str,
}

impl DeviceInventory {
    fn new(status: InventoryStatus) -> Self {
        Self {
            schema_version: 1,
            source: "bluez_known_device_snapshot",
            scan_performed: false,
            status,
            adapters: Vec::new(),
            devices: Vec::new(),
            authorization: "never_evaluated",
        }
    }

    #[cfg(not(target_os = "linux"))]
    pub fn unsupported() -> Self {
        Self::new(InventoryStatus::UnsupportedPlatform)
    }

    pub fn failed(status: InventoryStatus) -> Self {
        Self::new(status)
    }
}

/// Normalized input to the production extractor; also makes object data
/// injectable for deterministic hardware-free tests.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Property {
    Bool(bool),
    Text(String),
    Invalid,
}

pub type InterfaceProperties = std::collections::BTreeMap<String, Property>;
pub type ManagedObjects =
    std::collections::BTreeMap<String, std::collections::BTreeMap<String, InterfaceProperties>>;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InventoryError {
    NoAdapter,
    TooManyAdapters,
    TooManyDevices,
    MalformedData,
}

pub fn extract(objects: &ManagedObjects) -> Result<DeviceInventory, InventoryError> {
    let adapters: Vec<_> = objects
        .iter()
        .filter(|(_, interfaces)| interfaces.contains_key("org.bluez.Adapter1"))
        .collect();
    if adapters.len() > MAX_ADAPTERS {
        return Err(InventoryError::TooManyAdapters);
    }
    if adapters.is_empty() {
        return Err(InventoryError::NoAdapter);
    }

    let mut output = DeviceInventory::new(InventoryStatus::Ok);
    for (index, (path, interfaces)) in adapters.iter().enumerate() {
        let properties = &interfaces["org.bluez.Adapter1"];
        let powered = optional_bool(properties, "Powered")?;
        output.adapters.push(Adapter {
            id: format!("adapter-{:03}", index + 1),
            powered,
        });
        let _ = path;
    }
    let mut devices: Vec<_> = objects
        .iter()
        .filter(|(_, interfaces)| interfaces.contains_key("org.bluez.Device1"))
        .collect();
    devices.sort_by(|(left, _), (right, _)| left.cmp(right));
    if devices.len() > MAX_DEVICES {
        return Err(InventoryError::TooManyDevices);
    }
    for (index, (path, interfaces)) in devices.into_iter().enumerate() {
        let Some((adapter_index, _)) =
            adapters.iter().enumerate().find(|(_, (adapter_path, _))| {
                path.strip_prefix(adapter_path.as_str())
                    .is_some_and(|tail| tail.starts_with("/"))
            })
        else {
            return Err(InventoryError::MalformedData);
        };
        let properties = &interfaces["org.bluez.Device1"];
        let alias = optional_text(properties, "Alias")?;
        let name = optional_text(properties, "Name")?;
        let alias_name = alias.map(|value| sanitize_name(Some(value)));
        let display_name = alias_name
            .filter(|value| value != "Unnamed Bluetooth device")
            .or_else(|| name.map(|value| sanitize_name(Some(value))))
            .unwrap_or_else(|| "Unnamed Bluetooth device".to_owned());
        output.devices.push(Device {
            id: format!("device-{:03}", index + 1),
            adapter_id: format!("adapter-{:03}", adapter_index + 1),
            display_name,
            connected: optional_bool(properties, "Connected")?,
            paired: optional_bool(properties, "Paired")?,
            trusted: optional_bool(properties, "Trusted")?,
            services_resolved: optional_bool(properties, "ServicesResolved")?,
        });
    }
    Ok(output)
}

fn optional_bool(
    properties: &InterfaceProperties,
    key: &str,
) -> Result<Option<bool>, InventoryError> {
    match properties.get(key) {
        None => Ok(None),
        Some(Property::Bool(value)) => Ok(Some(*value)),
        Some(_) => Err(InventoryError::MalformedData),
    }
}
fn optional_text(
    properties: &InterfaceProperties,
    key: &str,
) -> Result<Option<String>, InventoryError> {
    match properties.get(key) {
        None => Ok(None),
        Some(Property::Text(value)) => Ok(Some(value.clone())),
        Some(_) => Err(InventoryError::MalformedData),
    }
}

pub fn sanitize_name(name: Option<String>) -> String {
    let Some(name) = name else {
        return "Unnamed Bluetooth device".to_owned();
    };
    let mut result = String::new();
    let mut characters = 0;
    for ch in name.chars() {
        let codepoint = ch as u32;
        if ch.is_control()
            || matches!(codepoint, 0x00ad | 0x0600..=0x0605 | 0x061c | 0x06dd | 0x070f | 0x0890..=0x0891 | 0x08e2 | 0x180e | 0x200b..=0x200f | 0x2028..=0x202e | 0x2060..=0x206f | 0xfeff | 0xfff9..=0xfffb)
        {
            continue;
        }
        if characters >= MAX_NAME_CHARS {
            break;
        }
        result.push(ch);
        characters += 1;
    }
    if result.trim().is_empty() {
        "Unnamed Bluetooth device".to_owned()
    } else {
        result
    }
}

#[cfg(target_os = "linux")]
pub fn read_snapshot() -> DeviceInventory {
    match bounded_call(SNAPSHOT_TIMEOUT, linux::read_objects) {
        Ok(Ok(objects)) => match extract(&objects) {
            Ok(report) => report,
            Err(InventoryError::NoAdapter) => DeviceInventory::new(InventoryStatus::NoAdapter),
            Err(_) => DeviceInventory::new(InventoryStatus::MalformedData),
        },
        Ok(Err(status)) => DeviceInventory::new(status),
        Err(_) => DeviceInventory::new(InventoryStatus::Timeout),
    }
}

fn bounded_call<T: Send + 'static>(
    timeout: Duration,
    operation: impl FnOnce() -> T + Send + 'static,
) -> Result<T, ()> {
    let (send, receive) = std::sync::mpsc::sync_channel(1);
    // A detached worker lets the command honor its wall-clock bound even if
    // the system bus is wedged inside a synchronous zbus call.
    std::thread::spawn(move || {
        let _ = send.send(operation());
    });
    receive.recv_timeout(timeout).map_err(|_| ())
}

#[cfg(not(target_os = "linux"))]
pub fn read_snapshot() -> DeviceInventory {
    DeviceInventory::unsupported()
}

#[cfg(target_os = "linux")]
mod linux {
    use super::*;
    use std::collections::{BTreeMap, HashMap};
    use zbus::{
        blocking::{Connection, Proxy},
        zvariant::{OwnedObjectPath, OwnedValue},
    };

    const BLUEZ: &str = "org.bluez";
    const MANAGER: &str = "org.freedesktop.DBus.ObjectManager";

    pub(super) fn read_objects() -> Result<ManagedObjects, InventoryStatus> {
        let connection = Connection::system().map_err(classify_error)?;
        let proxy = Proxy::new(&connection, BLUEZ, "/", MANAGER).map_err(classify_error)?;
        let raw: HashMap<OwnedObjectPath, HashMap<String, HashMap<String, OwnedValue>>> = proxy
            .call("GetManagedObjects", &())
            .map_err(classify_error)?;
        let mut normalized = ManagedObjects::new();
        for (path, interfaces) in raw {
            let mut normalized_interfaces = BTreeMap::new();
            for (name, properties) in interfaces {
                if name != "org.bluez.Adapter1" && name != "org.bluez.Device1" {
                    continue;
                }
                let mut normalized_properties = BTreeMap::new();
                for (key, value) in properties {
                    if name == "org.bluez.Adapter1" && key != "Powered" {
                        continue;
                    }
                    if name == "org.bluez.Device1"
                        && !matches!(
                            key.as_str(),
                            "Alias"
                                | "Name"
                                | "Connected"
                                | "Paired"
                                | "Trusted"
                                | "ServicesResolved"
                        )
                    {
                        continue;
                    }
                    let value = if key == "Alias" || key == "Name" {
                        String::try_from(value.clone())
                            .ok()
                            .filter(|text| text.len() <= 4096)
                            .map(Property::Text)
                            .unwrap_or(Property::Invalid)
                    } else {
                        bool::try_from(value.clone())
                            .map(Property::Bool)
                            .unwrap_or(Property::Invalid)
                    };
                    normalized_properties.insert(key, value);
                }
                normalized_interfaces.insert(name, normalized_properties);
            }
            normalized.insert(path.to_string(), normalized_interfaces);
        }
        Ok(normalized)
    }

    pub(super) fn classify_error(error: zbus::Error) -> InventoryStatus {
        match &error {
            zbus::Error::InputOutput(_) | zbus::Error::Address(_) | zbus::Error::Handshake(_) => {
                return InventoryStatus::SystemBusUnavailable;
            }
            zbus::Error::FDO(error) => match error.as_ref() {
                zbus::fdo::Error::AccessDenied(_) => return InventoryStatus::AccessDenied,
                zbus::fdo::Error::ServiceUnknown(_) | zbus::fdo::Error::NameHasNoOwner(_) => {
                    return InventoryStatus::BluezUnavailable;
                }
                zbus::fdo::Error::NoReply(_) | zbus::fdo::Error::Timeout(_) => {
                    return InventoryStatus::Timeout;
                }
                zbus::fdo::Error::IOError(_)
                | zbus::fdo::Error::NoServer(_)
                | zbus::fdo::Error::NoNetwork(_)
                | zbus::fdo::Error::Disconnected(_) => {
                    return InventoryStatus::SystemBusUnavailable;
                }
                _ => {}
            },
            zbus::Error::MethodError(name, _, _) => {
                let name = name.as_str().to_ascii_lowercase();
                if name.contains("accessdenied") {
                    return InventoryStatus::AccessDenied;
                }
                if name.contains("serviceunknown") || name.contains("namehasnoowner") {
                    return InventoryStatus::BluezUnavailable;
                }
                if name.contains("noreply") || name.contains("timeout") {
                    return InventoryStatus::Timeout;
                }
            }
            _ => {}
        }
        let message = error.to_string().to_ascii_lowercase();
        if message.contains("accessdenied")
            || message.contains("access denied")
            || message.contains("permission denied")
        {
            InventoryStatus::AccessDenied
        } else if message.contains("serviceunknown")
            || message.contains("name has no owner")
            || message.contains("no such service")
        {
            InventoryStatus::BluezUnavailable
        } else if message.contains("timed out")
            || message.contains("timeout")
            || message.contains("deadline")
        {
            InventoryStatus::Timeout
        } else if message.contains("failed to connect")
            || message.contains("connection refused")
            || message.contains("no such file")
        {
            InventoryStatus::SystemBusUnavailable
        } else {
            InventoryStatus::MalformedData
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;
    fn iface(pairs: &[(&str, Property)]) -> InterfaceProperties {
        pairs
            .iter()
            .map(|(k, v)| ((*k).to_owned(), v.clone()))
            .collect()
    }
    fn objects() -> ManagedObjects {
        let mut o = ManagedObjects::new();
        o.insert(
            "/org/bluez/hci0".into(),
            BTreeMap::from([(
                "org.bluez.Adapter1".into(),
                iface(&[("Powered", Property::Bool(true))]),
            )]),
        );
        o
    }
    #[test]
    fn zero_adapter_is_distinct() {
        assert_eq!(
            extract(&ManagedObjects::new()),
            Err(InventoryError::NoAdapter)
        );
    }
    #[test]
    fn empty_known_inventory_is_ok_and_schema_stable() {
        let r = extract(&objects()).unwrap();
        assert_eq!(r.status, InventoryStatus::Ok);
        assert_eq!(r.devices.len(), 0);
        assert_eq!(r.source, "bluez_known_device_snapshot");
        assert!(!r.scan_performed);
        let json: serde_json::Value =
            serde_json::from_str(&serde_json::to_string(&r).unwrap()).unwrap();
        let keys: std::collections::BTreeSet<_> =
            json.as_object().unwrap().keys().cloned().collect();
        assert_eq!(
            keys,
            [
                "adapters",
                "authorization",
                "devices",
                "scan_performed",
                "schema_version",
                "source",
                "status"
            ]
            .into_iter()
            .map(str::to_owned)
            .collect()
        );
    }
    #[test]
    fn properties_names_association_and_snapshot_ids() {
        let mut o = objects();
        o.insert(
            "/org/bluez/hci0/dev_AA_BB".into(),
            BTreeMap::from([(
                "org.bluez.Device1".into(),
                iface(&[
                    ("Alias", Property::Text("Watch".into())),
                    ("Connected", Property::Bool(false)),
                    ("Paired", Property::Bool(true)),
                ]),
            )]),
        );
        let r = extract(&o).unwrap();
        assert_eq!(r.devices[0].id, "device-001");
        assert_eq!(r.devices[0].adapter_id, "adapter-001");
        assert_eq!(r.devices[0].trusted, None);
        assert_eq!(r.devices[0].display_name, "Watch");
        let json = serde_json::to_string(&r).unwrap();
        assert!(!json.contains("AA_BB"));
    }
    #[test]
    fn multiple_adapters_and_devices_have_snapshot_index_ids() {
        let mut o = objects();
        o.insert(
            "/org/bluez/hci1".into(),
            BTreeMap::from([("org.bluez.Adapter1".into(), iface(&[]))]),
        );
        for (path, name) in [
            ("/org/bluez/hci0/dev_02", "Second"),
            ("/org/bluez/hci1/dev_01", "First"),
        ] {
            o.insert(
                path.into(),
                BTreeMap::from([(
                    "org.bluez.Device1".into(),
                    iface(&[("Name", Property::Text(name.into()))]),
                )]),
            );
        }
        let first = extract(&o).unwrap();
        let second = extract(&o).unwrap();
        assert_eq!(first.adapters.len(), 2);
        assert_eq!(first.devices.len(), 2);
        assert_eq!(first.devices[0].adapter_id, "adapter-001");
        assert_eq!(first.devices[1].adapter_id, "adapter-002");
        assert_eq!(first.devices[0].id, "device-001");
        assert_eq!(first.devices[0].id, second.devices[0].id);
    }
    #[test]
    fn unknown_types_missing_names_and_sanitizing() {
        let mut o = objects();
        o.insert(
            "/org/bluez/hci0/dev_1".into(),
            BTreeMap::from([(
                "org.bluez.Device1".into(),
                iface(&[
                    ("Paired", Property::Invalid),
                    ("Name", Property::Text("A\u{202e}lice\n".into())),
                ]),
            )]),
        );
        assert_eq!(extract(&o), Err(InventoryError::MalformedData));
        assert_eq!(sanitize_name(None), "Unnamed Bluetooth device");
        assert_eq!(sanitize_name(Some("A\u{202e}lice\n".into())), "Alice");
        assert_eq!(sanitize_name(Some("界".repeat(100))).chars().count(), 80);
        let mut nameless = objects();
        nameless.insert(
            "/org/bluez/hci0/dev_no_name".into(),
            BTreeMap::from([("org.bluez.Device1".into(), iface(&[]))]),
        );
        let report = extract(&nameless).unwrap();
        assert_eq!(report.devices[0].display_name, "Unnamed Bluetooth device");
        assert_eq!(report.devices[0].connected, None);
        assert_eq!(report.devices[0].services_resolved, None);
    }
    #[test]
    fn invalid_association_and_limits_fail_closed() {
        let mut o = objects();
        o.insert(
            "/org/bluez/hci9/dev_1".into(),
            BTreeMap::from([("org.bluez.Device1".into(), iface(&[]))]),
        );
        assert_eq!(extract(&o), Err(InventoryError::MalformedData));
        let mut o = ManagedObjects::new();
        for i in 0..=MAX_ADAPTERS {
            o.insert(
                format!("/org/bluez/hci{i}"),
                BTreeMap::from([("org.bluez.Adapter1".into(), iface(&[]))]),
            );
        }
        assert_eq!(extract(&o), Err(InventoryError::TooManyAdapters));
        let mut o = objects();
        for index in 0..=MAX_DEVICES {
            o.insert(
                format!("/org/bluez/hci0/dev_{index:03}"),
                BTreeMap::from([("org.bluez.Device1".into(), iface(&[]))]),
            );
        }
        assert_eq!(extract(&o), Err(InventoryError::TooManyDevices));
    }
    #[test]
    fn inventory_module_has_no_mutating_bluez_method_names() {
        let source = include_str!("device_inventory.rs");
        let production = source.split("#[cfg(test)]").next().unwrap();
        assert_eq!(production.matches(".call(").count(), 1);
        assert!(production.contains("\"GetManagedObjects\""));
    }
    #[test]
    fn bounded_call_times_out_without_waiting_for_worker() {
        let started = std::time::Instant::now();
        let result = bounded_call(Duration::from_millis(1), || {
            std::thread::sleep(Duration::from_millis(25));
            7
        });
        assert_eq!(result, Err(()));
        assert!(started.elapsed() < Duration::from_millis(20));
    }
    #[cfg(target_os = "linux")]
    #[test]
    fn dbus_error_classes_are_stable_and_sanitized() {
        assert_eq!(
            linux::classify_error(zbus::Error::FDO(Box::new(zbus::fdo::Error::AccessDenied(
                "private path".into()
            )))),
            InventoryStatus::AccessDenied
        );
        assert_eq!(
            linux::classify_error(zbus::Error::FDO(Box::new(
                zbus::fdo::Error::ServiceUnknown("org.bluez".into())
            ))),
            InventoryStatus::BluezUnavailable
        );
        assert_eq!(
            linux::classify_error(zbus::Error::FDO(Box::new(zbus::fdo::Error::Timeout(
                "detail".into()
            )))),
            InventoryStatus::Timeout
        );
        assert_eq!(
            linux::classify_error(zbus::Error::FDO(Box::new(zbus::fdo::Error::NoServer(
                "socket".into()
            )))),
            InventoryStatus::SystemBusUnavailable
        );
    }
    #[cfg(not(target_os = "linux"))]
    #[test]
    fn unsupported_platform_is_explicit() {
        assert_eq!(read_snapshot().status, InventoryStatus::UnsupportedPlatform);
    }
}
