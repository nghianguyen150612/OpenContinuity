//! Explicit bounded discovery, independent from the read-only inventory.
use serde::Serialize;
use std::{
    collections::VecDeque,
    io::Write,
    sync::mpsc::{self, SyncSender, TrySendError},
    sync::{
        Arc,
        atomic::{AtomicBool, AtomicU8, AtomicU32, Ordering},
    },
    time::{Duration, Instant},
};

pub const MAX_DEVICES: usize = 128;
pub const MAX_EVENTS: usize = 4096;
pub const MAX_DEVICE_RECORDS: usize = 512;
pub const MAX_JSONL_RECORD_BYTES: usize = 2 * 1024;
pub const MAX_JSONL_OUTPUT_BYTES: usize = 512 * 1024;
pub const SETUP_TIMEOUT: Duration = Duration::from_secs(5);
pub const CALL_TIMEOUT: Duration = Duration::from_secs(2);
const OUTPUT_WRITE_TIMEOUT: Duration = Duration::from_secs(2);
const TERMINAL_RESERVE_BYTES: usize = MAX_JSONL_RECORD_BYTES;

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
    OutputBlocked,
    OutputClosed,
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
    pub output: OutputMode,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OutputMode {
    Json,
    EventsJsonl,
}
impl Options {
    pub fn parse(args: &[String]) -> Result<Self, ()> {
        let mut seconds = None;
        let mut adapter = None;
        let mut output = None;
        let mut args = args.iter();
        while let Some(arg) = args.next() {
            match arg.as_str() {
                "--seconds" if seconds.is_none() => seconds = Some(number(args.next().ok_or(())?)?),
                "--adapter-index" if adapter.is_none() => {
                    adapter = Some(number(args.next().ok_or(())?)?)
                }
                "--json" if output.is_none() => output = Some(OutputMode::Json),
                "--events-jsonl" if output.is_none() => output = Some(OutputMode::EventsJsonl),
                _ => return Err(()),
            }
        }
        let seconds = seconds.unwrap_or(10);
        let adapter = adapter.unwrap_or(1);
        let Some(output) = output else {
            return Err(());
        };
        if !(1..=30).contains(&seconds) || !(1..=16).contains(&adapter) {
            return Err(());
        }
        Ok(Self {
            seconds,
            adapter_index: usize::from(adapter),
            output,
        })
    }
}
fn number(text: &str) -> Result<u16, ()> {
    if text.is_empty() || !text.bytes().all(|b| b.is_ascii_digit()) {
        return Err(());
    }
    text.parse().map_err(|_| ())
}

#[derive(Serialize)]
struct SessionEvent<'a> {
    schema_version: u8,
    source: &'static str,
    #[serde(rename = "type")]
    event_type: &'static str,
    authorization: &'static str,
    scan_session_id: &'a str,
    sequence: u32,
    state: &'static str,
    status: &'a str,
    scan_started: bool,
    requested_duration_seconds: u16,
    adapter_index: Option<usize>,
}

#[derive(Serialize)]
struct DeviceEvent<'a> {
    schema_version: u8,
    source: &'static str,
    #[serde(rename = "type")]
    event_type: &'static str,
    authorization: &'static str,
    scan_session_id: &'a str,
    sequence: u32,
    id: &'a str,
    display_name: &'a str,
    provenance: &'static str,
    change: &'static str,
}

#[derive(Serialize)]
struct TerminalEvent<'a> {
    schema_version: u8,
    source: &'static str,
    #[serde(rename = "type")]
    event_type: &'static str,
    authorization: &'static str,
    scan_session_id: &'a str,
    sequence: u32,
    scan_requested: bool,
    scan_started: bool,
    scan_completed: bool,
    requested_duration_seconds: u16,
    actual_duration_ms: u64,
    adapter_index: Option<usize>,
    status: &'a str,
    cleanup_status: Cleanup,
    device_count: usize,
    device_event_count: usize,
    termination_reason: &'static str,
}

struct WriteItem {
    sequence: u32,
    line: Vec<u8>,
}

struct WriterState {
    completed: AtomicU32,
    failed: AtomicU8,
}

