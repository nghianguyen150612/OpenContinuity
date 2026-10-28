#![cfg(target_os = "linux")]

use sha2::{Digest, Sha256};
use std::{
    io::{BufRead, BufReader},
    process::{Child, Command, Stdio},
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, AtomicUsize, Ordering},
        mpsc,
    },
    thread,
    time::Duration,
};
use zbus::{
    DBusError,
    blocking::{Proxy, connection::Builder},
    fdo::ObjectManager,
    message::Header,
    zvariant::{OwnedObjectPath, OwnedValue, Value},
};

#[derive(Debug, DBusError)]
#[zbus(prefix = "org.bluez.Error")]
enum MockBluezError {
    Failed(String),
    NotPermitted(String),
    AlreadyExists(String),
    DoesNotExist(String),
}

#[derive(Clone, Copy)]
enum FailureMode {
    None,
    Application,
    Advertisement,
    NoBluez,
    NoAdapter,
    NoGatt,
    NoAdvertisement,
    MissingAdvertisementOnCleanup,
    CleanupFailure,
    AdvertisementAndRollbackFailure,
    SlowAdvertisement,
}

struct MockAdapter {
    powered: Arc<AtomicBool>,
}

#[zbus::interface(name = "org.bluez.Adapter1")]
impl MockAdapter {
    #[zbus(property)]
    fn powered(&self) -> bool {
        self.powered.load(Ordering::Relaxed)
    }
}

struct MockGattManager {
    events: Arc<Mutex<Vec<&'static str>>>,
    failure: FailureMode,
    app_info: Arc<Mutex<Option<(String, String)>>>,
}

#[zbus::interface(name = "org.bluez.GattManager1")]
impl MockGattManager {
    fn register_application(
        &self,
        application: OwnedObjectPath,
        _options: std::collections::HashMap<String, zbus::zvariant::OwnedValue>,
        #[zbus(header)] header: Header<'_>,
    ) -> Result<(), MockBluezError> {
        self.events.lock().unwrap().push("register_application");
        let caller = header
            .sender()
            .ok_or_else(|| MockBluezError::Failed("missing D-Bus sender".into()))?
            .to_string();
        *self.app_info.lock().unwrap() = Some((caller, application.to_string()));
        if matches!(self.failure, FailureMode::Application) {
            return Err(MockBluezError::AlreadyExists(
                "mock GATT registration rejection".into(),
            ));
        }
        Ok(())
    }

    fn unregister_application(&self, _application: OwnedObjectPath) -> Result<(), MockBluezError> {
        self.events.lock().unwrap().push("unregister_application");
        if matches!(self.failure, FailureMode::AdvertisementAndRollbackFailure) {
            return Err(MockBluezError::Failed("rollback failed".into()));
        }
        Ok(())
    }
}

struct MockAdvertisingManager {
    events: Arc<Mutex<Vec<&'static str>>>,
    failure: FailureMode,
    advertisement_info: Arc<Mutex<Option<(String, String)>>>,
}

#[zbus::interface(name = "org.bluez.LEAdvertisingManager1")]
impl MockAdvertisingManager {
    fn register_advertisement(
        &self,
        advertisement: OwnedObjectPath,
        _options: std::collections::HashMap<String, zbus::zvariant::OwnedValue>,
        #[zbus(header)] header: Header<'_>,
    ) -> Result<(), MockBluezError> {
        self.events.lock().unwrap().push("register_advertisement");
        if matches!(self.failure, FailureMode::SlowAdvertisement) {
            thread::sleep(Duration::from_secs(3));
        }
        let caller = header
            .sender()
            .ok_or_else(|| MockBluezError::Failed("missing D-Bus sender".into()))?
            .to_string();
        *self.advertisement_info.lock().unwrap() = Some((caller, advertisement.to_string()));
        if matches!(
            self.failure,
            FailureMode::Advertisement | FailureMode::AdvertisementAndRollbackFailure
        ) {
            return Err(MockBluezError::NotPermitted(
                "mock advertising slots exhausted".into(),
            ));
        }
        Ok(())
    }

