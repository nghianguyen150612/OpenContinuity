//! Opt-in Linux BlueZ GATT experiment. This transport is synthetic and never
//! participates in OS authentication or authorization.

use serde::Serialize;
use std::{
    sync::{Arc, atomic::AtomicBool},
    time::Duration,
};

pub const MAX_DURATION_SECONDS: u16 = 300;
pub const MIN_DURATION_SECONDS: u16 = 1;

#[derive(Debug, Serialize)]
pub struct ExperimentReport {
    pub status: &'static str,
    pub stage: &'static str,
    pub adapter: String,
    pub runtime_seconds: u16,
    pub shutdown_reason: Option<&'static str>,
    pub application_registered: bool,
    pub advertisement_registered: bool,
    pub controller_le_peripheral_capability: &'static str,
    pub supported_advertising_instances: Option<u8>,
    pub active_advertising_instances: Option<u8>,
    pub error_class: Option<&'static str>,
    pub error: Option<String>,
    pub authorization: &'static str,
}

#[derive(Debug)]
pub struct ExperimentError {
    pub report: Box<ExperimentReport>,
}

pub fn validate_options(adapter: &str, duration: u16, experimental: bool) -> Result<(), String> {
    if !experimental {
        return Err("the GATT server requires explicit --experimental opt-in".into());
    }
    if !(MIN_DURATION_SECONDS..=MAX_DURATION_SECONDS).contains(&duration) {
        return Err(format!(
            "duration must be between {MIN_DURATION_SECONDS} and {MAX_DURATION_SECONDS} seconds"
        ));
    }
    let suffix = adapter.strip_prefix("hci").unwrap_or_default();
    if suffix.is_empty() || !suffix.bytes().all(|byte| byte.is_ascii_digit()) {
        return Err("adapter must explicitly name a BlueZ adapter such as hci0".into());
    }
    Ok(())
}

#[cfg(target_os = "linux")]
mod linux {
    use super::*;
    use opencontinuity_core::gatt_wire::{
        ControlCommand, FrameAssembler, FrameProgress, WIRE_VERSION, WireError,
    };
    use sha2::{Digest, Sha256};
    use std::{collections::HashMap, fs::File, io::Read, sync::Mutex, thread, time::Instant};
    use zbus::{
        DBusError,
        blocking::{Connection, Proxy},
        fdo::ObjectManager,
        zvariant::{ObjectPath, OwnedObjectPath, OwnedValue},
    };

    const BLUEZ: &str = "org.bluez";
    const OM: &str = "org.freedesktop.DBus.ObjectManager";
    const ADAPTER: &str = "org.bluez.Adapter1";
    const GATT_MANAGER: &str = "org.bluez.GattManager1";
    const ADVERTISING_MANAGER: &str = "org.bluez.LEAdvertisingManager1";
    const SERVICE_UUID: &str = "8e6f0001-6f4e-4c4f-9b4f-4f70656e436f";
    const VERSION_UUID: &str = "8e6f0002-6f4e-4c4f-9b4f-4f70656e436f";
    const CHALLENGE_UUID: &str = "8e6f0003-6f4e-4c4f-9b4f-4f70656e436f";
    const EXCHANGE_UUID: &str = "8e6f0004-6f4e-4c4f-9b4f-4f70656e436f";
    const RESPONSE_UUID: &str = "8e6f0005-6f4e-4c4f-9b4f-4f70656e436f";
    const CONTROL_UUID: &str = "8e6f0006-6f4e-4c4f-9b4f-4f70656e436f";

    #[derive(Debug, DBusError)]
    #[zbus(prefix = "org.bluez.Error")]
    enum BluezGattError {
        Failed(String),
        InvalidArgs(String),
        InvalidValueLength(String),
        NotPermitted(String),
        NotAuthorized(String),
        InvalidOffset(String),
        #[zbus(error)]
        ZBus(zbus::Error),
    }

    fn as_bluez_error(error: WireError) -> BluezGattError {
        use WireError::*;
        match error {
            FrameTooLarge | MessageTooLarge | LengthMismatch => {
                BluezGattError::InvalidValueLength(format!("invalid GATT frame: {error:?}"))
            }
            SessionMismatch
            | VersionMismatch
            | DuplicateOrOutOfOrder
            | StartRequired
            | MessageInProgress
            | PartialMessageExpired
            | AlreadyComplete
            | Malformed
            | InvalidAttMtu => BluezGattError::Failed(format!("invalid GATT frame: {error:?}")),
        }
    }

