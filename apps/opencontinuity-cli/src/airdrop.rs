//! Read-only AirDrop backend doctor and explicit Owly receive supervision.

use opencontinuity_core::airdrop::{
    AirDropReport, BackendState, BlockReason, ObservationSource, PlaneState, RadioReport,
};
use serde_json::Value;
use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};
use std::time::{Duration, Instant};

#[cfg(target_os = "linux")]
#[path = "airdrop_containment.rs"]
mod containment;
#[path = "airdrop_process.rs"]
mod process;
#[cfg(target_os = "linux")]
#[path = "airdrop_quota.rs"]
mod quota;
#[cfg(target_os = "linux")]
#[path = "airdrop_receive.rs"]
mod storage;

const OWLY_VERSION: &str = "0.4.2";
const OWLY_DOCTOR_OUTPUT_LIMIT: usize = 64 * 1024;
const MIN_RECEIVE_SECONDS: u16 = 15;
const MAX_RECEIVE_SECONDS: u16 = 3600;
const DEFAULT_RECEIVE_SECONDS: u16 = 300;

const USAGE: &str = "Usage:\n  opencontinuity airdrop doctor [--json]\n  opencontinuity airdrop backend status [--json]\n  opencontinuity airdrop receive --backend owly --destination <existing-directory> --duration-seconds <15..3600> --radio-prepared --consent\n\nDoctor/status are read-only. Receive requires an already-active Owly AWDL radio, a private user runtime directory, a terminal, explicit consent, and Owly's per-transfer approval prompt. OpenContinuity never activates the radio.";

pub fn run(args: &[String]) -> ExitCode {
    match args.first().map(String::as_str) {
        Some("doctor") if valid_json_suffix(&args[1..]) => print_report(),
        Some("backend")
            if args.get(1).map(String::as_str) == Some("status")
                && valid_json_suffix(&args[2..]) =>
        {
            print_report()
        }
        Some("receive") => run_receive(&args[1..]),
        _ => {
            eprintln!("{USAGE}");
            ExitCode::from(2)
        }
    }
}

fn valid_json_suffix(args: &[String]) -> bool {
    args.is_empty() || (args.len() == 1 && args[0] == "--json")
}

fn print_report() -> ExitCode {
    let report = probe();
    match serde_json::to_string(&report) {
        Ok(json) => {
            println!("{json}");
            ExitCode::SUCCESS
        }
        Err(_) => {
            println!(
                "{{\"schemaVersion\":1,\"kind\":\"airdrop_backend_status\",\"authorization\":\"never_evaluated\",\"physicalQualification\":\"not_qualified\",\"backend\":\"failed\"}}"
            );
            ExitCode::from(1)
        }
    }
}

#[cfg(target_os = "linux")]
fn probe() -> AirDropReport {
    let mut report = AirDropReport::unknown();
    report.source = ObservationSource::LocalSysfs;
    report.radio = probe_radio();
    report.backend = BackendState::Blocked;
    report.control_plane = PlaneState::Blocked;
    report.data_plane = PlaneState::Unknown;
    report.advertisement = PlaneState::Unknown;
    report.block_reason = Some(BlockReason::BackendNotInstalled);

    let Some(executable) = find_fixed_executable("owly") else {
        return report;
    };
    let version = process::run_bounded(&executable, ["--version"], Duration::from_secs(2), 4096);
    let Ok(version) = version else {
        report.block_reason = Some(BlockReason::BackendProbeFailed);
        return report;
    };
    if version.timed_out
        || version.stdout_truncated
        || version.stderr_truncated
        || !version.status.success()
    {
        report.block_reason = Some(BlockReason::BackendProbeFailed);
        return report;
    }
    let version_line = String::from_utf8_lossy(&version.stdout).trim().to_owned();
    let Some(actual_version) = version_line.strip_prefix("owly ") else {
        report.block_reason = Some(BlockReason::BackendVersionUnsupported);
        return report;
    };
    if !supported_owly_version(actual_version) {
        report.block_reason = Some(BlockReason::BackendVersionUnsupported);
        report.backend = BackendState::Unknown;
        return report;
    }
    report.upstream_version = Some(OWLY_VERSION.to_owned());

    let doctor = process::run_bounded(
        &executable,
        ["doctor", "--json"],
        Duration::from_secs(3),
        OWLY_DOCTOR_OUTPUT_LIMIT,
    );
    let Ok(doctor) = doctor else {
        report.block_reason = Some(BlockReason::BackendProbeFailed);
        return report;
    };
    let Ok(ready) = doctor_ready(&doctor) else {
        report.block_reason = Some(BlockReason::BackendProbeFailed);
        return report;
    };
    report.source = ObservationSource::OwlyDoctor;
    if !ready {
        report.backend = BackendState::Blocked;
        report.control_plane = PlaneState::Blocked;
        report.block_reason = Some(BlockReason::BackendProbeFailed);
        return report;
    }

    report.control_plane = PlaneState::Ready;
    if report.radio.awdl_interface_state == BackendState::Ready
        && report.radio.awdl_link_local_address_present
    {
        report.backend = BackendState::Ready;
        report.data_plane = PlaneState::Available;
        report.receiver_available = true;
        report.block_reason = None;
    } else {
        report.backend = BackendState::Blocked;
        report.data_plane = PlaneState::Blocked;
        report.block_reason = Some(BlockReason::RadioNotPrepared);
    }
    // Owly's CLI has no machine-readable advertisement-ready or per-transfer
    // progress API. Do not infer those states from process availability.
    report.advertisement = PlaneState::Unknown;
    report.live_airdrop_interoperability = BackendState::Unknown;
    report
}

fn supported_owly_version(version: &str) -> bool {
    version == OWLY_VERSION
}

/// Owly 0.4.2 Capabilities/Card/ChannelUse at upstream 6f2e8ea.
fn doctor_ready(output: &process::CommandOutput) -> Result<bool, ()> {
    if output.timed_out
        || output.stdout_truncated
        || output.stderr_truncated
        || !output.status.success()
    {
        return Err(());
    }
    let json: Value = serde_json::from_slice(&output.stdout).map_err(|_| ())?;
    validate_doctor(&json)
}

fn validate_doctor(json: &Value) -> Result<bool, ()> {
    let object = json.as_object().ok_or(())?;
    for key in ["station", "driver", "device"] {
        let value = object.get(key).ok_or(())?;
        if !value.is_null() && !value.is_string() {
            return Err(());
        }
    }
    for (key, values) in [
        ("status", &["tested", "untested", "broken"][..]),
        ("mode", &["shared", "exclusive", "active-monitor"][..]),
    ] {
        let value = object.get(key).ok_or(())?;
        if !value.is_null() && !value.as_str().is_some_and(|s| values.contains(&s)) {
            return Err(());
        }
    }
    let channel = object.get("channel").ok_or(())?;
    if !channel.is_null() && channel.as_u64().is_none_or(|n| n > 255) {
        return Err(());
    }
    let hardware = object.get("hardware").and_then(Value::as_bool).ok_or(())?;
    let issues = object.get("issues").and_then(Value::as_array).ok_or(())?;
    if !issues.iter().all(Value::is_string) {
        return Err(());
    }
    let missing = object.get("missing").and_then(Value::as_array).ok_or(())?;
    for entry in missing {
        if !matches!(
            entry.get("id").and_then(Value::as_str),
            Some("hardware" | "backend" | "radio")
        ) || !entry.get("say").is_some_and(Value::is_string)
        {
            return Err(());
        }
    }
    let card = object.get("card").ok_or(())?;
    if !card.is_null() {
        if !card.get("phy").is_some_and(Value::is_string) {
            return Err(());
        }
        for key in [
            "ibss",
            "ap",
            "active_monitor",
            "monitor_beside_station",
            "ibss_beside_station",
            "self_managed_reg",
        ] {
            if !card.get(key).is_some_and(Value::is_boolean) {
                return Err(());
            }
        }
        let regdom = card.get("regdom").ok_or(())?;
        if !regdom.is_null() && !regdom.is_string() {
            return Err(());
        }
        for channel in card.get("channels").and_then(Value::as_array).ok_or(())? {
            if channel
                .get("channel")
                .and_then(Value::as_u64)
                .is_none_or(|n| n > 255)
                || channel
                    .get("freq")
                    .and_then(Value::as_u64)
                    .is_none_or(|n| n > u32::MAX.into())
                || !channel.get("transmit").is_some_and(Value::is_boolean)
            {
                return Err(());
            }
            // Upstream omits 'why' when it is None.
            if let Some(why) = channel.get("why") {
                if !why.is_null() && !why.is_string() {
                    return Err(());
                }
            }
        }
    }
    Ok(missing.is_empty() && hardware)
}

#[cfg(not(target_os = "linux"))]
fn probe() -> AirDropReport {
    let mut report = AirDropReport::unknown();
    report.backend = BackendState::Unsupported;
    report.control_plane = PlaneState::Unsupported;
    report.data_plane = PlaneState::Unsupported;
    report.advertisement = PlaneState::Unsupported;
    report.radio.state = BackendState::Unsupported;
    report.radio.monitor_mode = BackendState::Unsupported;
    report
}