/// A one-record queue keeps blocked stdout from growing memory or blocking the
/// asynchronous BlueZ event loop. The producer checks write progress while it
/// collects, and the CLI exits after bounded cleanup if the consumer stalls.
struct EventStream {
    session_id: String,
    sender: SyncSender<WriteItem>,
    writer: Arc<WriterState>,
    sequence: u32,
    total_bytes: usize,
    device_records: usize,
    pending: VecDeque<(u32, Instant)>,
    session_emitted: bool,
}

impl EventStream {
    fn new(session_id: String) -> Self {
        let (sender, receiver) = mpsc::sync_channel::<WriteItem>(1);
        let writer = Arc::new(WriterState {
            completed: AtomicU32::new(0),
            failed: AtomicU8::new(0),
        });
        let writer_state = writer.clone();
        std::thread::spawn(move || {
            let stdout = std::io::stdout();
            let mut output = stdout.lock();
            while let Ok(item) = receiver.recv() {
                if output.write_all(&item.line).is_err() || output.flush().is_err() {
                    writer_state.failed.store(1, Ordering::Release);
                    break;
                }
                writer_state
                    .completed
                    .store(item.sequence, Ordering::Release);
            }
        });
        Self {
            session_id,
            sender,
            writer,
            sequence: 1,
            total_bytes: 0,
            device_records: 0,
            pending: VecDeque::new(),
            session_emitted: false,
        }
    }

    fn session_id(&self) -> &str {
        &self.session_id
    }

    fn next_sequence(&self) -> u32 {
        self.sequence
    }

    fn check(&mut self) -> Result<(), Status> {
        if self.writer.failed.load(Ordering::Acquire) != 0 {
            return Err(Status::OutputClosed);
        }
        let completed = self.writer.completed.load(Ordering::Acquire);
        while self
            .pending
            .front()
            .is_some_and(|(sequence, _)| *sequence <= completed)
        {
            self.pending.pop_front();
        }
        if self
            .pending
            .front()
            .is_some_and(|(_, queued)| queued.elapsed() >= OUTPUT_WRITE_TIMEOUT)
        {
            return Err(Status::OutputBlocked);
        }
        Ok(())
    }

    fn drained(&mut self) -> Result<bool, Status> {
        self.check()?;
        Ok(self.pending.is_empty())
    }

    fn emit<T: Serialize>(&mut self, event: &T, terminal: bool) -> Result<(), Status> {
        self.check()?;
        let mut line = serde_json::to_vec(event).map_err(|_| Status::MalformedData)?;
        line.push(b'\n');
        if line.len() > MAX_JSONL_RECORD_BYTES {
            return Err(Status::DataLimit);
        }
        let limit = if terminal {
            MAX_JSONL_OUTPUT_BYTES
        } else {
            MAX_JSONL_OUTPUT_BYTES.saturating_sub(TERMINAL_RESERVE_BYTES)
        };
        if self.total_bytes.saturating_add(line.len()) > limit {
            return Err(Status::DataLimit);
        }
        let line_bytes = line.len();
        let sequence = self.sequence;
        match self.sender.try_send(WriteItem { sequence, line }) {
            Ok(()) => {}
            Err(TrySendError::Full(_)) => return Err(Status::OutputBlocked),
            Err(TrySendError::Disconnected(_)) => return Err(Status::OutputClosed),
        }
        self.total_bytes += line_bytes;
        self.sequence = self.sequence.saturating_add(1);
        self.pending.push_back((sequence, Instant::now()));
        Ok(())
    }

    fn session(&mut self, report: &Report, status: Status) -> Result<(), Status> {
        let session_id = self.session_id.clone();
        let event = SessionEvent {
            schema_version: 1,
            source: "bluez_le_discovery_events",
            event_type: "session",
            authorization: "never_evaluated",
            scan_session_id: &session_id,
            sequence: self.next_sequence(),
            state: if report.scan_started {
                "started"
            } else {
                "not_started"
            },
            status: if report.scan_started {
                "started"
            } else {
                status_name(status)
            },
            scan_started: report.scan_started,
            requested_duration_seconds: report.requested_duration_seconds,
            adapter_index: report.adapter_index,
        };
        self.emit(&event, false)?;
        self.session_emitted = true;
        Ok(())
    }