    fn option_u16(options: &HashMap<String, OwnedValue>, key: &str) -> Option<u16> {
        options
            .get(key)
            .and_then(|value| u16::try_from(value.clone()).ok())
    }

    fn option_device(options: &HashMap<String, OwnedValue>) -> Option<String> {
        options
            .get("device")
            .and_then(|value| OwnedObjectPath::try_from(value.clone()).ok())
            .map(|path| path.to_string())
    }

    fn read_fragment(
        value: &[u8],
        options: &HashMap<String, OwnedValue>,
    ) -> Result<Vec<u8>, BluezGattError> {
        let offset = option_u16(options, "offset").unwrap_or(0) as usize;
        if offset > value.len() {
            return Err(BluezGattError::InvalidOffset(
                "read offset exceeds value length".into(),
            ));
        }
        let mtu = option_u16(options, "mtu").unwrap_or(23) as usize;
        if mtu < 23 {
            return Err(BluezGattError::Failed(
                "BlueZ supplied an invalid ATT MTU".into(),
            ));
        }
        let end = (offset + mtu - 1).min(value.len());
        Ok(value[offset..end].to_vec())
    }

    fn random_bytes<const N: usize>() -> std::io::Result<[u8; N]> {
        let mut bytes = [0u8; N];
        File::open("/dev/urandom")?.read_exact(&mut bytes)?;
        Ok(bytes)
    }

    struct ClientSession {
        session_id: [u8; 8],
        challenge: [u8; 16],
        assembler: FrameAssembler,
        response: Option<Vec<u8>>,
        last_activity: Instant,
    }

    impl ClientSession {
        fn new() -> std::io::Result<Self> {
            let session_id = random_bytes()?;
            Ok(Self {
                session_id,
                challenge: random_bytes()?,
                assembler: FrameAssembler::new(session_id),
                response: None,
                last_activity: Instant::now(),
            })
        }

        fn challenge_bytes(&self) -> Vec<u8> {
            [
                vec![WIRE_VERSION],
                self.session_id.to_vec(),
                self.challenge.to_vec(),
            ]
            .concat()
        }

        fn status_response(&self, status: u8, payload: &[u8]) -> Vec<u8> {
            let digest = Sha256::digest(payload);
            [
                vec![WIRE_VERSION, status],
                self.session_id.to_vec(),
                (payload.len() as u16).to_le_bytes().to_vec(),
                digest.to_vec(),
            ]
            .concat()
        }

        fn error_response(&self, error: WireError) -> Vec<u8> {
            [
                vec![WIRE_VERSION, 0x80],
                self.session_id.to_vec(),
                vec![error_code(error)],
            ]
            .concat()
        }

        fn rotate(&mut self) -> Result<(), BluezGattError> {
            let new_id = random_bytes().map_err(|error| {
                BluezGattError::Failed(format!("operating-system random source failed: {error}"))
            })?;
            let new_challenge = random_bytes().map_err(|error| {
                BluezGattError::Failed(format!("operating-system random source failed: {error}"))
            })?;
            self.session_id = new_id;
            self.challenge = new_challenge;
            self.assembler.reset(new_id);
            self.last_activity = Instant::now();
            Ok(())
        }
    }

    fn error_code(error: WireError) -> u8 {
        use WireError::*;
        match error {
            InvalidAttMtu => 1,
            FrameTooLarge => 2,
            Malformed => 3,
            VersionMismatch => 4,
            SessionMismatch => 5,
            DuplicateOrOutOfOrder => 6,
            StartRequired => 7,
            MessageInProgress => 8,
            MessageTooLarge => 9,
            LengthMismatch => 10,
            PartialMessageExpired => 11,
            AlreadyComplete => 12,
        }
    }

    #[derive(Default)]
    struct GattState {
        clients: Mutex<HashMap<String, ClientSession>>,
    }

