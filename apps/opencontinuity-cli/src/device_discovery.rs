//! Explicit bounded discovery, independent from the read-only inventory.
use serde::Serialize;
use std::{
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    time::{Duration, Instant},
};

pub const MAX_DEVICES: usize = 128;
pub const MAX_EVENTS: usize = 4096;
pub const SETUP_TIMEOUT: Duration = Duration::from_secs(5);
pub const CALL_TIMEOUT: Duration = Duration::from_secs(2);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Status {
    Completed,
    Cancelled,
    NoAdapter,
    AdapterNotPowered,
    UnsupportedLe,
    AdapterBusy,
    BluezUnavailable,
    SystemBusUnavailable,
    AccessDenied,
    Timeout,
    MalformedData,
    CleanupIncomplete,
    DataLimit,
    #[cfg(not(target_os = "linux"))]
    UnsupportedPlatform,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Cleanup {
    NotNeeded,
    Complete,
    Incomplete,
}
#[derive(Debug, Serialize)]
pub struct ObservedDevice {
    pub id: String,
    pub display_name: String,
    pub provenance: &'static str,
}
#[derive(Debug, Serialize)]
pub struct Report {
    pub schema_version: u8,
    pub source: &'static str,
    pub scan_requested: bool,
    pub scan_started: bool,
    pub scan_completed: bool,
    pub requested_duration_seconds: u16,
    pub actual_duration_ms: u64,
    pub adapter_index: Option<usize>,
    pub status: Status,
    pub cleanup_status: Cleanup,
    pub devices: Vec<ObservedDevice>,
    pub authorization: &'static str,
}
impl Report {
    fn new(options: Options) -> Self {
        Self {
            schema_version: 1,
            source: "bluez_le_discovery_session",
            scan_requested: true,
            scan_started: false,
            scan_completed: false,
            requested_duration_seconds: options.seconds,
            actual_duration_ms: 0,
            adapter_index: None,
            status: Status::MalformedData,
            cleanup_status: Cleanup::NotNeeded,
            devices: vec![],
            authorization: "never_evaluated",
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Options {
    pub seconds: u16,
    pub adapter_index: usize,
}
impl Options {
    pub fn parse(args: &[String]) -> Result<Self, ()> {
        let mut seconds = None;
        let mut adapter = None;
        let mut json = false;
        let mut args = args.iter();
        while let Some(arg) = args.next() {
            match arg.as_str() {
                "--seconds" if seconds.is_none() => seconds = Some(number(args.next().ok_or(())?)?),
                "--adapter-index" if adapter.is_none() => {
                    adapter = Some(number(args.next().ok_or(())?)?)
                }
                "--json" if !json => json = true,
                _ => return Err(()),
            }
        }
        let seconds = seconds.unwrap_or(10);
        let adapter = adapter.unwrap_or(1);
        if !json || !(1..=30).contains(&seconds) || !(1..=16).contains(&adapter) {
            return Err(());
        }
        Ok(Self {
            seconds,
            adapter_index: usize::from(adapter),
        })
    }
}
fn number(text: &str) -> Result<u16, ()> {
    if text.is_empty() || !text.bytes().all(|b| b.is_ascii_digit()) {
        return Err(());
    }
    text.parse().map_err(|_| ())
}

pub fn run(options: Options) -> Report {
    let cancelled = Arc::new(AtomicBool::new(false));
    let mut handlers = Vec::new();
    for signal in [signal_hook::consts::SIGINT, signal_hook::consts::SIGTERM] {
        match signal_hook::flag::register(signal, cancelled.clone()) {
            Ok(id) => handlers.push(id),
            Err(_) => {
                for id in handlers {
                    signal_hook::low_level::unregister(id);
                }
                return Report::new(options);
            }
        }
    }
    #[cfg(target_os = "linux")]
    let report = futures_lite::future::block_on(linux::run(options, cancelled));
    #[cfg(not(target_os = "linux"))]
    let report = {
        let mut r = Report::new(options);
        r.status = Status::UnsupportedPlatform;
        r
    };
    for id in handlers {
        signal_hook::low_level::unregister(id);
    }
    report
}

fn private_name(name: Option<String>) -> String {
    let name = crate::device_inventory::sanitize_name(name);
    let bytes = name.as_bytes();
    let address = bytes.windows(17).any(|w| {
        [2, 5, 8, 11, 14]
            .iter()
            .all(|i| matches!(w[*i], b':' | b'-' | b'_'))
            && w.iter()
                .enumerate()
                .all(|(i, b)| [2, 5, 8, 11, 14].contains(&i) || b.is_ascii_hexdigit())
    });
    if address || name.contains("/org/bluez/") {
        "Unnamed Bluetooth device".into()
    } else {
        name
    }
}

#[cfg(target_os = "linux")]
mod linux {
    use super::*;
    use futures_lite::{FutureExt, StreamExt};
    use std::collections::HashMap;
    use zbus::{
        Connection, MatchRule, Message, MessageStream,
        message::{Sequence, Type},
        zvariant::{OwnedObjectPath, OwnedValue, Value},
    };
    type Props = HashMap<String, OwnedValue>;
    type Interfaces = HashMap<String, Props>;
    type Objects = HashMap<OwnedObjectPath, Interfaces>;
    const ADAPTER: &str = "org.bluez.Adapter1";
    const DEVICE: &str = "org.bluez.Device1";
    const OM: &str = "org.freedesktop.DBus.ObjectManager";
    const PROPERTIES: &str = "org.freedesktop.DBus.Properties";

    pub(super) fn classify(error: zbus::Error) -> Status {
        if let zbus::Error::InputOutput(io) = &error {
            if io.kind() == std::io::ErrorKind::TimedOut {
                return Status::Timeout;
            }
        }
        let name = match &error {
            zbus::Error::MethodError(name, _, _) => name.as_str(),
            zbus::Error::FDO(e) => match e.as_ref() {
                zbus::fdo::Error::AccessDenied(_) => return Status::AccessDenied,
                zbus::fdo::Error::ServiceUnknown(_) | zbus::fdo::Error::NameHasNoOwner(_) => {
                    return Status::BluezUnavailable;
                }
                zbus::fdo::Error::NoReply(_) | zbus::fdo::Error::Timeout(_) => {
                    return Status::Timeout;
                }
                _ => return Status::MalformedData,
            },
            zbus::Error::InputOutput(_) | zbus::Error::Address(_) | zbus::Error::Handshake(_) => {
                return Status::SystemBusUnavailable;
            }
            _ => return Status::MalformedData,
        };
        match name {
            "org.bluez.Error.NotAuthorized"
            | "org.bluez.Error.NotPermitted"
            | "org.freedesktop.DBus.Error.AccessDenied" => Status::AccessDenied,
            "org.bluez.Error.NotSupported" | "org.freedesktop.DBus.Error.UnknownMethod" => {
                Status::UnsupportedLe
            }
            "org.bluez.Error.InProgress" => Status::AdapterBusy,
            "org.bluez.Error.NotReady" => Status::AdapterNotPowered,
            "org.freedesktop.DBus.Error.ServiceUnknown"
            | "org.freedesktop.DBus.Error.NameHasNoOwner" => Status::BluezUnavailable,
            "org.freedesktop.DBus.Error.NoReply" | "org.freedesktop.DBus.Error.Timeout" => {
                Status::Timeout
            }
            _ => Status::MalformedData,
        }
    }
    async fn bounded<T>(
        duration: Duration,
        operation: impl std::future::Future<Output = Result<T, zbus::Error>>,
    ) -> Result<T, Status> {
        async {
            async_io::Timer::after(duration).await;
            Err(Status::Timeout)
        }
        .or(async { operation.await.map_err(classify) })
        .await
    }
    async fn interrupted(cancel: &AtomicBool) -> Status {
        loop {
            if cancel.load(Ordering::Relaxed) {
                return Status::Cancelled;
            }
            async_io::Timer::after(Duration::from_millis(20)).await;
        }
    }
    #[derive(Default)]
    struct Ownership {
        filter_attempted: bool,
        start_attempted: bool,
        started: bool,
    }
    struct Session {
        connection: Connection,
        owner: String,
        path: String,
        signals: MessageStream,
        owners: MessageStream,
        ownership: Ownership,
        // Held through cleanup. This excludes only cooperative same-UID experiments.
        _guard: Option<std::os::unix::net::UnixListener>,
    }
    async fn owner(connection: &Connection) -> Result<String, Status> {
        let reply = connection
            .call_method(
                Some("org.freedesktop.DBus"),
                "/org/freedesktop/DBus",
                Some("org.freedesktop.DBus"),
                "GetNameOwner",
                &("org.bluez",),
            )
            .await
            .map_err(classify)?;
        reply.body().deserialize().map_err(classify)
    }
    async fn connect() -> Result<Session, Status> {
        let connection = zbus::connection::Builder::system()
            .map_err(classify)?
            .method_timeout(CALL_TIMEOUT)
            .max_queued(64)
            .build()
            .await
            .map_err(classify)?;
        let owner = owner(&connection).await?;
        let owners = MessageStream::for_match_rule(
            MatchRule::builder()
                .msg_type(Type::Signal)
                .sender("org.freedesktop.DBus")
                .map_err(classify)?
                .interface("org.freedesktop.DBus")
                .map_err(classify)?
                .member("NameOwnerChanged")
                .map_err(classify)?
                .add_arg("org.bluez")
                .map_err(classify)?
                .build(),
            &connection,
            Some(64),
        )
        .await
        .map_err(classify)?;
        let signals = MessageStream::for_match_rule(
            MatchRule::builder()
                .msg_type(Type::Signal)
                .sender(owner.as_str())
                .map_err(classify)?
                .build(),
            &connection,
            Some(64),
        )
        .await
        .map_err(classify)?;
        Ok(Session {
            connection,
            owner,
            path: String::new(),
            signals,
            owners,
            ownership: Ownership::default(),
            _guard: None,
        })
    }
    impl Session {
        async fn call<B>(&self, method: &str, body: &B) -> Result<Message, Status>
        where
            B: serde::Serialize + zbus::zvariant::DynamicType,
        {
            self.connection
                .call_method(
                    Some(self.owner.as_str()),
                    self.path.as_str(),
                    Some(ADAPTER),
                    method,
                    body,
                )
                .await
                .map_err(classify)
        }
        async fn setup(
            &mut self,
            options: Options,
            report: &mut Report,
        ) -> Result<Sequence, Status> {
            let reply = self
                .connection
                .call_method(
                    Some(self.owner.as_str()),
                    "/",
                    Some(OM),
                    "GetManagedObjects",
                    &(),
                )
                .await
                .map_err(classify)?;
            if reply.body().len() > 1024 * 1024 {
                return Err(Status::DataLimit);
            }
            let objects: Objects = reply.body().deserialize().map_err(classify)?;
            if objects.len() > 1024 {
                return Err(Status::DataLimit);
            }
            let mut adapters: Vec<_> = objects
                .iter()
                .filter(|(_, i)| i.contains_key(ADAPTER))
                .collect();
            adapters.sort_by(|(a, _), (b, _)| a.as_str().cmp(b.as_str()));
            if adapters.len() > 16 {
                return Err(Status::DataLimit);
            }
            let Some((path, interfaces)) = adapters.get(options.adapter_index - 1) else {
                return Err(Status::NoAdapter);
            };
            self.path = path.to_string();
            report.adapter_index = Some(options.adapter_index);
            match interfaces[ADAPTER]
                .get("Powered")
                .and_then(|v| bool::try_from(v).ok())
            {
                Some(true) => {}
                Some(false) => return Err(Status::AdapterNotPowered),
                None => return Err(Status::MalformedData),
            }
            self._guard =
                Some(crate::radio_guard::acquire(&self.path).map_err(|_| Status::AdapterBusy)?);
            // Never infer ownership from Adapter1.Discovering (shared across clients).
            let reply = self.call("GetDiscoveryFilters", &()).await?;
            if reply.body().len() > 4096 {
                return Err(Status::DataLimit);
            }
            let filters: Vec<String> = reply.body().deserialize().map_err(classify)?;
            if filters.len() > 32 {
                return Err(Status::DataLimit);
            }
            if !filters.iter().any(|f| f == "Transport") {
                return Err(Status::UnsupportedLe);
            }
            if owner(&self.connection).await? != self.owner {
                return Err(Status::BluezUnavailable);
            }
            self.ownership.filter_attempted = true;
            self.call(
                "SetDiscoveryFilter",
                &(HashMap::from([("Transport", Value::from("le"))]),),
            )
            .await?
            .body()
            .deserialize::<()>()
            .map_err(classify)?;
            self.ownership.start_attempted = true;
            let reply = match self.call("StartDiscovery", &()).await {
                Ok(reply) => reply,
                Err(status) => {
                    if matches!(
                        status,
                        Status::AccessDenied
                            | Status::AdapterNotPowered
                            | Status::AdapterBusy
                            | Status::UnsupportedLe
                    ) {
                        self.ownership.start_attempted = false;
                    }
                    return Err(status);
                }
            };
            reply.body().deserialize::<()>().map_err(classify)?;
            self.ownership.started = true;
            report.scan_started = true;
            Ok(reply.recv_position())
        }
        async fn collect(&mut self, after: Sequence, report: &mut Report) -> Status {
            let mut seen = HashMap::<String, usize>::new();
            let mut random = [0u8; 8];
            if getrandom::getrandom(&mut random).is_err() {
                return Status::MalformedData;
            }
            let mut prefix = String::with_capacity(16);
            for byte in random {
                use std::fmt::Write;
                write!(&mut prefix, "{byte:02x}").expect("String write cannot fail");
            }
            for _ in 0..MAX_EVENTS {
                let event = async { (true, self.owners.next().await) }
                    .or(async { (false, self.signals.next().await) })
                    .await;
                let Some(Ok(message)) = event.1 else {
                    return Status::BluezUnavailable;
                };
                if message.body().len() > 64 * 1024 {
                    return Status::DataLimit;
                }
                if event.0 {
                    return Status::BluezUnavailable;
                }
                // Subscriptions predate StartDiscovery: discard queued setup/cache activity.
                if message.recv_position() <= after {
                    continue;
                }
                match observation(&message, &self.path) {
                    Ok(Some((path, name))) => {
                        if let Some(index) = seen.get(&path) {
                            if name.is_some() {
                                report.devices[*index].display_name = private_name(name);
                            }
                        } else {
                            if seen.len() >= MAX_DEVICES {
                                return Status::DataLimit;
                            }
                            seen.insert(path, report.devices.len());
                            report.devices.push(ObservedDevice {
                                id: format!("scan-{prefix}-{:03}", seen.len()),
                                display_name: private_name(name),
                                provenance: "bluez_event_during_session",
                            });
                        }
                    }
                    Ok(None) => {}
                    Err(status) => return status,
                }
            }
            Status::DataLimit
        }
        async fn cleanup(&self) -> Cleanup {
            if !self.ownership.filter_attempted {
                return Cleanup::NotNeeded;
            }
            let mut ok = true;
            if self.ownership.started {
                ok &= bounded(CALL_TIMEOUT, async {
                    self.connection
                        .call_method(
                            Some(self.owner.as_str()),
                            self.path.as_str(),
                            Some(ADAPTER),
                            "StopDiscovery",
                            &(),
                        )
                        .await
                })
                .await
                .and_then(|reply| reply.body().deserialize::<()>().map_err(classify))
                .is_ok();
            } else if self.ownership.start_attempted {
                // Start may have taken effect despite a lost reply; disconnect is the fallback.
                // Never send StopDiscovery without a confirmed acquisition.
                ok = false;
            }
            ok &= bounded(CALL_TIMEOUT, async {
                self.connection
                    .call_method(
                        Some(self.owner.as_str()),
                        self.path.as_str(),
                        Some(ADAPTER),
                        "SetDiscoveryFilter",
                        &(HashMap::<&str, Value<'_>>::new(),),
                    )
                    .await
            })
            .await
            .and_then(|reply| reply.body().deserialize::<()>().map_err(classify))
            .is_ok();
            if ok {
                Cleanup::Complete
            } else {
                Cleanup::Incomplete
            }
        }
    }
    fn name(props: &Props) -> Result<Option<String>, Status> {
        let value = props.get("Name").or_else(|| props.get("Alias"));
        value
            .map(|v| {
                <&str>::try_from(v)
                    .map(|s| s.to_owned())
                    .map_err(|_| Status::MalformedData)
            })
            .transpose()
            .and_then(|v| {
                if v.as_ref().is_some_and(|s| s.len() > 4096) {
                    Err(Status::DataLimit)
                } else {
                    Ok(v)
                }
            })
    }
    fn activity(props: &Props) -> Result<bool, Status> {
        let mut observed = false;
        for (key, signature) in [
            ("RSSI", "n"),
            ("TxPower", "n"),
            ("ManufacturerData", "a{qv}"),
            ("ServiceData", "a{sv}"),
            ("AdvertisingData", "a{yv}"),
        ] {
            if let Some(value) = props.get(key) {
                if value.value_signature().to_string() != signature {
                    return Err(Status::MalformedData);
                }
                observed = true;
            }
        }
        Ok(observed)
    }
    fn observation(
        message: &Message,
        adapter: &str,
    ) -> Result<Option<(String, Option<String>)>, Status> {
        let header = message.header();
        let interface = header.interface().map(|v| v.as_str());
        let member = header.member().map(|v| v.as_str());
        if interface == Some(OM) && member == Some("InterfacesRemoved") {
            let (path, removed): (OwnedObjectPath, Vec<String>) =
                message.body().deserialize().map_err(classify)?;
            if path.as_str() == adapter && removed.iter().any(|i| i == ADAPTER) {
                return Err(Status::NoAdapter);
            }
        }
        if interface == Some(OM) && member == Some("InterfacesAdded") {
            let (path, interfaces): (OwnedObjectPath, Interfaces) =
                message.body().deserialize().map_err(classify)?;
            if let Some(props) = interfaces.get(DEVICE) {
                if !path
                    .as_str()
                    .strip_prefix(adapter)
                    .is_some_and(|s| s.starts_with("/dev_"))
                {
                    return Ok(None);
                }
                if !activity(props)? {
                    return Ok(None);
                }
                return Ok(Some((path.to_string(), name(props)?)));
            }
        }
        if interface == Some(PROPERTIES) && member == Some("PropertiesChanged") {
            let (iface, props, _): (String, Props, Vec<String>) =
                message.body().deserialize().map_err(classify)?;
            let path = header.path().map(|p| p.as_str()).unwrap_or_default();
            if path == adapter
                && iface == ADAPTER
                && props.get("Powered").and_then(|v| bool::try_from(v).ok()) == Some(false)
            {
                return Err(Status::AdapterNotPowered);
            }
            if iface == DEVICE
                && path
                    .strip_prefix(adapter)
                    .is_some_and(|s| s.starts_with("/dev_"))
                && activity(&props)?
            {
                return Ok(Some((path.to_owned(), name(&props)?)));
            }
        }
        Ok(None)
    }
    pub(super) async fn run(options: Options, cancel: Arc<AtomicBool>) -> Report {
        let mut report = Report::new(options);
        // No detached worker: dropping each future cancels its work on this connection.
        let setup_deadline = Instant::now() + SETUP_TIMEOUT;
        let connected = async { Err(interrupted(&cancel).await) }
            .or(async {
                async {
                    async_io::Timer::at(setup_deadline).await;
                    Err(Status::Timeout)
                }
                .or(connect())
                .await
            })
            .await;
        let mut session = match connected {
            Ok(s) => s,
            Err(status) => {
                report.status = status;
                return report;
            }
        };
        let setup = async { Err(interrupted(&cancel).await) }
            .or(async {
                async {
                    async_io::Timer::at(setup_deadline).await;
                    Err(Status::Timeout)
                }
                .or(session.setup(options, &mut report))
                .await
            })
            .await;
        report.status = match setup {
            Ok(after) => {
                let started = Instant::now();
                let status = async { interrupted(&cancel).await }
                    .or(async {
                        async {
                            async_io::Timer::after(Duration::from_secs(u64::from(options.seconds)))
                                .await;
                            Status::Completed
                        }
                        .or(session.collect(after, &mut report))
                        .await
                    })
                    .await;
                report.actual_duration_ms = started.elapsed().as_millis() as u64;
                status
            }
            Err(status) => status,
        };
        report.cleanup_status = session.cleanup().await;
        // Explicitly close even if subscriptions still hold connection references.
        if bounded(Duration::from_secs(1), session.connection.clone().close())
            .await
            .is_err()
            && session.ownership.filter_attempted
        {
            report.cleanup_status = Cleanup::Incomplete;
        }
        if report.cleanup_status == Cleanup::Incomplete && report.status == Status::Completed {
            report.status = Status::CleanupIncomplete;
        }
        report.scan_completed =
            report.status == Status::Completed && report.cleanup_status == Cleanup::Complete;
        report
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn args(values: &[&str]) -> Vec<String> {
        values.iter().map(|s| (*s).into()).collect()
    }
    #[test]
    fn explicit_options_and_bounds() {
        assert_eq!(
            Options::parse(&args(&["--json"])),
            Ok(Options {
                seconds: 10,
                adapter_index: 1
            })
        );
        for seconds in ["0", "31", "-1", "+1", "1.5", "65536", "$(id)"] {
            assert!(Options::parse(&args(&["--seconds", seconds, "--json"])).is_err());
        }
        for flags in [
            vec![],
            vec!["--seconds", "1"],
            vec!["--json", "--json"],
            vec!["--adapter-index", "/org/bluez/hci0", "--json"],
            vec!["--adapter-index", "0", "--json"],
            vec!["--seconds", "1", "--seconds", "2", "--json"],
        ] {
            assert!(Options::parse(&args(&flags)).is_err());
        }
        assert!(
            Options::parse(&args(&[
                "--json",
                "--seconds",
                "30",
                "--adapter-index",
                "16"
            ]))
            .is_ok()
        );
    }
    #[test]
    fn privacy_reuses_p012_and_suppresses_address_aliases() {
        assert_eq!(private_name(Some("Watch\u{202e}\n".into())), "Watch");
        for value in [
            "AA:BB:CC:DD:EE:FF",
            "prefix AA-BB-CC-DD-EE-FF",
            "/org/bluez/hci0/dev_1",
        ] {
            assert_eq!(private_name(Some(value.into())), "Unnamed Bluetooth device");
        }
        assert_eq!(private_name(Some("界".repeat(100))).chars().count(), 80);
    }
    #[test]
    fn separate_schema_has_no_inventory_or_identity_fields() {
        let report = Report::new(Options {
            seconds: 10,
            adapter_index: 1,
        });
        let json = serde_json::to_value(report).unwrap();
        assert_eq!(json["source"], "bluez_le_discovery_session");
        assert_eq!(json["authorization"], "never_evaluated");
        assert_eq!(json["scan_started"], false);
        assert_eq!(json.as_object().unwrap().len(), 12);
    }
}