    fn unregister_advertisement(
        &self,
        _advertisement: OwnedObjectPath,
    ) -> Result<(), MockBluezError> {
        self.events.lock().unwrap().push("unregister_advertisement");
        if matches!(self.failure, FailureMode::CleanupFailure) {
            return Err(MockBluezError::Failed("cleanup failed".into()));
        }
        if matches!(self.failure, FailureMode::MissingAdvertisementOnCleanup) {
            return Err(MockBluezError::DoesNotExist(
                "mock advertisement already removed by BlueZ".into(),
            ));
        }
        Ok(())
    }

    #[zbus(property)]
    fn supported_instances(&self) -> u8 {
        2
    }

    #[zbus(property)]
    fn active_instances(&self) -> u8 {
        0
    }
}

struct PrivateBluez {
    daemon: Child,
    _service_thread: thread::JoinHandle<()>,
    events: Arc<Mutex<Vec<&'static str>>>,
    app_object_count: Arc<AtomicUsize>,
    app_info: Arc<Mutex<Option<(String, String)>>>,
    advertisement_info: Arc<Mutex<Option<(String, String)>>>,
    powered: Arc<AtomicBool>,
    other_events: Arc<Mutex<Vec<&'static str>>>,
    address: String,
    stop: Arc<AtomicBool>,
}

impl PrivateBluez {
    fn start(failure: FailureMode) -> Self {
        let mut daemon = Command::new("dbus-daemon")
            .args(["--session", "--nofork", "--print-address=1"])
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .expect("dbus-daemon is installed in the Linux test environment");
        let mut address = String::new();
        BufReader::new(daemon.stdout.take().unwrap())
            .read_line(&mut address)
            .expect("read private D-Bus address");
        let address = address.trim().to_owned();
        let events = Arc::new(Mutex::new(Vec::new()));
        let service_events = events.clone();
        let other_events = Arc::new(Mutex::new(Vec::new()));
        let service_other_events = other_events.clone();
        let app_object_count = Arc::new(AtomicUsize::new(0));
        let app_info = Arc::new(Mutex::new(None));
        let service_app_info = app_info.clone();
        let advertisement_info = Arc::new(Mutex::new(None));
        let service_advertisement_info = advertisement_info.clone();
        let powered = Arc::new(AtomicBool::new(true));
        let service_powered = powered.clone();
        let service_address = address.clone();
        let stop = Arc::new(AtomicBool::new(false));
        let service_stop = stop.clone();
        let (ready_tx, ready_rx) = mpsc::channel();
        let service_thread = thread::spawn(move || {
            let service = if matches!(failure, FailureMode::NoBluez) {
                Builder::address(service_address.as_str())
                    .unwrap()
                    .build()
                    .unwrap()
            } else {
                Builder::address(service_address.as_str())
                    .unwrap()
                    .name("org.bluez")
                    .unwrap()
                    .build()
                    .unwrap()
            };
            if !matches!(failure, FailureMode::NoBluez) {
                service.object_server().at("/", ObjectManager).unwrap();
            }
            if !matches!(failure, FailureMode::NoBluez | FailureMode::NoAdapter) {
                service
                    .object_server()
                    .at(
                        "/org/bluez/hci0",
                        MockAdapter {
                            powered: service_powered,
                        },
                    )
                    .unwrap();
                if !matches!(failure, FailureMode::NoGatt) {
                    service
                        .object_server()
                        .at(
                            "/org/bluez/hci0",
                            MockGattManager {
                                events: service_events.clone(),
                                failure,
                                app_info: service_app_info,
                            },
                        )
                        .unwrap();
                }
                if !matches!(failure, FailureMode::NoAdvertisement) {
                    service
                        .object_server()
                        .at(
                            "/org/bluez/hci0",
                            MockAdvertisingManager {
                                events: service_events,
                                failure,
                                advertisement_info: service_advertisement_info,
                            },
                        )
                        .unwrap();
                }
                service
                    .object_server()
                    .at(
                        "/org/bluez/hci1",
                        MockAdapter {
                            powered: Arc::new(AtomicBool::new(true)),
                        },
                    )
                    .unwrap();
                if !matches!(failure, FailureMode::NoGatt) {
                    service
                        .object_server()
                        .at(
                            "/org/bluez/hci1",
                            MockGattManager {
                                events: service_other_events.clone(),
                                failure,
                                app_info: Arc::new(Mutex::new(None)),
                            },
                        )
                        .unwrap();
                }
                if !matches!(failure, FailureMode::NoAdvertisement) {
                    service
                        .object_server()
                        .at(
                            "/org/bluez/hci1",
                            MockAdvertisingManager {
                                events: service_other_events,
                                failure,
                                advertisement_info: Arc::new(Mutex::new(None)),
                            },
                        )
                        .unwrap();
                }
            }
            ready_tx.send(()).unwrap();
            while !service_stop.load(Ordering::Relaxed) {
                thread::sleep(Duration::from_millis(10));
            }
        });
        ready_rx
            .recv_timeout(Duration::from_secs(5))
            .expect("mock BlueZ service acquired its bus name");
        Self {
            daemon,
            _service_thread: service_thread,
            events,
            app_object_count,
            app_info,
            advertisement_info,
            powered,
            other_events,
            address,
            stop,
        }
    }