    impl GattState {
        fn ensure_client(&self, device: &str) -> Result<(), BluezGattError> {
            let mut clients = self
                .clients
                .lock()
                .map_err(|_| BluezGattError::Failed("GATT client state lock poisoned".into()))?;
            let rotate = clients
                .get(device)
                .is_none_or(|session| session.last_activity.elapsed() > Duration::from_secs(30));
            if rotate {
                clients.insert(
                    device.to_owned(),
                    ClientSession::new().map_err(|error| {
                        BluezGattError::Failed(format!(
                            "operating-system random source failed: {error}"
                        ))
                    })?,
                );
            }
            if let Some(session) = clients.get_mut(device) {
                session.last_activity = Instant::now();
            }
            Ok(())
        }

        fn client_value(
            &self,
            device: &str,
            kind: CharacteristicKind,
        ) -> Result<Vec<u8>, BluezGattError> {
            if kind == CharacteristicKind::Version {
                return Ok(vec![WIRE_VERSION]);
            }
            self.ensure_client(device)?;
            let mut clients = self
                .clients
                .lock()
                .map_err(|_| BluezGattError::Failed("GATT client state lock poisoned".into()))?;
            let session = clients.get_mut(device).expect("client was inserted");
            session.last_activity = Instant::now();
            match kind {
                CharacteristicKind::Version => Ok(vec![WIRE_VERSION]),
                CharacteristicKind::Challenge => Ok(session.challenge_bytes()),
                CharacteristicKind::Response => session.response.clone().ok_or_else(|| {
                    BluezGattError::NotPermitted("no synthetic response is available".into())
                }),
                _ => Err(BluezGattError::NotPermitted(
                    "characteristic is not readable".into(),
                )),
            }
        }

        fn write_exchange(
            &self,
            device: &str,
            frame: &[u8],
            mtu: usize,
        ) -> Result<(), BluezGattError> {
            let mut clients = self
                .clients
                .lock()
                .map_err(|_| BluezGattError::Failed("GATT client state lock poisoned".into()))?;
            let session = clients.get_mut(device).ok_or_else(|| {
                BluezGattError::NotPermitted(
                    "read the challenge characteristic before writing".into(),
                )
            })?;
            session.last_activity = Instant::now();
            match session.assembler.push(frame, mtu, Instant::now()) {
                Ok(FrameProgress::Partial { .. }) => Ok(()),
                Ok(FrameProgress::Complete(_payload)) => {
                    session.response = Some(session.status_response(1, b"pending completion"));
                    Ok(())
                }
                Err(error) => {
                    session.response = Some(session.error_response(error));
                    session.rotate()?;
                    Err(as_bluez_error(error))
                }
            }
        }

        fn write_control(&self, device: &str, value: &[u8]) -> Result<(), BluezGattError> {
            let mut clients = self
                .clients
                .lock()
                .map_err(|_| BluezGattError::Failed("GATT client state lock poisoned".into()))?;
            let session = clients.get_mut(device).ok_or_else(|| {
                BluezGattError::NotPermitted(
                    "read the challenge characteristic before writing".into(),
                )
            })?;
            session.last_activity = Instant::now();
            let command = match session.assembler.check_control(value) {
                Ok(command) => command,
                Err(error) => {
                    session.response = Some(session.error_response(error));
                    return Err(as_bluez_error(error));
                }
            };
            match command {
                ControlCommand::Complete => {
                    let payload = session
                        .assembler
                        .completed_payload()
                        .ok_or_else(|| {
                            BluezGattError::NotPermitted(
                                "no complete synthetic payload is awaiting acknowledgement".into(),
                            )
                        })?
                        .to_vec();
                    session.response = Some(session.status_response(0, &payload));
                    session.rotate()?;
                    Ok(())
                }
                ControlCommand::Cancel => {
                    session.response = Some(session.status_response(2, b"cancelled"));
                    session.rotate()?;
                    Ok(())
                }
            }
        }

        fn prune_devices(&self, connected: &HashMap<String, bool>) -> Result<(), String> {
            let mut clients = self
                .clients
                .lock()
                .map_err(|_| "GATT client state lock poisoned".to_owned())?;
            clients.retain(|device, session| {
                connected.get(device).copied().unwrap_or(false)
                    && session.last_activity.elapsed() <= Duration::from_secs(30)
            });
            Ok(())
        }
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    enum CharacteristicKind {
        Version,
        Challenge,
        Exchange,
        Response,
        Control,
    }