    fn device(&mut self, device: &ObservedDevice, change: &'static str) -> Result<(), Status> {
        if self.device_records >= MAX_DEVICE_RECORDS {
            return Err(Status::DataLimit);
        }
        let session_id = self.session_id.clone();
        let event = DeviceEvent {
            schema_version: 1,
            source: "bluez_le_discovery_events",
            event_type: "device",
            authorization: "never_evaluated",
            scan_session_id: &session_id,
            sequence: self.next_sequence(),
            id: &device.id,
            display_name: &device.display_name,
            provenance: device.provenance,
            change,
        };
        self.emit(&event, false)?;
        self.device_records += 1;
        Ok(())
    }

    fn terminal(&mut self, report: &Report) -> Result<(), Status> {
        let session_id = self.session_id.clone();
        let event = TerminalEvent {
            schema_version: 1,
            source: "bluez_le_discovery_events",
            event_type: "terminal",
            authorization: "never_evaluated",
            scan_session_id: &session_id,
            sequence: self.next_sequence(),
            scan_requested: report.scan_requested,
            scan_started: report.scan_started,
            scan_completed: report.scan_completed,
            requested_duration_seconds: report.requested_duration_seconds,
            actual_duration_ms: report.actual_duration_ms,
            adapter_index: report.adapter_index,
            status: status_name(report.status),
            cleanup_status: report.cleanup_status,
            device_count: report.devices.len(),
            device_event_count: self.device_records,
            termination_reason: termination_reason(report.status),
        };
        self.emit(&event, true)
    }
}

fn status_name(status: Status) -> &'static str {
    match status {
        Status::Completed => "completed",
        Status::Cancelled => "cancelled",
        Status::NoAdapter => "no_adapter",
        Status::AdapterNotPowered => "adapter_not_powered",
        Status::UnsupportedLe => "unsupported_le",
        Status::AdapterBusy => "adapter_busy",
        Status::BluezUnavailable => "bluez_unavailable",
        Status::SystemBusUnavailable => "system_bus_unavailable",
        Status::AccessDenied => "access_denied",
        Status::Timeout => "timeout",
        Status::MalformedData => "malformed_data",
        Status::CleanupIncomplete => "cleanup_incomplete",
        Status::DataLimit => "data_limit",
        Status::OutputBlocked => "output_blocked",
        Status::OutputClosed => "output_closed",
        #[cfg(not(target_os = "linux"))]
        Status::UnsupportedPlatform => "unsupported_platform",
    }
}

fn termination_reason(status: Status) -> &'static str {
    match status {
        Status::Completed => "duration_elapsed",
        Status::Cancelled => "user_cancelled",
        Status::NoAdapter => "adapter_lost_or_unavailable",
        Status::AdapterNotPowered => "adapter_power_lost_or_off",
        Status::BluezUnavailable => "bluez_owner_lost_or_unavailable",
        Status::Timeout => "deadline_expired",
        Status::DataLimit => "stream_or_input_limit",
        Status::OutputBlocked => "stdout_blocked",
        Status::OutputClosed => "stdout_closed",
        Status::CleanupIncomplete => "cleanup_failed",
        _ => "setup_or_observation_failed",
    }
}

fn new_scan_session_id() -> Result<String, Status> {
    let mut random = [0u8; 16];
    getrandom::getrandom(&mut random).map_err(|_| Status::MalformedData)?;
    let mut id = String::with_capacity(32);
    for byte in random {
        use std::fmt::Write;
        write!(&mut id, "{byte:02x}").expect("String write cannot fail");
    }
    Ok(id)
}

pub fn run(options: Options) -> Report {
    run_internal(options, false)
}

pub fn run_events(options: Options) -> Report {
    run_internal(options, true)
}