    fn run_cli(&self, duration: u16) -> std::process::Output {
        self.spawn_cli(duration).wait_with_output().unwrap()
    }

    fn run_cli_at(&self, duration: u16, address: &str) -> std::process::Output {
        Command::new(env!("CARGO_BIN_EXE_opencontinuity"))
            .args([
                "research",
                "watch-gatt",
                "serve",
                "--adapter",
                "hci0",
                "--duration-seconds",
                &duration.to_string(),
                "--experimental",
                "--json",
            ])
            .env("DBUS_SYSTEM_BUS_ADDRESS", address)
            .output()
            .unwrap()
    }

    fn remove_controller_during_run(&self) -> serde_json::Value {
        let mut child = self.spawn_cli(30);
        let deadline = std::time::Instant::now() + Duration::from_secs(5);
        while !self
            .events
            .lock()
            .unwrap()
            .contains(&"register_advertisement")
        {
            assert!(
                std::time::Instant::now() < deadline,
                "server did not register its advertisement"
            );
            assert!(
                child.try_wait().unwrap().is_none(),
                "server exited before adapter removal"
            );
            thread::sleep(Duration::from_millis(10));
        }
        self.powered.store(false, Ordering::Relaxed);
        let output = child.wait_with_output().unwrap();
        assert!(
            output.status.success(),
            "stdout={} stderr={}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
        serde_json::from_slice(&output.stdout).unwrap()
    }

    fn spawn_cli(&self, duration: u16) -> Child {
        self.spawn_cli_for("hci0", duration)
    }

    fn spawn_cli_for(&self, adapter: &str, duration: u16) -> Child {
        Command::new(env!("CARGO_BIN_EXE_opencontinuity"))
            .args([
                "research",
                "watch-gatt",
                "serve",
                "--adapter",
                adapter,
                "--duration-seconds",
                &duration.to_string(),
                "--experimental",
                "--json",
            ])
            .env("DBUS_SYSTEM_BUS_ADDRESS", &self.address)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap()
    }

    fn terminate_cli(&self, signal: &str) -> serde_json::Value {
        let start_at = self.events.lock().unwrap().len();
        let mut child = self.spawn_cli(30);
        let deadline = std::time::Instant::now() + Duration::from_secs(5);
        while !self.events.lock().unwrap()[start_at..].contains(&"register_advertisement") {
            assert!(
                std::time::Instant::now() < deadline,
                "server did not register its advertisement"
            );
            assert!(
                child.try_wait().unwrap().is_none(),
                "server exited before signal test"
            );
            thread::sleep(Duration::from_millis(10));
        }
        let (destination, application) = self.app_info.lock().unwrap().clone().unwrap();
        let connection = Builder::address(self.address.as_str())
            .unwrap()
            .build()
            .unwrap();
        let proxy = Proxy::new(
            &connection,
            destination.as_str(),
            application.as_str(),
            "org.freedesktop.DBus.ObjectManager",
        )
        .unwrap();
        let objects: zbus::fdo::ManagedObjects = proxy.call("GetManagedObjects", &()).unwrap();
        let services = objects
            .values()
            .filter(|interfaces| {
                interfaces
                    .keys()
                    .any(|name| name.as_str() == "org.bluez.GattService1")
            })
            .count();
        let characteristics = objects
            .values()
            .filter(|interfaces| {
                interfaces
                    .keys()
                    .any(|name| name.as_str() == "org.bluez.GattCharacteristic1")
            })
            .count();
        assert_eq!(
            services, 1,
            "registered application exposes one GATT service"
        );
        assert_eq!(
            characteristics, 5,
            "registered application exposes five GATT characteristics"
        );
        let service_props = objects
            .values()
            .find_map(|interfaces| {
                interfaces
                    .iter()
                    .find(|(name, _)| name.as_str() == "org.bluez.GattService1")
            })
            .expect("service interface is exported")
            .1;
        for name in ["UUID", "Primary", "Characteristics"] {
            assert!(
                service_props.contains_key(name),
                "GATT service is missing property {name}"
            );
        }
        let characteristic_props = objects
            .values()
            .find_map(|interfaces| {
                interfaces
                    .iter()
                    .find(|(name, _)| name.as_str() == "org.bluez.GattCharacteristic1")
            })
            .expect("characteristic interface is exported")
            .1;
        for name in ["UUID", "Service", "Flags"] {
            assert!(
                characteristic_props.contains_key(name),
                "GATT characteristic is missing property {name}"
            );
        }
        let device = OwnedObjectPath::try_from("/org/bluez/hci0/dev_AA_BB_CC_DD_EE_FF").unwrap();
        let service_path = format!("{application}/service0");
        let characteristic_path = |index| format!("{service_path}/char{index}");
        let characteristic = |path: String| {
            Proxy::new_owned(
                connection.clone(),
                destination.clone(),
                OwnedObjectPath::try_from(path).unwrap(),
                "org.bluez.GattCharacteristic1".to_owned(),
            )
            .unwrap()
        };
        let read_options = |offset: Option<u16>| {
            let mut options = std::collections::HashMap::new();
            options.insert(
                "device".to_owned(),
                OwnedValue::try_from(Value::from(device.clone())).unwrap(),
            );
            options.insert(
                "mtu".to_owned(),
                OwnedValue::try_from(Value::from(23u16)).unwrap(),
            );
            if let Some(offset) = offset {
                options.insert(
                    "offset".to_owned(),
                    OwnedValue::try_from(Value::from(offset)).unwrap(),
                );
            }
            options
        };
        let version: Vec<u8> = characteristic(characteristic_path(0))
            .call("ReadValue", &read_options(None))
            .unwrap();
        assert_eq!(version, [1]);
        let mut challenge: Vec<u8> = characteristic(characteristic_path(1))
            .call("ReadValue", &read_options(None))
            .unwrap();
        challenge.extend::<Vec<u8>>(
            characteristic(characteristic_path(1))
                .call("ReadValue", &read_options(Some(22)))
                .unwrap(),
        );
        assert_eq!(challenge.len(), 25);
        let session_id: [u8; 8] = challenge[1..9].try_into().unwrap();
        let payload = b"synthetic gatt exchange";
        let chunks = payload.chunks(5).collect::<Vec<_>>();
        for (sequence, chunk) in chunks.iter().enumerate() {
            let frame = opencontinuity_core::gatt_wire::encode_frame(
                session_id,
                sequence as u8,
                sequence == 0,
                sequence + 1 == chunks.len(),
                if sequence == 0 { payload.len() } else { 0 },
                chunk,
            )
            .unwrap();
            characteristic(characteristic_path(2))
                .call::<_, _, ()>("WriteValue", &(frame, read_options(None)))
                .unwrap();
        }
        characteristic(characteristic_path(4))
            .call::<_, _, ()>(
                "WriteValue",
                &(
                    opencontinuity_core::gatt_wire::encode_control(
                        session_id,
                        opencontinuity_core::gatt_wire::ControlCommand::Complete,
                    )
                    .to_vec(),
                    read_options(None),
                ),
            )
            .unwrap();
        let mut response: Vec<u8> = characteristic(characteristic_path(3))
            .call("ReadValue", &read_options(None))
            .unwrap();
        response.extend::<Vec<u8>>(
            characteristic(characteristic_path(3))
                .call("ReadValue", &read_options(Some(22)))
                .unwrap(),
        );
        let terminator: Vec<u8> = characteristic(characteristic_path(3))
            .call("ReadValue", &read_options(Some(44)))
            .unwrap();
        assert!(terminator.is_empty());
        assert_eq!(response[0..2], [1, 0]);
        assert_eq!(response[2..10], session_id);
        assert_eq!(
            u16::from_le_bytes([response[10], response[11]]) as usize,
            payload.len()
        );
        let expected_digest = Sha256::digest(payload);
        assert_eq!(response[12..], expected_digest[..]);
        let (advertisement_sender, advertisement_path) = self
            .advertisement_info
            .lock()
            .unwrap()
            .clone()
            .expect("advertisement registration was requested");
        let advertisement_proxy = Proxy::new(
            &connection,
            advertisement_sender.as_str(),
            advertisement_path.as_str(),
            "org.freedesktop.DBus.Properties",
        )
        .unwrap();
        let advertisement_props: std::collections::HashMap<String, zbus::zvariant::OwnedValue> =
            advertisement_proxy
                .call("GetAll", &("org.bluez.LEAdvertisement1",))
                .unwrap();
        for name in ["Type", "ServiceUUIDs", "LocalName"] {
            assert!(
                advertisement_props.contains_key(name),
                "advertisement is missing property {name}"
            );
        }
        self.app_object_count
            .store(objects.len(), Ordering::Relaxed);
        let status = Command::new("kill")
            .args([format!("-{signal}"), child.id().to_string()])
            .status()
            .unwrap();
        assert!(status.success());
        let output = child.wait_with_output().unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        serde_json::from_slice(&output.stdout).unwrap()
    }
}

impl Drop for PrivateBluez {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        let _ = self.daemon.kill();
        let _ = self.daemon.wait();
    }
}