    struct GattService {
        characteristics: Vec<String>,
    }

    #[zbus::interface(name = "org.bluez.GattService1")]
    impl GattService {
        #[zbus(property, name = "UUID")]
        fn uuid(&self) -> &str {
            SERVICE_UUID
        }

        #[zbus(property)]
        fn primary(&self) -> bool {
            true
        }

        #[zbus(property)]
        fn characteristics(&self) -> Vec<OwnedObjectPath> {
            self.characteristics
                .iter()
                .filter_map(|path| OwnedObjectPath::try_from(path.as_str()).ok())
                .collect()
        }
    }

    struct GattCharacteristic {
        service: String,
        uuid: &'static str,
        flags: Vec<String>,
        kind: CharacteristicKind,
        state: Arc<GattState>,
    }

    #[zbus::interface(name = "org.bluez.GattCharacteristic1")]
    impl GattCharacteristic {
        #[zbus(property, name = "UUID")]
        fn uuid(&self) -> &str {
            self.uuid
        }

        #[zbus(property)]
        fn service(&self) -> OwnedObjectPath {
            OwnedObjectPath::try_from(self.service.as_str()).expect("constant D-Bus object path")
        }

        #[zbus(property)]
        fn flags(&self) -> Vec<String> {
            self.flags.clone()
        }

        fn read_value(
            &self,
            options: HashMap<String, OwnedValue>,
        ) -> Result<Vec<u8>, BluezGattError> {
            let device = option_device(&options).ok_or_else(|| {
                BluezGattError::NotPermitted("BlueZ did not provide the connected device".into())
            })?;
            let value = self.state.client_value(&device, self.kind)?;
            read_fragment(&value, &options)
        }

        fn write_value(
            &self,
            value: Vec<u8>,
            options: HashMap<String, OwnedValue>,
        ) -> Result<(), BluezGattError> {
            let device = option_device(&options).ok_or_else(|| {
                BluezGattError::NotPermitted("BlueZ did not provide the connected device".into())
            })?;
            let mtu = option_u16(&options, "mtu").unwrap_or(23) as usize;
            match self.kind {
                CharacteristicKind::Exchange => self.state.write_exchange(&device, &value, mtu),
                CharacteristicKind::Control => self.state.write_control(&device, &value),
                _ => Err(BluezGattError::NotPermitted(
                    "characteristic is read-only".into(),
                )),
            }
        }
    }

    struct LeAdvertisement;

    #[zbus::interface(name = "org.bluez.LEAdvertisement1")]
    impl LeAdvertisement {
        #[zbus(property, name = "Type")]
        fn type_(&self) -> &str {
            "peripheral"
        }

        #[zbus(property, name = "ServiceUUIDs")]
        fn service_uuids(&self) -> Vec<String> {
            vec![SERVICE_UUID.to_owned()]
        }

        #[zbus(property)]
        fn local_name(&self) -> &str {
            "OC Research"
        }

        fn release(&self) {}
    }

    fn managed_objects(connection: &Connection) -> Result<zbus::fdo::ManagedObjects, zbus::Error> {
        let proxy = Proxy::new(connection, BLUEZ, "/", OM)?;
        proxy.call("GetManagedObjects", &())
    }

    fn has_interface(objects: &zbus::fdo::ManagedObjects, path: &str, interface: &str) -> bool {
        ObjectPath::try_from(path)
            .ok()
            .and_then(|path| objects.get(&path))
            .is_some_and(|interfaces| interfaces.keys().any(|name| name.as_str() == interface))
    }

    fn adapter_powered(objects: &zbus::fdo::ManagedObjects, path: &str) -> Option<bool> {
        let object_path = ObjectPath::try_from(path).ok()?;
        let adapter = objects.get(&object_path)?;
        adapter
            .iter()
            .find(|(name, _)| name.as_str() == ADAPTER)?
            .1
            .get("Powered")
            .and_then(|value| bool::try_from(value.clone()).ok())
    }

    fn manager_capability(
        objects: &zbus::fdo::ManagedObjects,
        path: &str,
        property: &str,
    ) -> Option<u8> {
        let path = ObjectPath::try_from(path).ok()?;
        objects
            .get(&path)?
            .iter()
            .find(|(name, _)| name.as_str() == ADVERTISING_MANAGER)?
            .1
            .get(property)
            .and_then(|value| u8::try_from(value.clone()).ok())
    }