#[cfg(target_os = "linux")]
fn probe_radio() -> RadioReport {
    let mut radio = RadioReport::unknown();
    radio.kernel_release = sanitized_kernel_release();
    radio.effective_uid_is_root = unsafe { libc::geteuid() } == 0;
    let capabilities = effective_capabilities();
    radio.net_admin_capability = capabilities.is_some_and(|bits| bits & (1_u64 << 12) != 0);
    radio.net_raw_capability = capabilities.is_some_and(|bits| bits & (1_u64 << 13) != 0);

    let Ok(interfaces) = std::fs::read_dir("/sys/class/net") else {
        return radio;
    };
    let mut first_wireless: Option<(String, PathBuf)> = None;
    let mut count = 0_u8;
    for interface in interfaces.flatten() {
        let name = interface.file_name().to_string_lossy().into_owned();
        if name == "awdl0" || !valid_interface_name(&name) {
            continue;
        }
        let path = interface.path();
        if !path.join("wireless").exists() && !path.join("phy80211").exists() {
            continue;
        }
        count = count.saturating_add(1);
        if first_wireless.is_none() {
            first_wireless = Some((name, path));
        }
    }
    radio.wireless_interface_count = count;
    radio.conflicting_wireless_interface_count = count.saturating_sub(1);
    if let Some((name, path)) = first_wireless {
        radio.state = BackendState::Available;
        if let Ok(driver) = std::fs::read_link(path.join("device/driver")) {
            radio.driver = driver
                .file_name()
                .and_then(|v| v.to_str())
                .filter(|v| bounded_identifier(v, 32))
                .map(str::to_owned);
        }
        if let (Ok(vendor), Ok(device)) = (
            std::fs::read_to_string(path.join("device/vendor")),
            std::fs::read_to_string(path.join("device/device")),
        ) {
            if let (Some(vendor), Some(device)) = (
                vendor.trim().strip_prefix("0x"),
                device.trim().strip_prefix("0x"),
            ) {
                if hex_id(vendor) && hex_id(device) {
                    radio.pci_id = Some(format!(
                        "{}:{}",
                        vendor.to_ascii_lowercase(),
                        device.to_ascii_lowercase()
                    ));
                }
            }
        }
        inspect_iw(&name, &path, &mut radio);
    } else {
        radio.state = BackendState::Unsupported;
    }

    let awdl = Path::new("/sys/class/net/awdl0");
    if !awdl.exists() {
        radio.awdl_interface_state = BackendState::Blocked;
        return radio;
    }
    let flags_up = std::fs::read_to_string(awdl.join("flags"))
        .ok()
        .and_then(|value| u32::from_str_radix(value.trim().trim_start_matches("0x"), 16).ok())
        .is_some_and(|flags| flags & libc::IFF_UP as u32 != 0);
    radio.awdl_link_local_address_present = std::fs::read_to_string("/proc/net/if_inet6")
        .ok()
        .is_some_and(|entries| {
            entries.lines().any(|line| {
                let mut fields = line.split_whitespace();
                fields
                    .next()
                    .is_some_and(|address| address.starts_with("fe80"))
                    && fields.nth(4).is_some_and(|name| name == "awdl0")
            })
        });
    radio.awdl_interface_state = if flags_up && radio.awdl_link_local_address_present {
        BackendState::Ready
    } else {
        BackendState::Available
    };
    radio
}

#[cfg(target_os = "linux")]
fn inspect_iw(interface: &str, path: &Path, radio: &mut RadioReport) {
    let Some(iw) = find_fixed_executable("iw") else {
        return;
    };
    let link = process::run_bounded(
        &iw,
        ["dev", interface, "link"],
        Duration::from_secs(1),
        8192,
    );
    if let Ok(link) = link {
        if !link.timed_out && !link.stdout_truncated && link.status.success() {
            let output = String::from_utf8_lossy(&link.stdout);
            radio.associated = if output.contains("Not connected.") {
                Some(false)
            } else if output
                .lines()
                .any(|line| line.trim_start().starts_with("Connected to "))
            {
                Some(true)
            } else {
                None
            };
            radio.wifi_channel = output
                .lines()
                .find_map(|line| line.trim().strip_prefix("freq:"))
                .and_then(|value| value.trim().parse::<u16>().ok())
                .and_then(channel_for_frequency);
        }
    }
    let reg = process::run_bounded(&iw, ["reg", "get"], Duration::from_secs(1), 8192);
    if let Ok(reg) = reg {
        if !reg.timed_out && !reg.stdout_truncated && reg.status.success() {
            radio.regulatory_domain = String::from_utf8_lossy(&reg.stdout)
                .lines()
                .find_map(|line| line.trim().strip_prefix("country "))
                .and_then(|country| country.get(..2))
                .filter(|country| country.bytes().all(|b| b.is_ascii_uppercase()))
                .map(str::to_owned);
        }
    }
    let phy = std::fs::read_link(path.join("phy80211"))
        .ok()
        .and_then(|link| link.file_name().map(|v| v.to_string_lossy().into_owned()));
    if let Some(phy) = phy.filter(|name| bounded_identifier(name, 32)) {
        let info = process::run_bounded(
            &iw,
            ["phy", phy.as_str(), "info"],
            Duration::from_secs(1),
            32 * 1024,
        );
        if let Ok(info) = info {
            if !info.timed_out && !info.stdout_truncated && info.status.success() {
                let output = String::from_utf8_lossy(&info.stdout);
                radio.monitor_mode = if output.lines().any(|line| line.trim() == "* monitor") {
                    BackendState::Available
                } else {
                    BackendState::Unsupported
                };
            }
        }
    }
}

#[cfg(target_os = "linux")]
fn sanitized_kernel_release() -> Option<String> {
    std::fs::read_to_string("/proc/sys/kernel/osrelease")
        .ok()
        .map(|value| value.trim().chars().take(64).collect::<String>())
        .filter(|value| {
            !value.is_empty()
                && value
                    .chars()
                    .all(|c| c.is_ascii_alphanumeric() || ".+_-".contains(c))
        })
}

#[cfg(target_os = "linux")]
fn effective_capabilities() -> Option<u64> {
    let status = std::fs::read_to_string("/proc/self/status").ok()?;
    status.lines().find_map(|line| {
        let value = line.strip_prefix("CapEff:")?.trim();
        u64::from_str_radix(value, 16).ok()
    })
}

#[cfg(target_os = "linux")]
fn find_fixed_executable(name: &str) -> Option<PathBuf> {
    let candidates = match name {
        "owly" => ["/usr/bin/owly", "/usr/local/bin/owly"],
        "iw" => ["/usr/bin/iw", "/usr/sbin/iw"],
        _ => return None,
    };
    candidates.into_iter().find_map(|candidate| {
        let path = Path::new(candidate);
        let metadata = std::fs::symlink_metadata(path).ok()?;
        if !metadata.is_file() || metadata.file_type().is_symlink() {
            return None;
        }
        use std::os::unix::fs::{MetadataExt, PermissionsExt};
        trusted_executable_metadata(
            metadata.uid(),
            metadata.permissions().mode(),
            metadata.is_file(),
            metadata.file_type().is_symlink(),
        )
        .then(|| path.to_path_buf())
    })
}

#[cfg(target_os = "linux")]
fn trusted_executable_metadata(uid: u32, mode: u32, regular: bool, symlink: bool) -> bool {
    regular && !symlink && uid == 0 && mode & 0o111 != 0 && mode & 0o6022 == 0
}

#[cfg(target_os = "linux")]
fn valid_interface_name(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 15
        && value
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"_.-".contains(&b))
}

#[cfg(target_os = "linux")]
fn bounded_identifier(value: &str, max: usize) -> bool {
    !value.is_empty()
        && value.len() <= max
        && value
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"_.+-".contains(&b))
}

#[cfg(target_os = "linux")]
fn hex_id(value: &str) -> bool {
    value.len() == 4 && value.bytes().all(|b| b.is_ascii_hexdigit())
}

#[cfg(target_os = "linux")]
fn channel_for_frequency(frequency: u16) -> Option<u16> {
    match frequency {
        2484 => Some(14),
        2412..=2472 => Some((frequency - 2407) / 5),
        5005..=5895 => Some((frequency - 5000) / 5),
        5955..=7115 => Some((frequency - 5950) / 5),
        _ => None,
    }
}

#[cfg(all(test, target_os = "linux"))]
mod executable_tests {
    use super::trusted_executable_metadata;

    #[test]
    fn fixed_backend_must_be_root_owned_nonwritable_and_not_setid() {
        assert!(trusted_executable_metadata(0, 0o755, true, false));
        assert!(!trusted_executable_metadata(1000, 0o755, true, false));
        assert!(!trusted_executable_metadata(0, 0o775, true, false));
        assert!(!trusted_executable_metadata(0, 0o757, true, false));
        assert!(!trusted_executable_metadata(0, 0o4755, true, false));
        assert!(!trusted_executable_metadata(0, 0o2755, true, false));
        assert!(!trusted_executable_metadata(0, 0o644, true, false));
        assert!(!trusted_executable_metadata(0, 0o755, false, false));
        assert!(!trusted_executable_metadata(0, 0o755, true, true));
    }
}

#[cfg(target_os = "linux")]
fn run_receive(args: &[String]) -> ExitCode {
    receive::run(args)
}

#[cfg(not(target_os = "linux"))]
fn run_receive(_: &[String]) -> ExitCode {
    eprintln!("AirDrop receive is unsupported on this platform");
    ExitCode::from(2)
}

#[cfg(target_os = "linux")]
mod receive {
    use super::*;
    use crate::airdrop::containment::{self, ContainmentError, Controller, DelegatedSubtree};
    use crate::airdrop::process::MAX_RECEIVE_TERMINAL_OUTPUT;
    #[cfg(test)]
    use crate::airdrop::quota::QuotaSnapshot;
    use crate::airdrop::quota::{ProjectQuotaLease, QuotaError, QuotaLease};
    use crate::airdrop::storage::{ReceiveLock, ReceiveSessionState, ReceiveStaging};
    use opencontinuity_core::airdrop::{BlockReason, ReceiveLifecycle, ReceiveState};
    use serde::Serialize;
    use std::fs::{File, OpenOptions};
    use std::io;
    use std::os::fd::{AsRawFd, RawFd};
    use std::os::unix::fs::OpenOptionsExt;
    use std::os::unix::process::CommandExt;
    use std::process::{Child, Command, Stdio};
    use std::sync::atomic::AtomicUsize;

    #[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
    #[serde(rename_all = "snake_case")]
    enum EndReason {
        Cancelled,
        Expired,
        BackendExited,
        OutputLimitExceeded,
        Failed,
        WriterContainmentUnavailable,
    }

    #[derive(Serialize)]
    #[serde(rename_all = "camelCase")]
    struct ReceiveResult {
        schema_version: u8,
        kind: &'static str,
        state: ReceiveState,
        end_reason: EndReason,
        backend: &'static str,
        backend_process: BackendState,
        control_plane: PlaneState,
        advertisement: PlaneState,
        data_plane: PlaneState,
        transfer_outcome: BackendState,
        completed_file_count: u64,
        completed_bytes: u64,
        cleanup: &'static str,
        block_reason: Option<BlockReason>,
        authorization: &'static str,
        physical_qualification: &'static str,
    }

    struct Options {
        destination: PathBuf,
        duration_seconds: u16,
        consent: bool,
        radio_prepared: bool,
    }

    impl Options {
        fn parse(args: &[String]) -> Result<Self, ()> {
            let mut backend = None;
            let mut destination = None;
            let mut duration_seconds = DEFAULT_RECEIVE_SECONDS;
            let mut duration_seen = false;
            let mut consent = false;
            let mut radio_prepared = false;
            let mut index = 0;
            while index < args.len() {
                match args[index].as_str() {
                    "--backend" if backend.is_none() => {
                        index += 1;
                        backend = args.get(index).map(String::as_str);
                        if backend != Some("owly") {
                            return Err(());
                        }
                    }
                    "--destination" if destination.is_none() => {
                        index += 1;
                        destination = args.get(index).map(PathBuf::from);
                    }
                    "--duration-seconds" if !duration_seen => {
                        duration_seen = true;
                        index += 1;
                        duration_seconds =
                            args.get(index).ok_or(())?.parse::<u16>().map_err(|_| ())?;
                        if !(MIN_RECEIVE_SECONDS..=MAX_RECEIVE_SECONDS).contains(&duration_seconds)
                        {
                            return Err(());
                        }
                    }
                    "--consent" if !consent => consent = true,
                    "--radio-prepared" if !radio_prepared => radio_prepared = true,
                    _ => return Err(()),
                }
                index += 1;
            }
            if backend != Some("owly") || !consent || !radio_prepared {
                return Err(());
            }
            Ok(Self {
                destination: destination.ok_or(())?,
                duration_seconds,
                consent,
                radio_prepared,
            })
        }
    }