#[test]
fn real_zbus_registration_and_timeout_cleanup_use_only_owned_resources() {
    let bluez = PrivateBluez::start(FailureMode::None);
    let output = bluez.run_cli(1);
    assert!(
        output.status.success(),
        "stdout={} stderr={}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let report: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(report["shutdown_reason"], "timeout");
    assert_eq!(report["authorization"], "never_evaluated");
    assert_eq!(report["supported_advertising_instances"], 2);
    assert_eq!(
        *bluez.events.lock().unwrap(),
        [
            "register_application",
            "register_advertisement",
            "unregister_advertisement",
            "unregister_application"
        ]
    );
}

#[test]
fn explicit_adapter_selection_does_not_register_on_another_available_controller() {
    let bluez = PrivateBluez::start(FailureMode::None);
    let output = bluez.spawn_cli_for("hci1", 1).wait_with_output().unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stdout)
    );
    let report: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(report["adapter"], "hci1");
    assert!(bluez.events.lock().unwrap().is_empty());
    assert_eq!(
        *bluez.other_events.lock().unwrap(),
        [
            "register_application",
            "register_advertisement",
            "unregister_advertisement",
            "unregister_application"
        ]
    );
}

#[test]
fn advertisement_failure_rolls_back_only_the_registered_application() {
    let bluez = PrivateBluez::start(FailureMode::Advertisement);
    let output = bluez.run_cli(1);
    assert!(!output.status.success());
    let report: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(
        report["stage"], "registering_advertisement",
        "report={report}"
    );
    assert!(
        report["error"]
            .as_str()
            .unwrap()
            .contains("mock advertising slots exhausted")
    );
    assert_eq!(report["authorization"], "never_evaluated");
    assert_eq!(
        *bluez.events.lock().unwrap(),
        [
            "register_application",
            "register_advertisement",
            "unregister_application"
        ]
    );
}