    fn device_connections(objects: &zbus::fdo::ManagedObjects) -> HashMap<String, bool> {
        objects
            .iter()
            .filter_map(|(path, interfaces)| {
                let properties = interfaces
                    .iter()
                    .find(|(name, _)| name.as_str() == "org.bluez.Device1")?
                    .1;
                let connected = properties
                    .get("Connected")
                    .and_then(|value| bool::try_from(value.clone()).ok())?;
                Some((path.to_string(), connected))
            })
            .collect()
    }

    fn call_manager(
        connection: &Connection,
        path: &str,
        interface: &str,
        method: &str,
        object: &str,
    ) -> Result<(), zbus::Error> {
        let proxy = Proxy::new(connection, BLUEZ, path, interface)?;
        let object_path = OwnedObjectPath::try_from(object).map_err(zbus::Error::from)?;
        let options: HashMap<String, OwnedValue> = HashMap::new();
        if method.starts_with("Unregister") {
            proxy.call(method, &(object_path,))
        } else {
            proxy.call(method, &(object_path, options))
        }
    }

    struct Registration {
        connection: Connection,
        adapter_path: String,
        app_path: String,
        ad_path: String,
        app_registered: bool,
        ad_registered: bool,
    }

    impl Registration {
        fn cleanup(&mut self) -> Vec<String> {
            let mut errors = Vec::new();
            if self.ad_registered {
                match call_manager(
                    &self.connection,
                    &self.adapter_path,
                    ADVERTISING_MANAGER,
                    "UnregisterAdvertisement",
                    &self.ad_path,
                ) {
                    Ok(()) => self.ad_registered = false,
                    Err(error) if registration_already_gone(&error) => {
                        self.ad_registered = false;
                    }
                    Err(error) => errors.push(format!("advertisement unregister failed: {error}")),
                }
            }
            if self.app_registered {
                let proxy = Proxy::new(
                    &self.connection,
                    BLUEZ,
                    self.adapter_path.as_str(),
                    GATT_MANAGER,
                );
                let result = proxy.and_then(|proxy| {
                    let app = OwnedObjectPath::try_from(self.app_path.as_str())
                        .map_err(zbus::Error::from)?;
                    proxy.call("UnregisterApplication", &(app,))
                });
                match result {
                    Ok(()) => self.app_registered = false,
                    Err(error) if registration_already_gone(&error) => {
                        self.app_registered = false;
                    }
                    Err(error) => {
                        errors.push(format!("GATT application unregister failed: {error}"))
                    }
                }
            }
            errors
        }
    }

    impl Drop for Registration {
        fn drop(&mut self) {
            let _ = self.cleanup();
        }
    }

    fn classify_dbus_error(error: &zbus::Error) -> &'static str {
        classify_error_text(&error.to_string())
    }

    fn registration_already_gone(error: &zbus::Error) -> bool {
        let text = error.to_string().to_lowercase();
        text.contains("doesnotexist") || text.contains("unknownobject")
    }