    pub fn run(args: &[String]) -> ExitCode {
        let options = match Options::parse(args) {
            Ok(options) => options,
            Err(()) => {
                eprintln!("invalid AirDrop receive options\n{USAGE}");
                return ExitCode::from(2);
            }
        };
        if !options.consent || !options.radio_prepared {
            return print_blocked(
                BlockReason::RadioNotPrepared,
                EndReason::Failed,
                ReceiveState::Failed,
            );
        }
        if unsafe { libc::isatty(libc::STDIN_FILENO) } != 1
            || unsafe { libc::isatty(libc::STDERR_FILENO) } != 1
        {
            return print_blocked(
                BlockReason::InteractiveConsentRequired,
                EndReason::Failed,
                ReceiveState::Failed,
            );
        }
        let _lock = match ReceiveLock::acquire_runtime() {
            Ok(lock) => lock,
            Err(error) if error.kind() == io::ErrorKind::WouldBlock => {
                return print_blocked(
                    BlockReason::ConcurrentReceive,
                    EndReason::Failed,
                    ReceiveState::Failed,
                );
            }
            Err(_) => {
                return print_blocked(
                    BlockReason::PermissionUnavailable,
                    EndReason::Failed,
                    ReceiveState::Failed,
                );
            }
        };
        let report = probe();
        if !report.receiver_available {
            return print_blocked(
                report.block_reason.unwrap_or(BlockReason::RadioNotPrepared),
                EndReason::Failed,
                ReceiveState::Failed,
            );
        }
        if !writer_containment_available() {
            return print_blocked(
                BlockReason::BackendFailed,
                EndReason::WriterContainmentUnavailable,
                ReceiveState::Failed,
            );
        }
        let Some(executable) = find_fixed_executable("owly") else {
            return print_blocked(
                BlockReason::BackendNotInstalled,
                EndReason::Failed,
                ReceiveState::Failed,
            );
        };
        let cancelled = Arc::new(AtomicBool::new(false));
        let commit_gate = Arc::new(CommitGate::default());
        let mut signals = CancelSignals(Vec::new());
        for signal in [signal_hook::consts::SIGINT, signal_hook::consts::SIGTERM] {
            let cancelled = cancelled.clone();
            let commit_gate = commit_gate.clone();
            // SAFETY: the handler performs only lock-free AtomicBool/AtomicU8
            // operations, which are signal-safe on the required Linux target.
            match unsafe {
                signal_hook::low_level::register(signal, move || {
                    commit_gate.revoke();
                    cancelled.store(true, Ordering::SeqCst);
                })
            } {
                Ok(id) => signals.0.push(id),
                Err(_) => {
                    return print_blocked(
                        BlockReason::BackendFailed,
                        EndReason::Failed,
                        ReceiveState::Failed,
                    );
                }
            }
        }
        let admission = Admission {
            destination: &options.destination,
            cancelled: &cancelled,
            commit_gate: &commit_gate,
            consent_valid: options.consent,
            deadline: Instant::now() + Duration::from_secs(options.duration_seconds.into()),
            now: &Instant::now,
        };
        let result = coordinate(
            admission,
            DelegatedSubtree::from_environment,
            || ProjectQuotaLease::from_environment(&options.destination),
            |session, staging, runtime| {
                let mut command = Command::new(executable);
                command
                    .args([
                        "receive",
                        "--iface",
                        "awdl0",
                        "--name",
                        "OpenContinuity",
                        "--dir",
                    ])
                    .arg(staging.path())
                    .env("XDG_STATE_HOME", runtime.path())
                    .env_remove("RUST_LOG")
                    .stdin(Stdio::inherit())
                    .stdout(Stdio::piped())
                    .stderr(Stdio::piped());
                install_child_limits(&mut command);
                session.spawn(command)
            },
            || open_nonblocking_terminal(libc::STDERR_FILENO),
            MAX_RECEIVE_TERMINAL_OUTPUT,
        );
        match result {
            Ok(completion) => report_completion(completion),
            Err((block, completion)) if completion.cleanup == "complete" => {
                print_blocked(block, completion.run.0, completion.run.1)
            }
            Err((block, completion)) => emit_completion(completion, Some(block)),
        }
    }

    struct CancelSignals(Vec<signal_hook::SigId>);

    impl Drop for CancelSignals {
        fn drop(&mut self) {
            for id in self.0.drain(..) {
                signal_hook::low_level::unregister(id);
            }
        }
    }