#[test]
fn application_failure_does_not_attempt_to_unregister_unowned_resources() {
    let bluez = PrivateBluez::start(FailureMode::Application);
    let output = bluez.run_cli(1);
    assert!(!output.status.success());
    let report: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(
        report["stage"], "registering_application",
        "report={report}"
    );
    assert_eq!(report["authorization"], "never_evaluated");
    assert_eq!(*bluez.events.lock().unwrap(), ["register_application"]);
}

#[test]
fn missing_system_bus_bluez_service_and_adapter_have_distinct_failures() {
    let bus = PrivateBluez::start(FailureMode::NoBluez);
    let no_service = bus.run_cli(1);
    let report: serde_json::Value = serde_json::from_slice(&no_service.stdout).unwrap();
    assert_eq!(report["error_class"], "bluez_service_unavailable");

    let no_adapter = PrivateBluez::start(FailureMode::NoAdapter);
    let output = no_adapter.run_cli(1);
    let report: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(report["stage"], "preflight");
    assert!(
        report["error"]
            .as_str()
            .unwrap()
            .contains("adapter is absent")
    );

    for (mode, expected) in [
        (FailureMode::NoGatt, "GattManager1"),
        (FailureMode::NoAdvertisement, "LEAdvertisingManager1"),
    ] {
        let unsupported = PrivateBluez::start(mode);
        let output = unsupported.run_cli(1);
        let report: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
        assert_eq!(report["stage"], "preflight");
        assert!(report["error"].as_str().unwrap().contains(expected));
    }

    let no_bus = no_adapter.run_cli_at(1, "unix:path=/tmp/opencontinuity-p005-missing-bus");
    let report: serde_json::Value = serde_json::from_slice(&no_bus.stdout).unwrap();
    assert_eq!(report["error_class"], "system_bus_unavailable");
    assert_eq!(report["authorization"], "never_evaluated");
}

