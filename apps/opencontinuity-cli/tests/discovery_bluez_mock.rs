#![cfg(target_os = "linux")]
//! Real zbus 5.12 calls on private dbus-daemon instances, never Bluetooth hardware.
use std::{
    collections::{HashMap, HashSet},
    io::{BufRead, BufReader},
    process::{Child, Command, Output, Stdio},
    sync::{Arc, Mutex},
    thread,
    time::{Duration, Instant},
};
use zbus::{
    blocking::{Connection, Proxy, connection::Builder},
    fdo::ObjectManager,
    message::Header,
    zvariant::{OwnedValue, Value},
};
const PATH: &str = "/org/bluez/hci0";
type Calls = Arc<Mutex<Vec<(String, String)>>>;
#[derive(Debug, zbus::DBusError)]
#[zbus(prefix = "org.bluez.Error")]
enum Error {
    NotAuthorized(String),
    Failed(String),
}
struct Adapter {
    mode: &'static str,
    calls: Calls,
    sessions: Arc<Mutex<HashSet<String>>>,
}
#[zbus::interface(name = "org.bluez.Adapter1")]
impl Adapter {
    #[zbus(property)]
    fn powered(&self) -> bool {
        self.mode != "off"
    }
    #[zbus(property)]
    fn discovering(&self) -> bool {
        !self.sessions.lock().unwrap().is_empty()
    }
    fn get_discovery_filters(&self) -> Vec<String> {
        if self.mode == "unsupported" {
            vec![]
        } else {
            vec!["Transport".into()]
        }
    }
    fn set_discovery_filter(
        &self,
        filter: HashMap<String, OwnedValue>,
        #[zbus(header)] header: Header<'_>,
    ) -> Result<(), Error> {
        let caller = header.sender().unwrap().to_string();
        if filter.is_empty() {
            self.calls.lock().unwrap().push(("clear".into(), caller));
            if self.mode == "clear_fail" {
                return Err(Error::Failed("private failure".into()));
            }
        } else {
            assert_eq!(filter.len(), 1);
            assert_eq!(<&str>::try_from(&filter["Transport"]).unwrap(), "le");
            self.calls
                .lock()
                .unwrap()
                .push(("filter_le".into(), caller));
        }
        Ok(())
    }
    fn start_discovery(&self, #[zbus(header)] header: Header<'_>) -> Result<(), Error> {
        let caller = header.sender().unwrap().to_string();
        self.calls
            .lock()
            .unwrap()
            .push(("start".into(), caller.clone()));
        if self.mode == "denied" {
            return Err(Error::NotAuthorized("AA:BB:CC:DD:EE:FF /private".into()));
        }
        if self.mode == "slow_start" {
            thread::sleep(Duration::from_millis(2300));
        }
        self.sessions.lock().unwrap().insert(caller);
        Ok(())
    }
    fn stop_discovery(&self, #[zbus(header)] header: Header<'_>) -> Result<(), Error> {
        let caller = header.sender().unwrap().to_string();
        self.calls
            .lock()
            .unwrap()
            .push(("stop".into(), caller.clone()));
        if self.mode == "stop_fail" {
            return Err(Error::Failed("private cleanup failure".into()));
        }
        assert!(
            self.sessions.lock().unwrap().remove(&caller),
            "must own a session before StopDiscovery"
        );
        Ok(())
    }
}
struct Cached;
#[zbus::interface(name = "org.bluez.Device1")]
impl Cached {
    #[zbus(property)]
    fn name(&self) -> &str {
        "Cached object is not a sighting"
    }
    #[zbus(property)]
    fn rssi(&self) -> i16 {
        -50
    }
}
struct Bus {
    daemon: Child,
    service: Connection,
    address: String,
    calls: Calls,
    sessions: Arc<Mutex<HashSet<String>>>,
}
impl Bus {
    fn new(mode: &'static str) -> Self {
        let mut daemon = Command::new("dbus-daemon")
            .args(["--session", "--nofork", "--print-address=1"])
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .unwrap();
        let mut address = String::new();
        BufReader::new(daemon.stdout.take().unwrap())
            .read_line(&mut address)
            .unwrap();
        let address = address.trim().to_owned();
        let calls = Calls::default();
        let sessions = Arc::new(Mutex::new(HashSet::new()));
        let service = Builder::address(address.as_str())
            .unwrap()
            .name("org.bluez")
            .unwrap()
            .serve_at("/", ObjectManager)
            .unwrap()
            .build()
            .unwrap();
        if mode != "no_adapter" {
            service
                .object_server()
                .at(
                    PATH,
                    Adapter {
                        mode,
                        calls: calls.clone(),
                        sessions: sessions.clone(),
                    },
                )
                .unwrap();
            service
                .object_server()
                .at(format!("{PATH}/dev_CACHED"), Cached)
                .unwrap();
        }
        Self {
            daemon,
            service,
            address,
            calls,
            sessions,
        }
    }
    fn spawn(&self, seconds: &str) -> Child {
        Command::new(env!("CARGO_BIN_EXE_opencontinuity"))
            .args(["devices", "discover", "--seconds", seconds, "--json"])
            .env("DBUS_SYSTEM_BUS_ADDRESS", &self.address)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap()
    }
    fn wait_started(&self, child: &mut Child) {
        let end = Instant::now() + Duration::from_secs(5);
        while !self
            .calls
            .lock()
            .unwrap()
            .iter()
            .any(|(call, _)| call == "start")
        {
            assert!(
                child.try_wait().unwrap().is_none(),
                "CLI exited before start"
            );
            assert!(Instant::now() < end);
            thread::sleep(Duration::from_millis(5));
        }
        // The test emits after the StartDiscovery method reply, as BlueZ events do.
        thread::sleep(Duration::from_millis(40));
    }
    fn event(&self, path: &str, iface: &str, name: Option<&str>, activity: bool) {
        let mut props = HashMap::<&str, Value<'_>>::new();
        if let Some(name) = name {
            props.insert("Name", Value::from(name));
        }
        if activity {
            props.insert("RSSI", Value::from(-40i16));
        }
        self.service
            .emit_signal(
                None::<&str>,
                path,
                "org.freedesktop.DBus.Properties",
                "PropertiesChanged",
                &(iface, props, Vec::<String>::new()),
            )
            .unwrap();
    }
    fn names(&self) -> Vec<String> {
        self.calls
            .lock()
            .unwrap()
            .iter()
            .map(|(a, _)| a.clone())
            .collect()
    }
}
impl Drop for Bus {
    fn drop(&mut self) {
        let _ = self.daemon.kill();
        let _ = self.daemon.wait();
    }
}
fn report(output: Output, expected: &str) -> serde_json::Value {
    assert!(
        output.stderr.is_empty(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let text = String::from_utf8(output.stdout).unwrap();
    let value: serde_json::Value = serde_json::from_str(&text).unwrap();
    assert_eq!(value["status"], expected, "{text}");
    assert_eq!(
        output.status.code(),
        Some(if expected == "completed" { 0 } else { 1 })
    );
    assert!(!text.contains("/org/bluez/"));
    assert!(!text.contains("AA:BB:CC:DD:EE:FF"));
    assert!(!text.contains("rssi"));
    assert_eq!(value["authorization"], "never_evaluated");
    value
}
#[test]
fn owned_le_session_filters_events_deduplicates_and_ignores_cached_objects() {
    let bus = Bus::new("normal");
    let mut child = bus.spawn("1");
    bus.wait_started(&mut child);
    bus.event(
        "/org/bluez/hci1/dev_OTHER",
        "org.bluez.Device1",
        Some("Wrong adapter"),
        true,
    );
    bus.event(
        "/org/bluez/hci0/dev_LABEL",
        "org.bluez.Device1",
        Some("Only a name change"),
        false,
    );
    bus.event(
        "/org/bluez/hci0/dev_WRONG",
        "org.bluez.Battery1",
        Some("Wrong interface"),
        true,
    );
    for _ in 0..3 {
        bus.event(
            "/org/bluez/hci0/dev_OBSERVED",
            "org.bluez.Device1",
            Some("Watch\n\u{202e}"),
            true,
        );
    }
    let r = report(child.wait_with_output().unwrap(), "completed");
    assert_eq!(r["scan_started"], true);
    assert_eq!(r["scan_completed"], true);
    assert_eq!(r["devices"].as_array().unwrap().len(), 1);
    assert_eq!(r["devices"][0]["display_name"], "Watch");
    assert!(r["actual_duration_ms"].as_u64().unwrap() >= 1000);
    assert_eq!(bus.names(), ["filter_le", "start", "stop", "clear"]);
}
#[test]
fn cached_only_is_completed_with_no_observations() {
    let bus = Bus::new("normal");
    let r = report(bus.spawn("1").wait_with_output().unwrap(), "completed");
    assert!(r["devices"].as_array().unwrap().is_empty());
}
#[test]
fn preflight_failures_never_start_or_stop() {
    for (mode, status) in [
        ("no_adapter", "no_adapter"),
        ("off", "adapter_not_powered"),
        ("unsupported", "unsupported_le"),
    ] {
        let bus = Bus::new(mode);
        let r = report(bus.spawn("1").wait_with_output().unwrap(), status);
        assert_eq!(r["scan_started"], false);
        assert!(bus.names().is_empty());
    }
}
#[test]
fn denied_start_clears_only_its_filter_and_never_stops() {
    let bus = Bus::new("denied");
    let r = report(bus.spawn("1").wait_with_output().unwrap(), "access_denied");
    assert_eq!(r["cleanup_status"], "complete");
    assert_eq!(bus.names(), ["filter_le", "start", "clear"]);
}
#[test]
fn sigint_and_sigterm_cleanup_owned_session_once() {
    for signal in [libc::SIGINT, libc::SIGTERM] {
        let bus = Bus::new("normal");
        let mut child = bus.spawn("30");
        bus.wait_started(&mut child);
        // SAFETY: kill sends a standard termination signal to this test's live child process.
        unsafe {
            libc::kill(child.id() as i32, signal);
        }
        let r = report(child.wait_with_output().unwrap(), "cancelled");
        assert_eq!(r["cleanup_status"], "complete");
        assert_eq!(bus.names(), ["filter_le", "start", "stop", "clear"]);
    }
}
#[test]
fn cancellation_during_uncertain_start_disconnects_without_stop() {
    let bus = Bus::new("slow_start");
    let mut child = bus.spawn("1");
    bus.wait_started(&mut child);
    // SAFETY: child is live and owned by this test.
    unsafe {
        libc::kill(child.id() as i32, libc::SIGTERM);
    }
    let r = report(child.wait_with_output().unwrap(), "cancelled");
    assert_eq!(r["scan_started"], false);
    assert_eq!(r["cleanup_status"], "incomplete");
    assert!(!bus.names().contains(&"stop".into()));
}
#[test]
fn cleanup_failures_are_explicit_and_clear_is_still_attempted() {
    for mode in ["stop_fail", "clear_fail"] {
        let bus = Bus::new(mode);
        let r = report(
            bus.spawn("1").wait_with_output().unwrap(),
            "cleanup_incomplete",
        );
        assert_eq!(r["cleanup_status"], "incomplete");
        assert_eq!(r["scan_completed"], false);
        assert_eq!(bus.names(), ["filter_le", "start", "stop", "clear"]);
    }
}
#[test]
fn other_clients_sessions_remain_owned_and_running() {
    let bus = Bus::new("normal");
    let other = Builder::address(bus.address.as_str())
        .unwrap()
        .build()
        .unwrap();
    let proxy = Proxy::new(&other, "org.bluez", PATH, "org.bluez.Adapter1").unwrap();
    proxy.call::<_, _, ()>("StartDiscovery", &()).unwrap();
    report(bus.spawn("1").wait_with_output().unwrap(), "completed");
    assert!(
        bus.sessions
            .lock()
            .unwrap()
            .contains(other.unique_name().unwrap().as_str())
    );
    assert_eq!(bus.sessions.lock().unwrap().len(), 1);
    proxy.call::<_, _, ()>("StopDiscovery", &()).unwrap();
}
#[test]
fn bluez_owner_loss_is_detected_without_touching_replacement() {
    let bus = Bus::new("normal");
    let mut child = bus.spawn("30");
    bus.wait_started(&mut child);
    bus.service.release_name("org.bluez").unwrap();
    let replacement = Builder::address(bus.address.as_str())
        .unwrap()
        .name("org.bluez")
        .unwrap()
        .build()
        .unwrap();
    let r = report(child.wait_with_output().unwrap(), "bluez_unavailable");
    assert_eq!(r["cleanup_status"], "complete");
    assert_ne!(replacement.unique_name(), bus.service.unique_name());
    assert_eq!(bus.names(), ["filter_le", "start", "stop", "clear"]);
}
#[test]
fn adapter_removal_cleans_up_and_reports_unavailable() {
    let bus = Bus::new("normal");
    let mut child = bus.spawn("30");
    bus.wait_started(&mut child);
    bus.service
        .object_server()
        .remove::<Adapter, _>(PATH)
        .unwrap();
    let r = report(child.wait_with_output().unwrap(), "no_adapter");
    assert_eq!(r["cleanup_status"], "incomplete");
}
#[test]
fn record_cap_stops_scan_and_releases_session() {
    let bus = Bus::new("normal");
    let mut child = bus.spawn("30");
    bus.wait_started(&mut child);
    for i in 0..129 {
        bus.event(
            &format!("{PATH}/dev_{i}"),
            "org.bluez.Device1",
            Some("AA:BB:CC:DD:EE:FF"),
            true,
        );
    }
    let r = report(child.wait_with_output().unwrap(), "data_limit");
    assert_eq!(r["devices"].as_array().unwrap().len(), 128);
    assert_eq!(r["cleanup_status"], "complete");
}
#[test]
fn method_timeout_reports_uncertain_start_and_never_sends_unowned_stop() {
    let bus = Bus::new("slow_start");
    let began = Instant::now();
    let r = report(bus.spawn("1").wait_with_output().unwrap(), "timeout");
    assert!(began.elapsed() < Duration::from_secs(6));
    assert_eq!(r["cleanup_status"], "incomplete");
    assert!(!bus.names().contains(&"stop".into()));
}
#[test]
fn cooperative_scan_collision_is_busy_without_mutations() {
    let bus = Bus::new("normal");
    let mut first = bus.spawn("30");
    bus.wait_started(&mut first);
    report(bus.spawn("1").wait_with_output().unwrap(), "adapter_busy");
    // SAFETY: first is this test's live child.
    unsafe {
        libc::kill(first.id() as i32, libc::SIGTERM);
    }
    report(first.wait_with_output().unwrap(), "cancelled");
    assert_eq!(bus.names(), ["filter_le", "start", "stop", "clear"]);
}

#[test]
fn interfaces_added_is_an_event_but_an_unrelated_sender_is_ignored() {
    use zbus::zvariant::OwnedObjectPath;
    let bus = Bus::new("normal");
    let mut child = bus.spawn("1");
    bus.wait_started(&mut child);
    let impostor = Builder::address(bus.address.as_str())
        .unwrap()
        .build()
        .unwrap();
    impostor
        .emit_signal(
            None::<&str>,
            "/org/bluez/hci0/dev_FAKE",
            "org.freedesktop.DBus.Properties",
            "PropertiesChanged",
            &(
                "org.bluez.Device1",
                HashMap::from([("RSSI", Value::from(-20i16))]),
                Vec::<String>::new(),
            ),
        )
        .unwrap();
    let path = OwnedObjectPath::try_from("/org/bluez/hci0/dev_NEW").unwrap();
    bus.service
        .emit_signal(
            None::<&str>,
            "/",
            "org.freedesktop.DBus.ObjectManager",
            "InterfacesAdded",
            &(
                path,
                HashMap::from([(
                    "org.bluez.Device1",
                    HashMap::from([
                        ("RSSI", Value::from(-50i16)),
                        ("Name", Value::from("Fresh event")),
                    ]),
                )]),
            ),
        )
        .unwrap();
    let r = report(child.wait_with_output().unwrap(), "completed");
    assert_eq!(r["devices"].as_array().unwrap().len(), 1);
    assert_eq!(r["devices"][0]["display_name"], "Fresh event");
}
#[test]
fn adapter_power_loss_cancels_without_repowering() {
    let bus = Bus::new("normal");
    let mut child = bus.spawn("30");
    bus.wait_started(&mut child);
    bus.service
        .emit_signal(
            None::<&str>,
            PATH,
            "org.freedesktop.DBus.Properties",
            "PropertiesChanged",
            &(
                "org.bluez.Adapter1",
                HashMap::from([("Powered", Value::from(false))]),
                Vec::<String>::new(),
            ),
        )
        .unwrap();
    let r = report(child.wait_with_output().unwrap(), "adapter_not_powered");
    assert_eq!(r["cleanup_status"], "complete");
    assert_eq!(bus.names(), ["filter_le", "start", "stop", "clear"]);
}
#[test]
fn event_count_cap_bounds_duplicate_floods() {
    let bus = Bus::new("normal");
    let mut child = bus.spawn("30");
    bus.wait_started(&mut child);
    for _ in 0..4100 {
        bus.event(
            "/org/bluez/hci0/dev_FLOOD",
            "org.bluez.Device1",
            Some("Repeated"),
            true,
        );
    }
    let r = report(child.wait_with_output().unwrap(), "data_limit");
    assert_eq!(r["devices"].as_array().unwrap().len(), 1);
    assert_eq!(r["cleanup_status"], "complete");
}
#[test]
fn bus_loss_is_reported_and_cleanup_is_uncertain() {
    let mut bus = Bus::new("normal");
    let mut child = bus.spawn("30");
    bus.wait_started(&mut child);
    bus.daemon.kill().unwrap();
    bus.daemon.wait().unwrap();
    let r = report(child.wait_with_output().unwrap(), "bluez_unavailable");
    assert_eq!(r["cleanup_status"], "incomplete");
}
#[test]
fn each_session_has_different_ephemeral_ids() {
    let bus = Bus::new("normal");
    let mut ids = vec![];
    for _ in 0..2 {
        bus.calls.lock().unwrap().clear();
        let mut child = bus.spawn("1");
        bus.wait_started(&mut child);
        bus.event(
            "/org/bluez/hci0/dev_SAME",
            "org.bluez.Device1",
            Some("Peripheral"),
            true,
        );
        let r = report(child.wait_with_output().unwrap(), "completed");
        ids.push(r["devices"][0]["id"].as_str().unwrap().to_owned());
    }
    assert_ne!(ids[0], ids[1]);
}

#[test]
fn malformed_activity_fails_closed_then_cleans_up() {
    let bus = Bus::new("normal");
    let mut child = bus.spawn("30");
    bus.wait_started(&mut child);
    bus.service
        .emit_signal(
            None::<&str>,
            "/org/bluez/hci0/dev_BAD",
            "org.freedesktop.DBus.Properties",
            "PropertiesChanged",
            &(
                "org.bluez.Device1",
                HashMap::from([("RSSI", Value::from("not an integer"))]),
                Vec::<String>::new(),
            ),
        )
        .unwrap();
    let r = report(child.wait_with_output().unwrap(), "malformed_data");
    assert!(r["devices"].as_array().unwrap().is_empty());
    assert_eq!(r["cleanup_status"], "complete");
}