fn run_internal(options: Options, stream_enabled: bool) -> Report {
    let mut stream = if stream_enabled {
        match new_scan_session_id() {
            Ok(id) => Some(EventStream::new(id)),
            Err(status) => {
                let mut report = Report::new(options);
                report.status = status;
                return report;
            }
        }
    } else {
        None
    };
    let cancelled = Arc::new(AtomicBool::new(false));
    let mut handlers = Vec::new();
    let mut handler_error = false;
    for signal in [signal_hook::consts::SIGINT, signal_hook::consts::SIGTERM] {
        match signal_hook::flag::register(signal, cancelled.clone()) {
            Ok(id) => handlers.push(id),
            Err(_) => {
                for id in handlers.drain(..) {
                    signal_hook::low_level::unregister(id);
                }
                handler_error = true;
                break;
            }
        }
    }
    #[cfg(target_os = "linux")]
    let mut report = if handler_error {
        let mut report = Report::new(options);
        report.status = Status::MalformedData;
        report
    } else {
        futures_lite::future::block_on(linux::run(options, cancelled, stream.as_mut()))
    };
    #[cfg(not(target_os = "linux"))]
    let mut report = {
        let mut r = Report::new(options);
        r.status = if handler_error {
            Status::MalformedData
        } else {
            Status::UnsupportedPlatform
        };
        r
    };
    if let Some(stream) = stream.as_mut() {
        #[cfg(target_os = "linux")]
        if let Err(status) =
            futures_lite::future::block_on(linux::finish_stream(stream, &mut report))
        {
            report.status = status;
            report.scan_completed = false;
        }
        #[cfg(not(target_os = "linux"))]
        {
            if stream.session(&report, report.status).is_err() || stream.terminal(&report).is_err()
            {
                report.status = Status::OutputClosed;
            }
        }
    }
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
        async fn collect(
            &mut self,
            after: Sequence,
            report: &mut Report,
            mut stream: Option<&mut EventStream>,
        ) -> Status {
            let mut seen = HashMap::<String, usize>::new();
            let prefix = if let Some(stream) = stream.as_deref() {
                stream.session_id()[..16].to_owned()
            } else {
                let mut random = [0u8; 8];
                if getrandom::getrandom(&mut random).is_err() {
                    return Status::MalformedData;
                }
                let mut prefix = String::with_capacity(16);
                for byte in random {
                    use std::fmt::Write;
                    write!(&mut prefix, "{byte:02x}").expect("String write cannot fail");
                }
                prefix
            };
            for _ in 0..MAX_EVENTS {
                if let Some(sink) = stream.as_deref_mut() {
                    if let Err(status) = sink.check() {
                        return status;
                    }
                }
                let event = if stream.is_some() {
                    async { Some((true, self.owners.next().await)) }
                        .or(async {
                            async_io::Timer::after(Duration::from_millis(20)).await;
                            None
                        })
                        .or(async { Some((false, self.signals.next().await)) })
                        .await
                } else {
                    Some(
                        async { (true, self.owners.next().await) }
                            .or(async { (false, self.signals.next().await) })
                            .await,
                    )
                };
                let Some(event) = event else {
                    continue;
                };
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
                                let sanitized = private_name(name);
                                if report.devices[*index].display_name != sanitized {
                                    let previous_name = report.devices[*index].display_name.clone();
                                    report.devices[*index].display_name = sanitized;
                                    if let Some(sink) = stream.as_deref_mut() {
                                        if let Err(status) =
                                            sink.device(&report.devices[*index], "updated")
                                        {
                                            report.devices[*index].display_name = previous_name;
                                            return status;
                                        }
                                    }
                                }
                            }
                        } else {
                            if seen.len() >= MAX_DEVICES {
                                return Status::DataLimit;
                            }
                            seen.insert(path.clone(), report.devices.len());
                            report.devices.push(ObservedDevice {
                                id: format!("scan-{prefix}-{:03}", seen.len()),
                                display_name: private_name(name),
                                provenance: "bluez_event_during_session",
                            });
                            if let Some(sink) = stream.as_deref_mut() {
                                if let Err(status) = sink.device(
                                    report.devices.last().expect("device was just inserted"),
                                    "observed",
                                ) {
                                    report.devices.pop();
                                    seen.remove(&path);
                                    return status;
                                }
                            }
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
    pub(super) async fn run(
        options: Options,
        cancel: Arc<AtomicBool>,
        mut stream: Option<&mut EventStream>,
    ) -> Report {
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
                let stream_error = stream
                    .as_deref_mut()
                    .and_then(|sink| sink.session(&report, Status::Completed).err());
                if let Some(status) = stream_error {
                    report.actual_duration_ms = started.elapsed().as_millis() as u64;
                    status
                } else {
                    let status = async { interrupted(&cancel).await }
                        .or(async {
                            async {
                                async_io::Timer::after(Duration::from_secs(u64::from(
                                    options.seconds,
                                )))
                                .await;
                                Status::Completed
                            }
                            .or(session.collect(after, &mut report, stream.as_deref_mut()))
                            .await
                        })
                        .await;
                    report.actual_duration_ms = started.elapsed().as_millis() as u64;
                    status
                }
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

    pub(super) async fn finish_stream(
        stream: &mut EventStream,
        report: &mut Report,
    ) -> Result<(), Status> {
        if !stream.session_emitted {
            if let Err(status) = wait_for_drain(stream, OUTPUT_WRITE_TIMEOUT).await {
                report.status = status;
                report.scan_completed = false;
                return Err(status);
            }
            if let Err(status) = stream.session(report, report.status) {
                report.status = status;
                report.scan_completed = false;
                return Err(status);
            }
        }
        if let Err(status) = wait_for_drain(stream, OUTPUT_WRITE_TIMEOUT).await {
            report.status = status;
            report.scan_completed = false;
            return Err(status);
        }
        if let Err(status) = stream.terminal(report) {
            report.status = status;
            report.scan_completed = false;
            return Err(status);
        }
        if let Err(status) = wait_for_drain(stream, OUTPUT_WRITE_TIMEOUT).await {
            // The terminal may have reached stdout, but an undrained write is
            // not a confirmed successful CLI result. Consumers reject the
            // resulting exit/terminal mismatch and mark cleanup uncertain.
            report.status = status;
            report.scan_completed = false;
            return Err(status);
        }
        Ok(())
    }

    async fn wait_for_drain(stream: &mut EventStream, timeout: Duration) -> Result<(), Status> {
        let deadline = Instant::now() + timeout;
        loop {
            if stream.drained()? {
                return Ok(());
            }
            if Instant::now() >= deadline {
                return Err(Status::OutputBlocked);
            }
            async_io::Timer::after(Duration::from_millis(10)).await;
        }
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
                adapter_index: 1,
                output: OutputMode::Json,
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
            vec!["--json", "--events-jsonl"],
            vec!["--events-jsonl", "--events-jsonl"],
            vec!["--events-jsonl", "--json"],
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
            output: OutputMode::Json,
        });
        let json = serde_json::to_value(report).unwrap();
        assert_eq!(json["source"], "bluez_le_discovery_session");
        assert_eq!(json["authorization"], "never_evaluated");
        assert_eq!(json["scan_started"], false);
        assert_eq!(json.as_object().unwrap().len(), 12);
    }

    #[test]
    fn jsonl_record_and_total_output_caps_fail_before_enqueue() {
        let mut stream = EventStream::new("0123456789abcdef0123456789abcdef".into());
        let oversized = serde_json::json!({"text": "x".repeat(MAX_JSONL_RECORD_BYTES)});
        assert_eq!(stream.emit(&oversized, false), Err(Status::DataLimit));
        let small = serde_json::json!({"ok": true});
        stream.total_bytes = MAX_JSONL_OUTPUT_BYTES - TERMINAL_RESERVE_BYTES;
        assert_eq!(stream.emit(&small, false), Err(Status::DataLimit));
        stream.device_records = MAX_DEVICE_RECORDS;
        let device = ObservedDevice {
            id: "scan-0123456789abcdef-001".into(),
            display_name: "name".into(),
            provenance: "bluez_event_during_session",
        };
        assert_eq!(stream.device(&device, "updated"), Err(Status::DataLimit));
    }
}