#[test]
fn sigint_and_sigterm_both_unregister_advertisement_before_application() {
    let bluez = PrivateBluez::start(FailureMode::None);
    let interrupt = bluez.terminate_cli("INT");
    assert_eq!(bluez.app_object_count.load(Ordering::Relaxed), 6);
    let terminate = bluez.terminate_cli("TERM");
    assert_eq!(interrupt["shutdown_reason"], "termination_signal");
    assert_eq!(terminate["shutdown_reason"], "termination_signal");
    assert_eq!(interrupt["authorization"], "never_evaluated");
    assert_eq!(terminate["authorization"], "never_evaluated");
    assert_eq!(
        *bluez.events.lock().unwrap(),
        [
            "register_application",
            "register_advertisement",
            "unregister_advertisement",
            "unregister_application",
            "register_application",
            "register_advertisement",
            "unregister_advertisement",
            "unregister_application"
        ]
    );
}

#[test]
fn controller_power_loss_stops_and_cleans_up_owned_resources() {
    let bluez = PrivateBluez::start(FailureMode::None);
    let report = bluez.remove_controller_during_run();
    assert_eq!(report["shutdown_reason"], "adapter_lost");
    assert_eq!(report["authorization"], "never_evaluated");
    assert_eq!(
        *bluez.events.lock().unwrap(),
        [
            "register_application",
            "register_advertisement",
            "unregister_advertisement",
            "unregister_application"
        ]
    );
}

#[test]
fn cleanup_treats_an_already_removed_advertisement_as_idempotent() {
    let bluez = PrivateBluez::start(FailureMode::MissingAdvertisementOnCleanup);
    let output = bluez.run_cli(1);
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stdout)
    );
    let report: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(report["shutdown_reason"], "timeout");
    assert_eq!(report["authorization"], "never_evaluated");
    assert_eq!(
        *bluez.events.lock().unwrap(),
        [
            "register_application",
            "register_advertisement",
            "unregister_advertisement",
            "unregister_application"
        ]
    );
}