    fn classify_error_text(error: &str) -> &'static str {
        let text = error.to_lowercase();
        if text.contains("accessdenied")
            || text.contains("access denied")
            || text.contains("not authorized")
        {
            "dbus_access_denied"
        } else if text.contains("org.bluez.error.notpermitted")
            || text.contains("org.bluez.error.alreadyexists")
            || text.contains("org.bluez.error.invalidlength")
            || text.contains("advertising slots")
        {
            "bluez_registration_rejected"
        } else if text.contains("serviceunknown")
            || text.contains("name has no owner")
            || text.contains("org.bluez is not provided")
        {
            "bluez_service_unavailable"
        } else {
            "dbus_error"
        }
    }

    fn err_report(
        adapter: &str,
        duration: u16,
        stage: &'static str,
        error: impl ToString,
    ) -> ExperimentError {
        let error = error.to_string();
        let error_lower = error.to_lowercase();
        let error_class = if error_lower.contains("accessdenied")
            || error_lower.contains("access denied")
            || error_lower.contains("not authorized")
        {
            Some("dbus_access_denied")
        } else if error_lower.contains("org.bluez.error.notpermitted")
            || error_lower.contains("org.bluez.error.alreadyexists")
            || error_lower.contains("advertising slots")
        {
            Some("bluez_registration_rejected")
        } else if error_lower.contains("system dbus unavailable")
            || error_lower.contains("system d-bus unavailable")
        {
            Some("system_bus_unavailable")
        } else if error_lower.contains("org.bluez is not provided")
            || error_lower.contains("serviceunknown")
        {
            Some("bluez_service_unavailable")
        } else {
            None
        };
        ExperimentError {
            report: Box::new(ExperimentReport {
                status: "failed",
                stage,
                adapter: adapter.into(),
                runtime_seconds: duration,
                shutdown_reason: None,
                application_registered: false,
                advertisement_registered: false,
                controller_le_peripheral_capability: "unknown",
                supported_advertising_instances: None,
                active_advertising_instances: None,
                error_class,
                error: Some(error),
                authorization: "never_evaluated",
            }),
        }
    }

    pub fn serve(
        adapter: &str,
        duration: u16,
        terminate: Arc<AtomicBool>,
    ) -> Result<ExperimentReport, ExperimentError> {
        let connection = Connection::system().map_err(|error| {
            err_report(
                adapter,
                duration,
                "preflight",
                format!("system D-Bus unavailable: {error}"),
            )
        })?;
        let objects = managed_objects(&connection).map_err(|error| {
            err_report(
                adapter,
                duration,
                "preflight",
                format!("BlueZ discovery {}: {error}", classify_dbus_error(&error)),
            )
        })?;
        let adapter_path = format!("/org/bluez/{adapter}");
        if !objects
            .contains_key(&ObjectPath::try_from(adapter_path.as_str()).expect("valid adapter path"))
        {
            return Err(err_report(
                adapter,
                duration,
                "preflight",
                "explicit adapter is absent from BlueZ ObjectManager",
            ));
        }
        if !has_interface(&objects, &adapter_path, GATT_MANAGER) {
            return Err(err_report(
                adapter,
                duration,
                "preflight",
                "selected adapter does not expose org.bluez.GattManager1 (unsupported)",
            ));
        }
        if !has_interface(&objects, &adapter_path, ADVERTISING_MANAGER) {
            return Err(err_report(
                adapter,
                duration,
                "preflight",
                "selected adapter does not expose org.bluez.LEAdvertisingManager1 (unsupported)",
            ));
        }
        if adapter_powered(&objects, &adapter_path) != Some(true) {
            return Err(err_report(
                adapter,
                duration,
                "preflight",
                "selected adapter is not reported powered; this experiment never powers it on",
            ));
        }
        let supported_instances = manager_capability(&objects, &adapter_path, "SupportedInstances");
        let active_instances = manager_capability(&objects, &adapter_path, "ActiveInstances");

        let nonce = random_bytes::<8>().map_err(|error| {
            err_report(
                adapter,
                duration,
                "preflight",
                format!("OS random source unavailable: {error}"),
            )
        })?;
        let mut token = String::with_capacity(16);
        for byte in nonce {
            use std::fmt::Write as _;
            write!(&mut token, "{byte:02x}").expect("writing to String cannot fail");
        }
        let app_path = format!("/org/opencontinuity/gatt_{token}");
        let ad_path = format!("/org/opencontinuity/advertisement_{token}");
        let service_path = format!("{app_path}/service0");
        let state = Arc::new(GattState::default());
        let specs = [
            (
                "char0",
                VERSION_UUID,
                vec!["read".to_owned()],
                CharacteristicKind::Version,
            ),
            (
                "char1",
                CHALLENGE_UUID,
                vec!["read".to_owned()],
                CharacteristicKind::Challenge,
            ),
            (
                "char2",
                EXCHANGE_UUID,
                vec!["write".to_owned()],
                CharacteristicKind::Exchange,
            ),
            (
                "char3",
                RESPONSE_UUID,
                vec!["read".to_owned()],
                CharacteristicKind::Response,
            ),
            (
                "char4",
                CONTROL_UUID,
                vec!["write".to_owned()],
                CharacteristicKind::Control,
            ),
        ];
        let char_paths = specs
            .iter()
            .map(|(name, _, _, _)| format!("{service_path}/{name}"))
            .collect::<Vec<_>>();
        connection
            .object_server()
            .at(
                OwnedObjectPath::try_from(app_path.as_str()).expect("valid application path"),
                ObjectManager,
            )
            .map_err(|error| {
                err_report(
                    adapter,
                    duration,
                    "preflight",
                    format!("could not export GATT ObjectManager: {error}"),
                )
            })?;
        connection
            .object_server()
            .at(
                OwnedObjectPath::try_from(service_path.as_str()).expect("valid service path"),
                GattService {
                    characteristics: char_paths.clone(),
                },
            )
            .map_err(|error| {
                err_report(
                    adapter,
                    duration,
                    "preflight",
                    format!("could not export GATT service: {error}"),
                )
            })?;
        for ((_, uuid, flags, kind), path) in specs.iter().zip(char_paths.iter()) {
            connection
                .object_server()
                .at(
                    OwnedObjectPath::try_from(path.as_str()).expect("valid characteristic path"),
                    GattCharacteristic {
                        service: service_path.clone(),
                        uuid,
                        flags: flags.clone(),
                        kind: *kind,
                        state: state.clone(),
                    },
                )
                .map_err(|error| {
                    err_report(
                        adapter,
                        duration,
                        "preflight",
                        format!("could not export GATT characteristic: {error}"),
                    )
                })?;
        }
        connection
            .object_server()
            .at(
                OwnedObjectPath::try_from(ad_path.as_str()).expect("valid advertisement path"),
                LeAdvertisement,
            )
            .map_err(|error| {
                err_report(
                    adapter,
                    duration,
                    "preflight",
                    format!("could not export LE advertisement: {error}"),
                )
            })?;

        let mut registration = Registration {
            connection: connection.clone(),
            adapter_path: adapter_path.clone(),
            app_path: app_path.clone(),
            ad_path: ad_path.clone(),
            app_registered: false,
            ad_registered: false,
        };
        let app_object = OwnedObjectPath::try_from(app_path.as_str()).expect("validated path");
        let options: HashMap<String, OwnedValue> = HashMap::new();
        let manager = Proxy::new(&connection, BLUEZ, adapter_path.as_str(), GATT_MANAGER).map_err(
            |error| {
                err_report(
                    adapter,
                    duration,
                    "registering_application",
                    format!("{}: {error}", classify_dbus_error(&error)),
                )
            },
        )?;
        manager
            .call::<_, _, ()>("RegisterApplication", &(app_object, options))
            .map_err(|error| {
                err_report(
                    adapter,
                    duration,
                    "registering_application",
                    format!(
                        "BlueZ GATT application registration failed ({}): {error}",
                        classify_dbus_error(&error)
                    ),
                )
            })?;
        registration.app_registered = true;

        if let Err(error) = call_manager(
            &connection,
            &adapter_path,
            ADVERTISING_MANAGER,
            "RegisterAdvertisement",
            &ad_path,
        ) {
            let cleanup = registration.cleanup();
            return Err(err_report(
                adapter,
                duration,
                "registering_advertisement",
                format!(
                    "BlueZ advertisement registration failed ({}): {error}; rollback: {}",
                    classify_dbus_error(&error),
                    cleanup.join("; ")
                ),
            ));
        }
        registration.ad_registered = true;

        let start = Instant::now();
        let deadline = start + Duration::from_secs(duration as u64);
        let mut last_probe = Instant::now();
        let shutdown_reason = loop {
            if terminate.load(std::sync::atomic::Ordering::Relaxed) {
                break "termination_signal";
            }
            if Instant::now() >= deadline {
                break "timeout";
            }
            if last_probe.elapsed() >= Duration::from_secs(1) {
                let current = managed_objects(&connection).map_err(|error| {
                    err_report(
                        adapter,
                        duration,
                        "active",
                        format!("D-Bus connection or BlueZ state lost: {error}"),
                    )
                })?;
                if !current.contains_key(
                    &ObjectPath::try_from(adapter_path.as_str()).expect("valid adapter path"),
                ) || adapter_powered(&current, &adapter_path) != Some(true)
                {
                    break "adapter_lost";
                }
                state
                    .prune_devices(&device_connections(&current))
                    .map_err(|error| err_report(adapter, duration, "active", error))?;
                last_probe = Instant::now();
            }
            thread::sleep(Duration::from_millis(100));
        };
        let elapsed = start.elapsed().as_secs().min(u16::MAX as u64) as u16;
        let cleanup_errors = registration.cleanup();
        if !cleanup_errors.is_empty() {
            return Err(err_report(
                adapter,
                duration,
                "stopping",
                cleanup_errors.join("; "),
            ));
        }
        Ok(ExperimentReport {
            status: "stopped",
            stage: "stopped",
            adapter: adapter.into(),
            runtime_seconds: elapsed,
            shutdown_reason: Some(shutdown_reason),
            application_registered: false,
            advertisement_registered: false,
            // Manager interfaces establish API availability, not controller radio capability.
            controller_le_peripheral_capability: "unknown",
            supported_advertising_instances: supported_instances,
            active_advertising_instances: active_instances,
            error_class: None,
            error: None,
            authorization: "never_evaluated",
        })
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        #[test]
        fn dbus_errors_keep_policy_denial_distinct_from_missing_service() {
            assert_eq!(
                classify_error_text("org.freedesktop.DBus.Error.AccessDenied"),
                "dbus_access_denied"
            );
            assert_eq!(
                classify_error_text("org.bluez.Error.NotPermitted: advertising slots exhausted"),
                "bluez_registration_rejected"
            );
            assert_eq!(
                classify_error_text(
                    "org.freedesktop.DBus.Error.ServiceUnknown: org.bluez is not provided"
                ),
                "bluez_service_unavailable"
            );
            assert_eq!(
                classify_error_text("org.freedesktop.DBus.Error.NoReply"),
                "dbus_error"
            );
        }

        #[test]
        fn client_sessions_are_isolated_and_disconnect_drops_state() {
            let state = GattState::default();
            let first = state
                .client_value("/org/bluez/hci0/dev_01", CharacteristicKind::Challenge)
                .unwrap();
            let second = state
                .client_value("/org/bluez/hci0/dev_02", CharacteristicKind::Challenge)
                .unwrap();
            assert_ne!(&first[1..9], &second[1..9]);
            let first_id: [u8; 8] = first[1..9].try_into().unwrap();
            let frame =
                opencontinuity_core::gatt_wire::encode_frame(first_id, 0, true, true, 1, b"x")
                    .unwrap();
            assert!(
                state
                    .write_exchange("/org/bluez/hci0/dev_02", &frame, 23)
                    .is_err()
            );
            state
                .prune_devices(&HashMap::from([(
                    "/org/bluez/hci0/dev_01".to_owned(),
                    true,
                )]))
                .unwrap();
            let clients = state.clients.lock().unwrap();
            assert!(clients.contains_key("/org/bluez/hci0/dev_01"));
            assert!(!clients.contains_key("/org/bluez/hci0/dev_02"));
        }
    }
}