    /// The CLI constructs this only after consent, argument and radio checks.
    /// Its deadline bounds consent through the terminal decision.
    struct Admission<'a> {
        destination: &'a Path,
        cancelled: &'a AtomicBool,
        commit_gate: &'a CommitGate,
        consent_valid: bool,
        deadline: Instant,
        now: &'a dyn Fn() -> Instant,
    }

    impl Admission<'_> {
        fn interrupted(&self) -> Option<BackendRun> {
            if self.cancelled.load(Ordering::SeqCst) || self.commit_gate.is_revoked() {
                Some((
                    EndReason::Cancelled,
                    ReceiveState::Cancelled,
                    BackendState::Failed,
                ))
            } else if (self.now)() >= self.deadline {
                Some((
                    EndReason::Expired,
                    ReceiveState::Expired,
                    BackendState::Failed,
                ))
            } else {
                None
            }
        }
    }

    type BackendRun = (EndReason, ReceiveState, BackendState);
    const FAILED: BackendRun = (
        EndReason::Failed,
        ReceiveState::Failed,
        BackendState::Failed,
    );
    const CRASH_SUPERVISION_QUALIFIED: bool = false;
    const STOP_TIMEOUT: Duration = Duration::from_secs(2);

    // Cancellation and commit admission share one atomic state. A signal that
    // wins before admission closes every attempt; once RENAME_NOREPLACE has
    // succeeded, publication wins and a later signal cannot undo the rename.
    const COMMIT_OPEN: u8 = 0;
    const COMMIT_ADMITTED: u8 = 1;
    const COMMIT_REVOKED: u8 = 2;
    const COMMIT_DONE: u8 = 3;

    #[derive(Default)]
    struct CommitGate(std::sync::atomic::AtomicU8);

    impl CommitGate {
        fn revoke(&self) {
            loop {
                let state = self.0.load(Ordering::SeqCst);
                if matches!(state, COMMIT_REVOKED | COMMIT_DONE) {
                    return;
                }
                if self
                    .0
                    .compare_exchange(state, COMMIT_REVOKED, Ordering::SeqCst, Ordering::SeqCst)
                    .is_ok()
                {
                    return;
                }
            }
        }

        fn is_revoked(&self) -> bool {
            self.0.load(Ordering::SeqCst) == COMMIT_REVOKED
        }

        fn admit_attempt(&self) -> io::Result<()> {
            loop {
                match self.0.load(Ordering::SeqCst) {
                    COMMIT_OPEN => {
                        if self
                            .0
                            .compare_exchange(
                                COMMIT_OPEN,
                                COMMIT_ADMITTED,
                                Ordering::SeqCst,
                                Ordering::SeqCst,
                            )
                            .is_ok()
                        {
                            return Ok(());
                        }
                    }
                    COMMIT_ADMITTED => return Ok(()),
                    COMMIT_REVOKED | COMMIT_DONE => {
                        return Err(io::Error::new(
                            io::ErrorKind::Interrupted,
                            "publication authorization revoked",
                        ));
                    }
                    _ => unreachable!("invalid commit gate state"),
                }
            }
        }

        fn published(&self) {
            self.0.store(COMMIT_DONE, Ordering::SeqCst);
        }
    }

    /// This capability cannot be constructed from serialized observations: it
    /// borrows the live containment reservation, session, quota lease, and
    /// staging identity for the entire finalization call.
    struct PublicationSeal<'a, 'b, C: Controller, Q: QuotaLease> {
        containment: &'a mut C,
        session: &'a C::Session,
        quota: &'a mut Q,
        staging_path: &'a Path,
        admission: &'a Admission<'b>,
    }

    impl<C: Controller, Q: QuotaLease> PublicationSeal<'_, '_, C, Q> {
        fn authorize(&mut self) -> io::Result<()> {
            if !self.admission.consent_valid || self.admission.interrupted().is_some() {
                return Err(io::Error::new(
                    io::ErrorKind::Interrupted,
                    "consent, cancellation, or deadline no longer authorizes publication",
                ));
            }
            if self.containment.populated(self.session) != Ok(false)
                || self.quota.revalidate().is_err()
                || self.quota.verify_directory(self.staging_path).is_err()
            {
                return Err(io::Error::other(
                    "live containment or quota reservation could not be verified",
                ));
            }
            // Last bounded operation before the publication syscall.
            self.admission.commit_gate.admit_attempt()
        }
    }

    #[derive(Debug)]
    struct Completion {
        run: BackendRun,
        files: u64,
        bytes: u64,
        cleanup: &'static str,
    }

    impl Completion {
        fn retained(run: BackendRun) -> Self {
            let run = if run.1 == ReceiveState::Completed {
                FAILED
            } else {
                run
            };
            Self {
                run,
                files: 0,
                bytes: 0,
                cleanup: "incomplete",
            }
        }

        fn no_resources(run: BackendRun) -> Self {
            Self {
                cleanup: "complete",
                ..Self::retained(run)
            }
        }
    }

    /// Own the lease from before staging creation through tree settlement.
    /// Injection is internal. Neither qualification gate depends on it.
    fn coordinate<C: Controller, Q: QuotaLease, W: io::Write + Send + 'static>(
        admission: Admission<'_>,
        acquire: impl FnOnce() -> Result<C, ContainmentError>,
        acquire_quota: impl FnOnce() -> Result<Q, QuotaError>,
        launch: impl FnOnce(&C::Session, &ReceiveStaging, &ReceiveSessionState) -> io::Result<Child>,
        terminal: impl Fn() -> io::Result<W>,
        output_limit: usize,
    ) -> Result<Completion, (BlockReason, Completion)> {
        if let Some(run) = admission.interrupted() {
            return Ok(Completion::no_resources(run));
        }
        let mut lease =
            acquire().map_err(|_| (BlockReason::BackendFailed, Completion::retained(FAILED)))?;
        let session = lease
            .create()
            .map_err(|_| (BlockReason::BackendFailed, Completion::retained(FAILED)))?;
        let mut quota = match acquire_quota() {
            Ok(quota) => quota,
            Err(_) => {
                let clean = lease
                    .cleanup(session)
                    .and_then(|()| lease.release())
                    .is_ok();
                let completion = if clean {
                    Completion::no_resources(FAILED)
                } else {
                    Completion::retained(FAILED)
                };
                return Err((BlockReason::PermissionUnavailable, completion));
            }
        };

        // Prepare both private resources before exec, with retention armed
        // before an untrusted process can ever see either path.
        let resources = ReceiveStaging::create(admission.destination)
            .map_err(|_| BlockReason::DestinationUnavailable)
            .and_then(|staging| {
                quota
                    .verify_directory(staging.path())
                    .map_err(|_| BlockReason::PermissionUnavailable)?;
                let runtime = ReceiveSessionState::create_in(quota.storage_root())
                    .map_err(|_| BlockReason::PermissionUnavailable)?;
                quota
                    .verify_directory(runtime.path())
                    .map_err(|_| BlockReason::PermissionUnavailable)?;
                Ok((staging, runtime))
            });
        let (mut staging, mut runtime) = match resources {
            Ok(resources) => resources,
            Err(reason) => {
                let containment_clean = lease
                    .cleanup(session)
                    .and_then(|()| lease.release())
                    .is_ok();
                let quota_clean = quota.release().is_ok();
                let completion = if containment_clean && quota_clean {
                    Completion::no_resources(FAILED)
                } else {
                    Completion::retained(FAILED)
                };
                return Err((reason, completion));
            }
        };
        staging.retain_until_quiescent();
        runtime.retain_until_quiescent();

        let output = Arc::new(RelayStatus::default());
        let mut relays = Vec::new();
        let mut child = None;
        let mut run = if let Some(run) = admission.interrupted() {
            run
        } else if let Ok(process) = launch(&session, &staging, &runtime) {
            let process = child.insert(process);
            match (
                process.stdout.take(),
                process.stderr.take(),
                terminal(),
                terminal(),
            ) {
                (Some(stdout), Some(stderr), Ok(out), Ok(err)) => {
                    relays.extend(
                        [
                            spawn_terminal_relay(stdout, out, &output, output_limit),
                            spawn_terminal_relay(stderr, err, &output, output_limit),
                        ]
                        .into_iter()
                        .flatten(),
                    );
                    poll_backend(
                        process,
                        &staging,
                        &admission,
                        &output.exceeded,
                        &output.failed,
                    )
                }
                _ => FAILED,
            }
        } else {
            FAILED
        };

        // Terminate the subtree before waiting for inherited pipe writers.
        // A direct child status can never establish recursive emptiness.
        let quiescence = containment::quiesce(
            &mut lease,
            &session,
            run.1 == ReceiveState::Completed,
            STOP_TIMEOUT,
        );
        let reaped = child.as_mut().is_none_or(reap_backend);
        let relay_deadline = Instant::now() + STOP_TIMEOUT;
        let mut relays_joined = true;
        for relay in relays {
            relays_joined &= join_relay(relay, relay_deadline, &output.stop);
        }
        run = admission.interrupted().unwrap_or_else(|| {
            if output.exceeded.load(Ordering::Relaxed) {
                (
                    EndReason::OutputLimitExceeded,
                    ReceiveState::Failed,
                    BackendState::Failed,
                )
            } else if output.failed.load(Ordering::Relaxed) || !relays_joined {
                FAILED
            } else {
                run
            }
        });
        let Ok(normal_exit) = quiescence else {
            return Ok(Completion::retained(run));
        };
        if !reaped || !relays_joined {
            return Ok(Completion::retained(run));
        }

        // Confirm emptiness again after reaping and relay completion. A
        // change since settlement is an ownership or observation failure.
        if lease.populated(&session) != Ok(false) {
            let _ = containment::quiesce(&mut lease, &session, false, STOP_TIMEOUT);
            return Ok(Completion::retained(run));
        }

        if quota.revalidate().is_err()
            || quota.verify_directory(staging.path()).is_err()
            || quota.verify_directory(runtime.path()).is_err()
        {
            return Ok(Completion::retained(run));
        }
        run = admission.interrupted().unwrap_or(run);
        let eligible = publication_eligible(
            run.0,
            run.1,
            run.2,
            admission.cancelled,
            &output.exceeded,
            &output.failed,
            normal_exit,
        );
        if eligible && !CRASH_SUPERVISION_QUALIFIED {
            // The session is clean, but there is no qualified external
            // authority to publish. Retain the staging/runtime and quota
            // reservation. Release containment only after this failure
            // decision has been finalized.
            staging.preserve();
            if lease
                .cleanup(session)
                .and_then(|()| lease.release())
                .is_err()
            {
                return Ok(Completion::retained(run));
            }
            return Ok(Completion::retained(run));
        }
        if runtime.discard().is_err() {
            return Ok(Completion::retained(run));
        }

        // Do not release containment or quota before the final staging scan,
        // per-attempt authorization and atomic rename (or definitive error).
        let (files, bytes, cleanup) = if eligible {
            let staging_path = staging.path().to_path_buf();
            let mut seal = PublicationSeal {
                containment: &mut lease,
                session: &session,
                quota: &mut quota,
                staging_path: &staging_path,
                admission: &admission,
            };
            match staging.finalize_with(|| seal.authorize()) {
                Ok(result) => {
                    admission.commit_gate.published();
                    (result.files, result.bytes, "complete")
                }
                Err(_) => (0, 0, "incomplete"),
            }
        } else {
            finish_staging(staging, false, true)
        };
        // A failure to finalize is definitive only after the finalizer has
        // returned; now the writer boundary may be settled and released.
        if lease
            .cleanup(session)
            .and_then(|()| lease.release())
            .is_err()
        {
            return Ok(Completion::retained(run));
        }
        if cleanup != "complete" || quota.revalidate().is_err() {
            return Ok(Completion::retained(run));
        }
        if quota.release().is_err() {
            return Ok(if files == 0 {
                Completion::retained(run)
            } else {
                Completion {
                    run: FAILED,
                    files,
                    bytes,
                    cleanup: "incomplete",
                }
            });
        }
        if run.1 == ReceiveState::Completed && (!eligible || cleanup != "complete") {
            run = FAILED;
        }
        Ok(Completion {
            run,
            files,
            bytes,
            cleanup,
        })
    }

    #[derive(Default)]
    struct RelayStatus {
        bytes: AtomicUsize,
        exceeded: AtomicBool,
        failed: AtomicBool,
        stop: AtomicBool,
    }

    fn report_completion(completion: Completion) -> ExitCode {
        let (end_reason, _, _) = completion.run;
        let cleanup = completion.cleanup;
        let block_reason = match end_reason {
            EndReason::Cancelled => Some(BlockReason::Cancelled),
            EndReason::Expired => Some(BlockReason::SessionExpired),
            EndReason::OutputLimitExceeded => Some(BlockReason::OutputLimitExceeded),
            EndReason::Failed | EndReason::WriterContainmentUnavailable => {
                Some(BlockReason::BackendFailed)
            }
            EndReason::BackendExited if cleanup != "complete" => Some(BlockReason::BackendFailed),
            EndReason::BackendExited => None,
        };
        emit_completion(completion, block_reason)
    }

    fn emit_completion(completion: Completion, block_reason: Option<BlockReason>) -> ExitCode {
        let (end_reason, state, child_status) = completion.run;
        let Completion {
            files,
            bytes,
            cleanup,
            ..
        } = completion;
        let mut lifecycle = ReceiveLifecycle::default();
        let _ = lifecycle.transition(ReceiveState::Preparing);
        let _ = lifecycle.transition(ReceiveState::Active);
        match state {
            ReceiveState::Cancelled | ReceiveState::Expired => {
                let _ = lifecycle.transition(ReceiveState::Cancelling);
                let _ = lifecycle.transition(state);
            }
            _ => {
                let _ = lifecycle.transition(state);
            }
        }
        emit(&ReceiveResult {
            schema_version: 1,
            kind: "airdrop_receive_result",
            state: lifecycle.state(),
            end_reason,
            backend: "owly_cli_0_4_2",
            backend_process: child_status,
            control_plane: PlaneState::Unknown,
            advertisement: PlaneState::Unknown,
            data_plane: PlaneState::Unknown,
            transfer_outcome: BackendState::Unknown,
            completed_file_count: files,
            completed_bytes: bytes,
            cleanup,
            block_reason,
            authorization: "never_evaluated",
            physical_qualification: "not_qualified",
        });
        if cleanup == "complete" && matches!(end_reason, EndReason::BackendExited) {
            ExitCode::SUCCESS
        } else {
            ExitCode::from(1)
        }
    }

    fn publication_eligible(
        reason: EndReason,
        state: ReceiveState,
        backend: BackendState,
        cancelled: &AtomicBool,
        exceeded: &AtomicBool,
        failed: &AtomicBool,
        writers_quiescent: bool,
    ) -> bool {
        matches!(reason, EndReason::BackendExited)
            && state == ReceiveState::Completed
            && backend == BackendState::Available
            && !cancelled.load(Ordering::Relaxed)
            && !exceeded.load(Ordering::Relaxed)
            && !failed.load(Ordering::Relaxed)
            && writers_quiescent
    }

    fn finish_staging(
        staging: ReceiveStaging,
        eligible: bool,
        writers_quiescent: bool,
    ) -> (u64, u64, &'static str) {
        if !writers_quiescent {
            staging.preserve();
            return (0, 0, "incomplete");
        }
        if !eligible {
            return (
                0,
                0,
                if staging.discard().is_ok() {
                    "complete"
                } else {
                    "incomplete"
                },
            );
        }
        #[cfg(test)]
        {
            match staging.finalize() {
                Ok(result) => (result.files, result.bytes, "complete"),
                Err(_) => (0, 0, "incomplete"),
            }
        }
        #[cfg(not(test))]
        {
            // Production publication is only possible through the sealed
            // coordinator path, never this failure-settlement helper.
            staging.preserve();
            (0, 0, "incomplete")
        }
    }

    fn poll_backend(
        child: &mut std::process::Child,
        staging: &ReceiveStaging,
        admission: &Admission<'_>,
        output_exceeded: &AtomicBool,
        output_failed: &AtomicBool,
    ) -> (EndReason, ReceiveState, BackendState) {
        loop {
            if output_exceeded.load(Ordering::Relaxed) {
                return (
                    EndReason::OutputLimitExceeded,
                    ReceiveState::Failed,
                    BackendState::Failed,
                );
            }
            if output_failed.load(Ordering::Relaxed) {
                return (
                    EndReason::Failed,
                    ReceiveState::Failed,
                    BackendState::Failed,
                );
            }
            if let Some(run) = admission.interrupted() {
                return run;
            }
            match child.try_wait() {
                Ok(Some(status)) => {
                    return if status.success() {
                        (
                            EndReason::BackendExited,
                            ReceiveState::Completed,
                            BackendState::Available,
                        )
                    } else {
                        (
                            EndReason::Failed,
                            ReceiveState::Failed,
                            BackendState::Failed,
                        )
                    };
                }
                Ok(None) => {}
                Err(_) => {
                    return (
                        EndReason::Failed,
                        ReceiveState::Failed,
                        BackendState::Failed,
                    );
                }
            }
            if staging.usage().is_err() {
                return (
                    EndReason::OutputLimitExceeded,
                    ReceiveState::Failed,
                    BackendState::Failed,
                );
            }
            std::thread::sleep(Duration::from_millis(100));
        }
    }

    fn writer_containment_available() -> bool {
        // Deliberately fail closed on every host; do not probe or modify host
        // cgroups, infer confinement from /proc, or silently require root.
        false
    }

    fn reap_backend(child: &mut Child) -> bool {
        let deadline = Instant::now() + STOP_TIMEOUT;
        loop {
            match child.try_wait() {
                Ok(Some(_)) => return true,
                Err(_) => return false,
                Ok(None) if Instant::now() >= deadline => return false,
                Ok(None) => std::thread::sleep(Duration::from_millis(10)),
            }
        }
    }

    #[cfg(test)]
    fn supervise(
        child: &mut Child,
        staging: &ReceiveStaging,
        deadline: Instant,
        cancelled: &AtomicBool,
        output_exceeded: &AtomicBool,
        output_failed: &AtomicBool,
    ) -> BackendRun {
        let commit_gate = CommitGate::default();
        let admission = Admission {
            destination: staging.path(),
            cancelled,
            commit_gate: &commit_gate,
            consent_valid: true,
            deadline,
            now: &Instant::now,
        };
        let result = poll_backend(child, staging, &admission, output_exceeded, output_failed);
        // Legacy fixtures own one direct child. The coordinator uses the
        // cgroup controller for termination instead.
        if child.try_wait().is_ok_and(|status| status.is_none()) {
            let _ = child.kill();
            let _ = reap_backend(child);
        }
        result
    }

    fn join_relay(
        relay: std::thread::JoinHandle<io::Result<()>>,
        deadline: Instant,
        stop: &AtomicBool,
    ) -> bool {
        while !relay.is_finished() {
            if Instant::now() >= deadline {
                stop.store(true, Ordering::Relaxed);
                return false;
            }
            std::thread::sleep(Duration::from_millis(10));
        }
        relay.join().is_ok_and(|result| result.is_ok())
    }

    // Child pipe read ends are private to the supervisor. Nonblocking reads
    // let us stop draining even if a pipe descriptor escaped the subtree.
    struct RelayReader<R> {
        pipe: R,
        status: Arc<RelayStatus>,
    }

    impl<R: io::Read> io::Read for RelayReader<R> {
        fn read(&mut self, buffer: &mut [u8]) -> io::Result<usize> {
            while !self.status.stop.load(Ordering::Relaxed) {
                match self.pipe.read(buffer) {
                    Err(error) if error.kind() == io::ErrorKind::WouldBlock => {
                        std::thread::sleep(Duration::from_millis(10));
                    }
                    result => return result,
                }
            }
            Err(io::Error::from(io::ErrorKind::Interrupted))
        }
    }

    fn spawn_terminal_relay(
        reader: impl io::Read + AsRawFd + Send + 'static,
        writer: impl io::Write + Send + 'static,
        output: &Arc<RelayStatus>,
        limit: usize,
    ) -> Option<std::thread::JoinHandle<io::Result<()>>> {
        let fd = reader.as_raw_fd();
        let flags = unsafe { libc::fcntl(fd, libc::F_GETFL) };
        if flags < 0 || unsafe { libc::fcntl(fd, libc::F_SETFL, flags | libc::O_NONBLOCK) } < 0 {
            output.failed.store(true, Ordering::Relaxed);
            return None;
        }
        let status = output.clone();
        match std::thread::Builder::new().spawn(move || {
            let reader = RelayReader {
                pipe: reader,
                status: status.clone(),
            };
            let result = process::relay_bounded(
                reader,
                writer,
                &status.bytes,
                &status.exceeded,
                &status.failed,
                limit,
            );
            if result.is_err() {
                status.failed.store(true, Ordering::Relaxed);
            }
            result
        }) {
            Ok(relay) => Some(relay),
            Err(_) => {
                output.failed.store(true, Ordering::Relaxed);
                None
            }
        }
    }

    fn open_nonblocking_terminal(fd: RawFd) -> io::Result<File> {
        if unsafe { libc::isatty(fd) } != 1 {
            return Err(io::Error::new(
                io::ErrorKind::NotConnected,
                "terminal output is unavailable",
            ));
        }
        let path = format!("/proc/self/fd/{fd}");
        let writer = OpenOptions::new()
            .write(true)
            .custom_flags(libc::O_CLOEXEC | libc::O_NONBLOCK)
            .open(path)?;
        if unsafe { libc::isatty(writer.as_raw_fd()) } != 1 {
            return Err(io::Error::new(
                io::ErrorKind::NotConnected,
                "terminal output is unavailable",
            ));
        }
        Ok(writer)
    }

    fn install_child_limits(command: &mut Command) {
        // SAFETY: this child-only hook performs async-signal-safe libc calls
        // before exec; it does not touch parent process or radio state.
        unsafe {
            command.pre_exec(|| {
                if libc::setsid() < 0 {
                    return Err(io::Error::last_os_error());
                }
                let limit = libc::rlimit {
                    rlim_cur: storage::MAX_RECEIVE_FILE_BYTES as libc::rlim_t,
                    rlim_max: storage::MAX_RECEIVE_FILE_BYTES as libc::rlim_t,
                };
                if libc::setrlimit(libc::RLIMIT_FSIZE, &limit) != 0 {
                    return Err(io::Error::last_os_error());
                }
                Ok(())
            });
        }
    }

    fn print_blocked(reason: BlockReason, end_reason: EndReason, state: ReceiveState) -> ExitCode {
        emit(&ReceiveResult {
            schema_version: 1,
            kind: "airdrop_receive_result",
            state,
            end_reason,
            backend: "owly_cli_0_4_2",
            backend_process: BackendState::Blocked,
            control_plane: PlaneState::Unknown,
            advertisement: PlaneState::Unknown,
            data_plane: PlaneState::Unknown,
            transfer_outcome: BackendState::Unknown,
            completed_file_count: 0,
            completed_bytes: 0,
            cleanup: "not_started",
            block_reason: Some(reason),
            authorization: "never_evaluated",
            physical_qualification: "not_qualified",
        });
        ExitCode::from(1)
    }

    fn emit(result: &ReceiveResult) {
        if let Ok(json) = serde_json::to_string(result) {
            println!("{json}");
        }
    }

    #[cfg(test)]
    mod tests {
        use super::*;
        use std::process::{Command, Stdio};

        #[derive(Clone, Copy, Default, PartialEq, Eq)]
        enum CrashPoint {
            #[default]
            None,
            Startup,
            Active,
            Finalization,
        }

        #[derive(Default)]
        struct Scenario {
            script: &'static str,
            placement_fails: bool,
            cancel: bool,
            expire: bool,
            relay_fails: bool,
            limited: bool,
            observations: Vec<Result<bool, ContainmentError>>,
            kill_fails: bool,
            stuck: bool,
            cleanup_fails: bool,
            release_fails: bool,
            quota_acquire_fails: bool,
            quota_verify_fails: bool,
            quota_revalidate_fails: bool,
            quota_release_fails: bool,
            crash: CrashPoint,
        }

        struct FixtureLease<'a> {
            marker: PathBuf,
            events: &'a std::cell::RefCell<Vec<&'static str>>,
            scenario: &'a Scenario,
            observations: std::vec::IntoIter<Result<bool, ContainmentError>>,
        }

        impl Controller for FixtureLease<'_> {
            type Session = ();

            fn create(&mut self) -> Result<(), ContainmentError> {
                self.events.borrow_mut().push("create");
                crash_at(self.scenario.crash, CrashPoint::Startup);
                Ok(())
            }

            fn populated(&mut self, (): &()) -> Result<bool, ContainmentError> {
                self.events.borrow_mut().push("observe");
                self.observations.next().unwrap_or(Ok(self.scenario.stuck))
            }

            fn kill(&mut self, (): &()) -> Result<(), ContainmentError> {
                self.events.borrow_mut().push("kill");
                if self.scenario.kill_fails {
                    return Err(ContainmentError::KillFailed);
                }
                Ok(())
            }

            fn cleanup(&mut self, (): ()) -> Result<(), ContainmentError> {
                self.events.borrow_mut().push("cleanup");
                crash_at(self.scenario.crash, CrashPoint::Finalization);
                if self.scenario.cleanup_fails {
                    return Err(ContainmentError::CleanupFailed);
                }
                Ok(())
            }

            fn release(&mut self) -> Result<(), ContainmentError> {
                self.events.borrow_mut().push("release");
                if self.scenario.release_fails {
                    return Err(ContainmentError::CleanupFailed);
                }
                std::fs::remove_dir(&self.marker).unwrap();
                Ok(())
            }
        }

        struct FixtureQuotaLease<'a> {
            root: PathBuf,
            marker: PathBuf,
            events: &'a std::cell::RefCell<Vec<&'static str>>,
            scenario: &'a Scenario,
        }

        impl QuotaLease for FixtureQuotaLease<'_> {
            fn storage_root(&self) -> &Path {
                &self.root
            }

            fn verify_directory(&self, directory: &Path) -> Result<(), QuotaError> {
                self.events.borrow_mut().push("quota_verify");
                let contained = std::fs::canonicalize(directory)
                    .ok()
                    .zip(std::fs::canonicalize(&self.root).ok())
                    .is_some_and(|(directory, root)| directory.starts_with(root));
                if self.scenario.quota_verify_fails || !contained {
                    Err(QuotaError::ProjectMismatch)
                } else {
                    Ok(())
                }
            }

            fn revalidate(&self) -> Result<QuotaSnapshot, QuotaError> {
                self.events.borrow_mut().push("quota_revalidate");
                if self.scenario.quota_revalidate_fails {
                    Err(QuotaError::EnforcementUnknown)
                } else {
                    Ok(QuotaSnapshot {
                        hard_bytes: 1024,
                        used_bytes: 7,
                        hard_inodes: 16,
                        used_inodes: 2,
                        enforcement: true,
                    })
                }
            }

            fn release(&mut self) -> Result<(), QuotaError> {
                self.events.borrow_mut().push("quota_release");
                if self.scenario.quota_release_fails {
                    Err(QuotaError::CleanupFailed)
                } else {
                    std::fs::remove_dir(&self.marker).map_err(|_| QuotaError::CleanupFailed)
                }
            }
        }

        fn crash_at(selected: CrashPoint, current: CrashPoint) {
            if selected == current {
                // Exit without unwinding to model abrupt supervisor loss.
                std::process::exit(86);
            }
        }

        struct FixtureTerminal(bool);

        impl io::Write for FixtureTerminal {
            fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
                if self.0 {
                    Err(io::Error::other("fixture terminal write failure"))
                } else {
                    Ok(bytes.len())
                }
            }

            fn flush(&mut self) -> io::Result<()> {
                Ok(())
            }
        }

        fn fixture_session(root: &Path, scenario: &Scenario) -> (Completion, Vec<&'static str>) {
            let cancelled = AtomicBool::new(false);
            let expired = std::cell::Cell::new(false);
            let origin = Instant::now();
            let deadline = origin + Duration::from_secs(30);
            let clock = || if expired.get() { deadline } else { origin };
            let destination = root.join("dest");
            let commit_gate = CommitGate::default();
            let events = std::cell::RefCell::new(Vec::new());
            let result = coordinate(
                Admission {
                    destination: &destination,
                    cancelled: &cancelled,
                    commit_gate: &commit_gate,
                    consent_valid: true,
                    deadline,
                    now: &clock,
                },
                || {
                    events.borrow_mut().push("acquire");
                    let marker = root.join("owner");
                    std::fs::create_dir(&marker)
                        .map_err(|_| ContainmentError::InvalidDelegation)?;
                    Ok(FixtureLease {
                        marker,
                        events: &events,
                        scenario,
                        observations: scenario.observations.clone().into_iter(),
                    })
                },
                || {
                    events.borrow_mut().push("quota_acquire");
                    if scenario.quota_acquire_fails {
                        return Err(QuotaError::QuotaUnavailable);
                    }
                    let marker = destination.join("quota-owner");
                    std::fs::create_dir(&marker).map_err(|_| QuotaError::OwnershipConflict)?;
                    Ok(FixtureQuotaLease {
                        root: destination.clone(),
                        marker,
                        events: &events,
                        scenario,
                    })
                },
                |(), staging, runtime| {
                    events.borrow_mut().push("launch");
                    if scenario.placement_fails {
                        return Err(io::Error::from(io::ErrorKind::PermissionDenied));
                    }
                    std::fs::write(staging.path().join("payload"), b"fixture").unwrap();
                    std::fs::write(runtime.path().join("identity"), b"fixture").unwrap();
                    crash_at(scenario.crash, CrashPoint::Active);
                    let child = Command::new("/bin/sh")
                        .args(["-c", scenario.script])
                        .stdin(Stdio::null())
                        .stdout(Stdio::piped())
                        .stderr(Stdio::piped())
                        .spawn()?;
                    cancelled.store(scenario.cancel, Ordering::Relaxed);
                    expired.set(scenario.expire);
                    Ok(child)
                },
                || Ok(FixtureTerminal(scenario.relay_fails)),
                if scenario.limited {
                    1
                } else {
                    MAX_RECEIVE_TERMINAL_OUTPUT
                },
            );
            let completion = result.unwrap_or_else(|(_, completion)| completion);
            assert_eq!((completion.files, completion.bytes), (0, 0));
            assert!(std::fs::read_dir(destination).unwrap().all(|entry| {
                !entry
                    .unwrap()
                    .file_name()
                    .to_string_lossy()
                    .starts_with("AirDrop Receive ")
            }));
            (completion, events.into_inner())
        }

        fn fixture_root() -> tempfile::TempDir {
            let root = tempfile::tempdir().unwrap();
            let mut destination = std::fs::DirBuilder::new();
            std::os::unix::fs::DirBuilderExt::mode(&mut destination, 0o700);
            destination.create(root.path().join("dest")).unwrap();
            root
        }

        fn private_resource_counts(root: &Path) -> (usize, usize) {
            let mut staging = 0;
            let mut runtime = 0;
            for entry in std::fs::read_dir(root.join("dest")).unwrap() {
                let name = entry.unwrap().file_name();
                let name = name.to_string_lossy();
                staging += usize::from(name.starts_with(".opencontinuity-airdrop-"));
                runtime += usize::from(name.starts_with("opencontinuity-airdrop-state-"));
            }
            (staging, runtime)
        }

        #[test]
        fn coordinator_success_retains_quota_reservation_and_remains_nonpublishable() {
            let root = fixture_root();
            let (completion, events) = fixture_session(root.path(), &Scenario::default());
            assert_eq!(completion.run, FAILED);
            assert_eq!(completion.cleanup, "incomplete");
            assert_eq!(
                events,
                [
                    "acquire",
                    "create",
                    "quota_acquire",
                    "quota_verify",
                    "quota_verify",
                    "launch",
                    "observe",
                    "observe",
                    "quota_revalidate",
                    "quota_verify",
                    "quota_verify",
                    "cleanup",
                    "release",
                ]
            );
            assert!(!root.path().join("owner").exists());
            assert!(root.path().join("dest/quota-owner").exists());
            assert_eq!(private_resource_counts(root.path()), (1, 1));

            let (retry, retry_events) = fixture_session(root.path(), &Scenario::default());
            assert_eq!(retry.cleanup, "complete");
            assert_eq!(
                retry_events,
                ["acquire", "create", "quota_acquire", "cleanup", "release"]
            );
            assert_eq!(private_resource_counts(root.path()), (1, 1));
        }

        #[test]
        fn publication_seal_keeps_live_reservations_through_final_rename() {
            let root = fixture_root();
            let destination = root.path().join("dest");
            let cgroup_marker = root.path().join("owner");
            let quota_marker = destination.join("quota-owner");
            std::fs::create_dir(&cgroup_marker).unwrap();
            std::fs::create_dir(&quota_marker).unwrap();
            let events = std::cell::RefCell::new(Vec::new());
            let scenario = Scenario {
                observations: vec![Ok(false)],
                ..Default::default()
            };
            let mut containment = FixtureLease {
                marker: cgroup_marker.clone(),
                events: &events,
                scenario: &scenario,
                observations: scenario.observations.clone().into_iter(),
            };
            let session = ();
            let mut quota = FixtureQuotaLease {
                root: destination.clone(),
                marker: quota_marker.clone(),
                events: &events,
                scenario: &scenario,
            };
            let cancelled = AtomicBool::new(false);
            let commit_gate = CommitGate::default();
            let deadline = Instant::now() + Duration::from_secs(30);
            let admission = Admission {
                destination: &destination,
                cancelled: &cancelled,
                commit_gate: &commit_gate,
                consent_valid: true,
                deadline,
                now: &Instant::now,
            };
            let staging = storage::ReceiveStaging::create(&destination).unwrap();
            std::fs::write(staging.path().join("received.bin"), b"payload").unwrap();
            let staging_path = staging.path().to_path_buf();
            let finalized = {
                let mut seal = PublicationSeal {
                    containment: &mut containment,
                    session: &session,
                    quota: &mut quota,
                    staging_path: &staging_path,
                    admission: &admission,
                };
                staging.finalize_with(|| seal.authorize()).unwrap()
            };
            commit_gate.published();
            assert_eq!(finalized.files, 1);
            assert_eq!(commit_gate.0.load(Ordering::SeqCst), COMMIT_DONE);
            assert!(cgroup_marker.exists());
            assert!(quota_marker.exists());
            assert!(std::fs::read_dir(&destination).unwrap().any(|entry| {
                entry
                    .unwrap()
                    .file_name()
                    .to_string_lossy()
                    .starts_with("AirDrop Receive ")
            }));
            containment.cleanup(session).unwrap();
            containment.release().unwrap();
            quota.release().unwrap();
            assert!(!cgroup_marker.exists());
            assert!(!quota_marker.exists());
        }

        #[test]
        fn cancellation_and_commit_admission_have_one_atomic_winner() {
            let cancelled_first = CommitGate::default();
            cancelled_first.revoke();
            assert!(cancelled_first.admit_attempt().is_err());

            let commit_first = CommitGate::default();
            commit_first.admit_attempt().unwrap();
            commit_first.revoke();
            assert!(commit_first.is_revoked());
            // The caller resolves an in-flight attempt from the rename result.
            // A successful no-replace rename is irreversible and wins.
            commit_first.published();
            commit_first.revoke();
            assert_eq!(commit_first.0.load(Ordering::SeqCst), COMMIT_DONE);
        }

        #[test]
        fn coordinator_failed_execution_settles_then_discards() {
            for (scenario, reason) in [
                (
                    Scenario {
                        placement_fails: true,
                        ..Default::default()
                    },
                    EndReason::Failed,
                ),
                (
                    Scenario {
                        script: "exit 7",
                        ..Default::default()
                    },
                    EndReason::Failed,
                ),
                (
                    Scenario {
                        cancel: true,
                        ..Default::default()
                    },
                    EndReason::Cancelled,
                ),
                (
                    Scenario {
                        expire: true,
                        ..Default::default()
                    },
                    EndReason::Expired,
                ),
                (
                    Scenario {
                        script: "printf abc",
                        limited: true,
                        ..Default::default()
                    },
                    EndReason::OutputLimitExceeded,
                ),
                (
                    Scenario {
                        script: "printf abc",
                        relay_fails: true,
                        ..Default::default()
                    },
                    EndReason::Failed,
                ),
            ] {
                let root = fixture_root();
                let (completion, events) = fixture_session(root.path(), &scenario);
                assert_eq!(completion.run.0, reason);
                assert_eq!(completion.cleanup, "complete");
                assert_eq!(private_resource_counts(root.path()), (0, 0));
                assert!(events.ends_with(&[
                    "cleanup",
                    "release",
                    "quota_revalidate",
                    "quota_release"
                ]));
                if scenario.placement_fails || scenario.cancel || scenario.expire {
                    assert!(events.contains(&"kill"));
                }
            }
        }

        #[test]
        fn coordinator_quota_uncertainty_blocks_and_retains_when_needed() {
            for scenario in [
                Scenario {
                    quota_acquire_fails: true,
                    ..Default::default()
                },
                Scenario {
                    quota_verify_fails: true,
                    ..Default::default()
                },
            ] {
                let root = fixture_root();
                let (completion, events) = fixture_session(root.path(), &scenario);
                assert_eq!(completion.run, FAILED);
                assert_eq!(completion.cleanup, "complete");
                assert_eq!(private_resource_counts(root.path()), (0, 0));
                assert!(!root.path().join("dest/quota-owner").exists());
                assert!(events.contains(&"quota_acquire"));
            }

            for (scenario, retained) in [
                (
                    Scenario {
                        script: "exit 7",
                        quota_revalidate_fails: true,
                        ..Default::default()
                    },
                    (1, 1),
                ),
                (
                    Scenario {
                        script: "exit 7",
                        quota_release_fails: true,
                        ..Default::default()
                    },
                    (0, 0),
                ),
            ] {
                let root = fixture_root();
                let (completion, _) = fixture_session(root.path(), &scenario);
                assert_eq!(completion.run, FAILED);
                assert_eq!(completion.cleanup, "incomplete");
                assert_eq!(private_resource_counts(root.path()), retained);
                assert!(root.path().join("dest/quota-owner").exists());
            }
        }

        #[test]
        fn coordinator_reuses_quota_after_safe_failed_session() {
            let root = fixture_root();
            for _ in 0..2 {
                let (completion, _) = fixture_session(
                    root.path(),
                    &Scenario {
                        script: "exit 7",
                        ..Default::default()
                    },
                );
                assert_eq!(completion.cleanup, "complete");
                assert_eq!(private_resource_counts(root.path()), (0, 0));
                assert!(!root.path().join("dest/quota-owner").exists());
            }
        }

        #[test]
        fn coordinator_surviving_or_migrated_descendants_require_tree_kill() {
            let root = fixture_root();
            let (completion, events) = fixture_session(
                root.path(),
                &Scenario {
                    // The delegation stays populated after child exit. Its
                    // writer may have moved out of the session into a sibling.
                    observations: vec![Ok(true), Ok(true), Ok(false), Ok(false)],
                    ..Default::default()
                },
            );
            assert_eq!(completion.run, FAILED);
            assert_eq!(completion.cleanup, "complete");
            assert!(
                events
                    .windows(5)
                    .any(|events| events == ["observe", "kill", "observe", "observe", "observe"])
            );
            assert!(events.ends_with(&["cleanup", "release", "quota_revalidate", "quota_release"]));
            assert_eq!(private_resource_counts(root.path()), (0, 0));
        }

        #[test]
        fn coordinator_uncertain_tree_or_cleanup_retains_both_private_resources() {
            for scenario in [
                Scenario {
                    observations: vec![Ok(true)],
                    kill_fails: true,
                    ..Default::default()
                },
                Scenario {
                    observations: vec![Err(ContainmentError::ObservationFailed)],
                    ..Default::default()
                },
                Scenario {
                    stuck: true,
                    ..Default::default()
                },
                Scenario {
                    observations: vec![Ok(false), Ok(true)],
                    ..Default::default()
                },
                Scenario {
                    cleanup_fails: true,
                    ..Default::default()
                },
                Scenario {
                    release_fails: true,
                    ..Default::default()
                },
            ] {
                let root = fixture_root();
                let (completion, events) = fixture_session(root.path(), &scenario);
                assert_eq!(completion.run, FAILED);
                assert_eq!(completion.cleanup, "incomplete");
                assert_eq!(private_resource_counts(root.path()), (1, 1));
                assert!(root.path().join("owner").exists());
                if !scenario.cleanup_fails && !scenario.release_fails {
                    assert!(events.contains(&"kill"));
                    assert!(!events.contains(&"cleanup"));
                }
            }
        }

        #[test]
        fn coordinator_relay_deadline_stops_a_reader_without_pipe_eof() {
            let (reader, writer) = std::os::unix::net::UnixStream::pair().unwrap();
            let output = Arc::new(RelayStatus::default());
            let relay = spawn_terminal_relay(reader, io::sink(), &output, 16).unwrap();
            assert!(!join_relay(relay, Instant::now(), &output.stop));
            // Retain the writer until the reader has observed the stop
            // request. EOF cannot account for completion in this test.
            let deadline = Instant::now() + STOP_TIMEOUT;
            while !output.failed.load(Ordering::Relaxed) && Instant::now() < deadline {
                std::thread::sleep(Duration::from_millis(10));
            }
            assert!(output.failed.load(Ordering::Relaxed));
            drop(writer);
        }

        #[test]
        fn coordinator_abrupt_exit_fixture() {
            let Ok(phase) = std::env::var("OC_P019D_FIXTURE_PHASE") else {
                return;
            };
            let root = PathBuf::from(std::env::var_os("OC_P019D_FIXTURE_ROOT").unwrap());
            let crash = match phase.as_str() {
                "startup" => CrashPoint::Startup,
                "active" => CrashPoint::Active,
                "finalization" => CrashPoint::Finalization,
                _ => panic!("unknown crash fixture phase"),
            };
            fixture_session(
                &root,
                &Scenario {
                    crash,
                    ..Default::default()
                },
            );
            panic!("fixture did not stop");
        }

        #[test]
        fn coordinator_crash_preserves_reservations_and_blocks_restart() {
            for (phase, retained) in [("startup", 0), ("active", 1), ("finalization", 1)] {
                let root = fixture_root();
                let exit = Command::new(std::env::current_exe().unwrap())
                    .args([
                        "--exact",
                        "airdrop::receive::tests::coordinator_abrupt_exit_fixture",
                    ])
                    .env("OC_P019D_FIXTURE_PHASE", phase)
                    .env("OC_P019D_FIXTURE_ROOT", root.path())
                    .stdout(Stdio::null())
                    .status()
                    .unwrap();
                assert_eq!(exit.code(), Some(86));
                assert!(root.path().join("owner").exists());
                assert_eq!(private_resource_counts(root.path()), (retained, retained));
                let (completion, events) = fixture_session(root.path(), &Scenario::default());
                assert_eq!(completion.cleanup, "incomplete");
                assert_eq!(events, ["acquire"]);
                assert_eq!(private_resource_counts(root.path()), (retained, retained));
            }
        }

        #[test]
        fn publication_boundary_crash_fixture() {
            let Ok(phase) = std::env::var("OC_P019G_PUBLICATION_CRASH_PHASE") else {
                return;
            };
            let root = PathBuf::from(std::env::var_os("OC_P019G_PUBLICATION_CRASH_ROOT").unwrap());
            let destination = root.join("dest");
            let containment_marker = root.join("owner");
            let quota_marker = destination.join("quota-owner");
            std::fs::create_dir(&containment_marker).unwrap();
            std::fs::create_dir(&quota_marker).unwrap();

            let events = std::cell::RefCell::new(Vec::new());
            let scenario = Scenario {
                observations: vec![Ok(false)],
                ..Default::default()
            };
            let mut containment = FixtureLease {
                marker: containment_marker,
                events: &events,
                scenario: &scenario,
                observations: scenario.observations.clone().into_iter(),
            };
            let mut quota = FixtureQuotaLease {
                root: destination.clone(),
                marker: quota_marker,
                events: &events,
                scenario: &scenario,
            };
            let session = ();
            let cancelled = AtomicBool::new(false);
            let commit_gate = CommitGate::default();
            let deadline = Instant::now() + Duration::from_secs(30);
            let admission = Admission {
                destination: &destination,
                cancelled: &cancelled,
                commit_gate: &commit_gate,
                consent_valid: true,
                deadline,
                now: &Instant::now,
            };
            let staging = ReceiveStaging::create(&destination).unwrap();
            std::fs::write(staging.path().join("received.bin"), b"payload").unwrap();
            let staging_path = staging.path().to_path_buf();
            let mut seal = PublicationSeal {
                containment: &mut containment,
                session: &session,
                quota: &mut quota,
                staging_path: &staging_path,
                admission: &admission,
            };

            match phase.as_str() {
                // finalize_with has completed its full scan and permission
                // pass. The process exits before authorization admission and
                // before the publication syscall.
                "before" => {
                    let _ = staging.finalize_with(|| std::process::exit(86));
                    panic!("fixture unexpectedly reached publication");
                }
                // A successful local rename and COMMITTED transition have
                // completed. Exit without unwinding to model supervisor loss.
                "after" => {
                    staging.finalize_with(|| seal.authorize()).unwrap();
                    commit_gate.published();
                    std::process::exit(87);
                }
                _ => panic!("unknown publication crash fixture phase"),
            }
        }

        #[test]
        fn publication_crash_before_or_after_commit_preserves_observed_state() {
            for (phase, exit_code, published) in [("before", 86, false), ("after", 87, true)] {
                let root = fixture_root();
                let exit = Command::new(std::env::current_exe().unwrap())
                    .args([
                        "--exact",
                        "airdrop::receive::tests::publication_boundary_crash_fixture",
                    ])
                    .env("OC_P019G_PUBLICATION_CRASH_PHASE", phase)
                    .env("OC_P019G_PUBLICATION_CRASH_ROOT", root.path())
                    .stdout(Stdio::null())
                    .status()
                    .unwrap();
                assert_eq!(exit.code(), Some(exit_code));
                assert!(root.path().join("owner").exists());
                assert!(root.path().join("dest/quota-owner").exists());
                let bundles: Vec<_> = std::fs::read_dir(root.path().join("dest"))
                    .unwrap()
                    .map(|entry| entry.unwrap().path())
                    .filter(|path| {
                        path.file_name()
                            .unwrap()
                            .to_string_lossy()
                            .starts_with("AirDrop Receive ")
                    })
                    .collect();
                assert_eq!(bundles.len(), usize::from(published));
                if published {
                    assert_eq!(
                        std::fs::read(bundles[0].join("received.bin")).unwrap(),
                        b"payload"
                    );
                } else {
                    assert_eq!(private_resource_counts(root.path()), (1, 0));
                }
            }
        }

        #[test]
        fn receive_options_require_exact_backend_consent_destination_and_bounds() {
            let args = [
                "--backend".into(),
                "owly".into(),
                "--destination".into(),
                "/tmp/receive".into(),
                "--duration-seconds".into(),
                "30".into(),
                "--radio-prepared".into(),
                "--consent".into(),
            ];
            assert!(Options::parse(&args).is_ok());
            assert!(Options::parse(&args[..args.len() - 1]).is_err());
            let mut unsupported = args.clone();
            unsupported[1] = "other".into();
            assert!(Options::parse(&unsupported).is_err());
            let mut unbounded = args.clone();
            unbounded[5] = "65535".into();
            assert!(Options::parse(&unbounded).is_err());
            let mut unknown = args.to_vec();
            unknown.push("--activate-radio".into());
            assert!(Options::parse(&unknown).is_err());
        }

        #[test]
        fn unsupported_versions_and_unprepared_radios_stay_blocked() {
            for version in ["0.4.1", "0.4.3", "0.4.2-dev", "", "1.0.0"] {
                assert!(!supported_owly_version(version));
            }
            assert!(supported_owly_version("0.4.2"));
            // These remain independent mandatory preflight conditions.
            for (interface, link_local) in [
                (BackendState::Unknown, true),
                (BackendState::Ready, false),
                (BackendState::Blocked, false),
            ] {
                assert!(!(interface == BackendState::Ready && link_local));
            }
        }

        #[test]
        fn leader_exit_with_known_writer_never_publishes() {
            let destination = tempfile::tempdir().unwrap();
            let stage = ReceiveStaging::create(destination.path()).unwrap();
            let path = std::fs::canonicalize(stage.path()).unwrap();
            let mut writer = Command::new("/bin/sh")
                .args([
                    "-c",
                    r#"trap '' INT; printf writing > "$1/file"; while :; do sleep 1; done"#,
                    "fixture",
                    path.to_str().unwrap(),
                ])
                .stdin(Stdio::null())
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .spawn()
                .unwrap();
            let mut leader = fixture_child("nonzero");
            let (_, state, _) = supervise(
                &mut leader,
                &stage,
                Instant::now() + Duration::from_secs(2),
                &AtomicBool::new(false),
                &AtomicBool::new(false),
                &AtomicBool::new(false),
            );
            assert_eq!(state, ReceiveState::Failed);
            assert!(writer.try_wait().unwrap().is_none());
            assert_eq!(finish_staging(stage, false, false), (0, 0, "incomplete"));
            assert!(path.exists());
            // Owned Child kill/reap, never a numeric PGID from a reaped leader.
            writer.kill().unwrap();
            writer.wait().unwrap();
            assert!(!destination.path().join("file").exists());
        }

        #[test]
        fn no_backend_can_start_without_writer_containment() {
            assert!(!writer_containment_available());
            for quiescent in [false, true] {
                let flag = AtomicBool::new(false);
                assert_eq!(
                    publication_eligible(
                        EndReason::BackendExited,
                        ReceiveState::Completed,
                        BackendState::Available,
                        &flag,
                        &flag,
                        &flag,
                        quiescent
                    ),
                    quiescent
                );
            }
        }

        #[test]
        fn failed_sessions_never_publish() {
            for (reason, state, backend, cancel, exceeded, failed) in [
                (
                    EndReason::Cancelled,
                    ReceiveState::Cancelled,
                    BackendState::Available,
                    true,
                    false,
                    false,
                ),
                (
                    EndReason::Expired,
                    ReceiveState::Expired,
                    BackendState::Available,
                    false,
                    false,
                    false,
                ),
                (
                    EndReason::Failed,
                    ReceiveState::Failed,
                    BackendState::Failed,
                    false,
                    false,
                    false,
                ),
                (
                    EndReason::BackendExited,
                    ReceiveState::Completed,
                    BackendState::Available,
                    false,
                    false,
                    true,
                ),
                (
                    EndReason::OutputLimitExceeded,
                    ReceiveState::Failed,
                    BackendState::Failed,
                    false,
                    true,
                    false,
                ),
            ] {
                let destination = tempfile::tempdir().unwrap();
                std::fs::write(destination.path().join("existing"), b"keep").unwrap();
                let stage = ReceiveStaging::create(destination.path()).unwrap();
                std::fs::write(stage.path().join("partial"), b"unsafe").unwrap();
                let eligible = publication_eligible(
                    reason,
                    state,
                    backend,
                    &AtomicBool::new(cancel),
                    &AtomicBool::new(exceeded),
                    &AtomicBool::new(failed),
                    true,
                );
                assert!(!eligible);
                assert_eq!(finish_staging(stage, eligible, true), (0, 0, "complete"));
                assert_eq!(std::fs::read_dir(destination.path()).unwrap().count(), 1);
            }
        }

        #[test]
        fn unknown_writers_preserve_containment_and_never_publish() {
            let destination = tempfile::tempdir().unwrap();
            let stage = ReceiveStaging::create(destination.path()).unwrap();
            let path = std::fs::canonicalize(stage.path()).unwrap();
            std::fs::write(path.join("partial"), b"unsafe").unwrap();
            assert_eq!(finish_staging(stage, false, false), (0, 0, "incomplete"));
            assert!(path.join("partial").exists());
        }

        fn fixture_child(mode: &str) -> std::process::Child {
            let mut command = Command::new(std::env::current_exe().unwrap());
            command
                .args([
                    "--exact",
                    "airdrop::process::tests::process_fixture_child",
                    "--nocapture",
                ])
                .env("OC_P019_PROCESS_FIXTURE", mode)
                .stdin(Stdio::null())
                .stdout(Stdio::null())
                .stderr(Stdio::null());
            install_child_limits(&mut command);
            command.spawn().unwrap()
        }

        fn fixture_child_with_output(mode: &str) -> std::process::Child {
            let mut command = Command::new(std::env::current_exe().unwrap());
            command
                .args([
                    "--exact",
                    "airdrop::process::tests::process_fixture_child",
                    "--nocapture",
                ])
                .env("OC_P019_PROCESS_FIXTURE", mode)
                .stdin(Stdio::null())
                .stdout(Stdio::piped())
                .stderr(Stdio::piped());
            install_child_limits(&mut command);
            command.spawn().unwrap()
        }

        #[test]
        fn receive_deadline_and_cancellation_terminate_owned_children() {
            let destination = tempfile::tempdir().unwrap();
            for (mode, cancellation, expected) in [
                ("hang", false, ReceiveState::Expired),
                ("hang", true, ReceiveState::Cancelled),
            ] {
                let stage = ReceiveStaging::create(destination.path()).unwrap();
                let mut child = fixture_child(mode);
                let cancelled = AtomicBool::new(cancellation);
                let output_exceeded = AtomicBool::new(false);
                let output_failed = AtomicBool::new(false);
                let deadline = if cancellation {
                    Instant::now() + Duration::from_secs(5)
                } else {
                    Instant::now() + Duration::from_millis(40)
                };
                let (_, state, _) = supervise(
                    &mut child,
                    &stage,
                    deadline,
                    &cancelled,
                    &output_exceeded,
                    &output_failed,
                );
                assert_eq!(state, expected);
                assert!(child.try_wait().unwrap().is_some());
                drop(stage);
            }
        }

        #[test]
        fn unsafe_staging_is_rejected_and_backend_is_stopped() {
            let destination = tempfile::tempdir().unwrap();
            let stage = ReceiveStaging::create(destination.path()).unwrap();
            #[cfg(unix)]
            std::os::unix::fs::symlink("/", stage.path().join("escape")).unwrap();
            let mut child = fixture_child("hang");
            let cancelled = AtomicBool::new(false);
            let output_exceeded = AtomicBool::new(false);
            let output_failed = AtomicBool::new(false);
            let (reason, state, _) = supervise(
                &mut child,
                &stage,
                Instant::now() + Duration::from_secs(5),
                &cancelled,
                &output_exceeded,
                &output_failed,
            );
            assert!(matches!(reason, EndReason::OutputLimitExceeded));
            assert_eq!(state, ReceiveState::Failed);
            assert!(child.try_wait().unwrap().is_some());
            drop(stage);
        }

        #[test]
        fn terminal_output_limit_stops_backend_without_pipe_backpressure() {
            use std::sync::atomic::AtomicUsize;

            let destination = tempfile::tempdir().unwrap();
            let stage = ReceiveStaging::create(destination.path()).unwrap();
            let mut child = fixture_child_with_output("flood");
            let bytes = Arc::new(AtomicUsize::new(0));
            let exceeded = Arc::new(AtomicBool::new(false));
            let failed = Arc::new(AtomicBool::new(false));
            let stdout = child.stdout.take().unwrap();
            let stderr = child.stderr.take().unwrap();
            let stdout_relay = std::thread::spawn({
                let bytes = bytes.clone();
                let exceeded = exceeded.clone();
                let failed = failed.clone();
                move || {
                    crate::airdrop::process::relay_bounded(
                        stdout,
                        std::io::sink(),
                        &bytes,
                        &exceeded,
                        &failed,
                        32,
                    )
                }
            });
            let stderr_relay = std::thread::spawn({
                let bytes = bytes.clone();
                let exceeded = exceeded.clone();
                let failed = failed.clone();
                move || {
                    crate::airdrop::process::relay_bounded(
                        stderr,
                        std::io::sink(),
                        &bytes,
                        &exceeded,
                        &failed,
                        32,
                    )
                }
            });
            let cancelled = AtomicBool::new(false);
            let (reason, state, _) = supervise(
                &mut child,
                &stage,
                Instant::now() + Duration::from_secs(2),
                &cancelled,
                &exceeded,
                &failed,
            );
            assert!(stdout_relay.join().unwrap().is_ok());
            assert!(stderr_relay.join().unwrap().is_ok());
            assert!(matches!(reason, EndReason::OutputLimitExceeded));
            assert_eq!(state, ReceiveState::Failed);
            assert_eq!(bytes.load(Ordering::Relaxed), 32);
            assert!(child.try_wait().unwrap().is_some());
            drop(stage);
        }

        #[test]
        fn nonblocking_terminal_reopen_does_not_mutate_source_descriptor_flags() {
            use std::ffi::CStr;

            let master =
                unsafe { libc::posix_openpt(libc::O_RDWR | libc::O_NOCTTY | libc::O_CLOEXEC) };
            assert!(master >= 0, "could not open a test pseudo-terminal");
            assert_eq!(unsafe { libc::grantpt(master) }, 0);
            assert_eq!(unsafe { libc::unlockpt(master) }, 0);
            let mut name = [0_i8; 256];
            assert_eq!(
                unsafe { libc::ptsname_r(master, name.as_mut_ptr(), name.len()) },
                0
            );
            // SAFETY: `ptsname_r` wrote a NUL-terminated path to `name`.
            let slave_name = unsafe { CStr::from_ptr(name.as_ptr()) };
            let slave = unsafe {
                libc::open(
                    slave_name.as_ptr(),
                    libc::O_RDWR | libc::O_NOCTTY | libc::O_CLOEXEC,
                )
            };
            assert!(slave >= 0, "could not open the test pseudo-terminal slave");
            let original_flags = unsafe { libc::fcntl(slave, libc::F_GETFL) };
            assert!(original_flags >= 0);
            let writer = open_nonblocking_terminal(slave).unwrap();
            let reopened_flags = unsafe { libc::fcntl(writer.as_raw_fd(), libc::F_GETFL) };
            let original_after = unsafe { libc::fcntl(slave, libc::F_GETFL) };
            assert!(reopened_flags & libc::O_NONBLOCK != 0);
            assert_eq!(original_after, original_flags);
            drop(writer);
            unsafe {
                libc::close(slave);
                libc::close(master);
            }
        }

        #[test]
        fn nonblocking_terminal_reopen_rejects_non_terminal_descriptors() {
            let file = std::fs::File::open("/dev/null").unwrap();
            assert_eq!(
                open_nonblocking_terminal(file.as_raw_fd())
                    .unwrap_err()
                    .kind(),
                io::ErrorKind::NotConnected
            );
        }
    }
}