#[test]
fn cleanup_failure_is_reported_and_application_cleanup_still_attempted() {
    let mock = PrivateBluez::start(FailureMode::CleanupFailure);
    let output = mock.run_cli(1);
    assert!(!output.status.success());
    let report: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(report["stage"], "stopping");
    assert_eq!(report["shutdown_reason"], "timeout");
    assert!(
        report["cleanup_errors"][0]
            .as_str()
            .unwrap()
            .contains("advertisement unregister failed")
    );
    assert!(report["advertisement_registered"].as_bool().unwrap());
    assert!(!report["application_registered"].as_bool().unwrap());
    assert_eq!(
        &*mock.events.lock().unwrap(),
        &[
            "register_application",
            "register_advertisement",
            "unregister_advertisement",
            "unregister_application"
        ]
    );
}

#[test]
fn rollback_failure_keeps_registration_failure_and_cleanup_details() {
    let mock = PrivateBluez::start(FailureMode::AdvertisementAndRollbackFailure);
    let output = mock.run_cli(2);
    assert!(!output.status.success());
    let report: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(report["stage"], "registering_advertisement");
    assert!(
        report["error"]
            .as_str()
            .unwrap()
            .contains("advertisement registration failed")
    );
    assert!(
        report["cleanup_errors"][0]
            .as_str()
            .unwrap()
            .contains("application unregister failed")
    );
}

#[test]
fn loss_of_bluez_owner_during_active_session_reports_failure_and_cleanup() {
    let mock = PrivateBluez::start(FailureMode::None);
    let mut child = mock.spawn_cli(30);
    let deadline = std::time::Instant::now() + Duration::from_secs(5);
    while !mock
        .events
        .lock()
        .unwrap()
        .contains(&"register_advertisement")
    {
        assert!(std::time::Instant::now() < deadline);
        assert!(child.try_wait().unwrap().is_none());
        thread::sleep(Duration::from_millis(10));
    }
    mock.stop.store(true, Ordering::Relaxed);
    let output = child.wait_with_output().unwrap();
    assert!(!output.status.success());
    let report: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(report["stage"], "active");
    assert!(report["error"].as_str().unwrap().contains("owner lost"));
    assert_eq!(report["application_registration_succeeded"], true);
    assert_eq!(report["advertisement_registration_succeeded"], true);
    assert!(!report["cleanup_errors"].as_array().unwrap().is_empty());
}

#[test]
fn registration_timeout_is_bounded_and_attempts_rollback() {
    let mock = PrivateBluez::start(FailureMode::SlowAdvertisement);
    let start = std::time::Instant::now();
    let output = mock.run_cli(10);
    assert!(start.elapsed() < Duration::from_secs(8));
    assert!(!output.status.success());
    let report: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(report["stage"], "registering_advertisement");
    assert!(report["error"].as_str().unwrap().contains("timed out"));
    let events = mock.events.lock().unwrap();
    assert!(events.contains(&"unregister_advertisement"));
    assert!(events.contains(&"unregister_application"));
}

#[test]
fn bus_loss_during_active_run_returns_original_error() {
    let mut mock = PrivateBluez::start(FailureMode::None);
    let mut child = mock.spawn_cli(30);
    let deadline = std::time::Instant::now() + Duration::from_secs(5);
    while !mock
        .events
        .lock()
        .unwrap()
        .contains(&"register_advertisement")
    {
        assert!(std::time::Instant::now() < deadline);
        assert!(child.try_wait().unwrap().is_none());
        thread::sleep(Duration::from_millis(10));
    }
    mock.daemon.kill().unwrap();
    let output = child.wait_with_output().unwrap();
    assert!(!output.status.success());
    let report: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(report["stage"], "active");
    assert!(report["error"].as_str().unwrap().contains("owner lost"));
    assert!(!report["cleanup_errors"].as_array().unwrap().is_empty());
    assert_eq!(report["authorization"], "never_evaluated");
}