#[cfg(target_os = "linux")]
pub use linux::serve;

#[cfg(not(target_os = "linux"))]
pub fn serve(
    adapter: &str,
    duration: u16,
    _terminate: Arc<AtomicBool>,
) -> Result<ExperimentReport, ExperimentError> {
    Err(ExperimentError {
        report: Box::new(ExperimentReport {
            status: "failed",
            stage: "preflight",
            adapter: adapter.into(),
            runtime_seconds: duration,
            shutdown_reason: None,
            application_registered: false,
            advertisement_registered: false,
            controller_le_peripheral_capability: "unknown",
            supported_advertising_instances: None,
            active_advertising_instances: None,
            error_class: Some("unsupported_platform"),
            error: Some("experimental BlueZ GATT transport is Linux-only".into()),
            authorization: "never_evaluated",
        }),
    })
}

#[allow(dead_code)]
fn _duration(_: Duration) {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn requires_explicit_opt_in_adapter_and_bounded_duration() {
        assert!(validate_options("hci0", 60, false).is_err());
        assert!(validate_options("hci0", 0, true).is_err());
        assert!(validate_options("hci0", 301, true).is_err());
        assert!(validate_options("hci0", 60, true).is_ok());
        assert!(validate_options("/org/bluez/hci0", 60, true).is_err());
        assert!(validate_options("hci0;pair", 60, true).is_err());
    }
}
