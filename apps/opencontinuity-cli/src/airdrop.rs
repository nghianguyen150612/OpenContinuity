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

#[path = "airdrop_process.rs"]
mod process;
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
    if actual_version != OWLY_VERSION {
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
    if doctor.timed_out || doctor.stdout_truncated || doctor.stderr_truncated {
        report.block_reason = Some(BlockReason::BackendProbeFailed);
        return report;
    }
    let Ok(json) = serde_json::from_slice::<Value>(&doctor.stdout) else {
        report.block_reason = Some(BlockReason::BackendProbeFailed);
        return report;
    };
    let Some(missing) = json.get("missing").and_then(Value::as_array) else {
        report.block_reason = Some(BlockReason::BackendProbeFailed);
        return report;
    };
    report.source = ObservationSource::OwlyDoctor;
    if !missing.is_empty() {
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
    use crate::airdrop::process::MAX_RECEIVE_TERMINAL_OUTPUT;
    use crate::airdrop::storage::{ReceiveLock, ReceiveSessionState, ReceiveStaging};
    use opencontinuity_core::airdrop::{BlockReason, ReceiveLifecycle, ReceiveState};
    use serde::Serialize;
    use std::fs::{File, OpenOptions};
    use std::io;
    use std::os::fd::{AsRawFd, RawFd};
    use std::os::unix::fs::OpenOptionsExt;
    use std::os::unix::process::CommandExt;
    use std::process::{Command, Stdio};

    #[derive(Debug, Clone, Copy, Serialize)]
    #[serde(rename_all = "snake_case")]
    enum EndReason {
        Cancelled,
        Expired,
        BackendExited,
        OutputLimitExceeded,
        Failed,
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
        let staging = match ReceiveStaging::create(&options.destination) {
            Ok(staging) => staging,
            Err(_) => {
                return print_blocked(
                    BlockReason::DestinationUnavailable,
                    EndReason::Failed,
                    ReceiveState::Failed,
                );
            }
        };
        let session_state = match ReceiveSessionState::create_runtime() {
            Ok(state) => state,
            Err(_) => {
                return print_blocked(
                    BlockReason::PermissionUnavailable,
                    EndReason::Failed,
                    ReceiveState::Failed,
                );
            }
        };
        let Some(executable) = find_fixed_executable("owly") else {
            return print_blocked(
                BlockReason::BackendNotInstalled,
                EndReason::Failed,
                ReceiveState::Failed,
            );
        };

        let stage_path = staging.path().as_os_str().to_os_string();
        let mut command = Command::new(executable);
        command
            .arg("receive")
            .arg("--iface")
            .arg("awdl0")
            .arg("--name")
            .arg("OpenContinuity")
            .arg("--dir")
            .arg(stage_path)
            .env("XDG_STATE_HOME", session_state.path())
            .env_remove("RUST_LOG")
            .stdin(Stdio::inherit())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        install_child_limits(&mut command);
        let mut child = match command.spawn() {
            Ok(child) => child,
            Err(_) => {
                return print_blocked(
                    BlockReason::BackendFailed,
                    EndReason::Failed,
                    ReceiveState::Failed,
                );
            }
        };

        let Some(child_stdout) = child.stdout.take() else {
            terminate_child(&mut child, Duration::from_secs(2));
            return print_blocked(
                BlockReason::BackendFailed,
                EndReason::Failed,
                ReceiveState::Failed,
            );
        };
        let Some(child_stderr) = child.stderr.take() else {
            terminate_child(&mut child, Duration::from_secs(2));
            return print_blocked(
                BlockReason::BackendFailed,
                EndReason::Failed,
                ReceiveState::Failed,
            );
        };
        let output_bytes = Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let output_exceeded = Arc::new(AtomicBool::new(false));
        let output_failed = Arc::new(AtomicBool::new(false));
        let stdout_relay = spawn_terminal_relay(
            child_stdout,
            output_bytes.clone(),
            output_exceeded.clone(),
            output_failed.clone(),
        );
        let stderr_relay = spawn_terminal_relay(
            child_stderr,
            output_bytes,
            output_exceeded.clone(),
            output_failed.clone(),
        );

        let cancelled = Arc::new(AtomicBool::new(false));
        let mut signal_ids = Vec::new();
        for signal in [signal_hook::consts::SIGINT, signal_hook::consts::SIGTERM] {
            match signal_hook::flag::register(signal, cancelled.clone()) {
                Ok(id) => signal_ids.push(id),
                Err(_) => {
                    terminate_child(&mut child, Duration::from_secs(2));
                    let _ = stdout_relay.join();
                    let _ = stderr_relay.join();
                    return print_blocked(
                        BlockReason::BackendFailed,
                        EndReason::Failed,
                        ReceiveState::Failed,
                    );
                }
            }
        }

        let deadline = Instant::now() + Duration::from_secs(options.duration_seconds.into());
        let (mut end_reason, mut state, mut child_status) = supervise(
            &mut child,
            &staging,
            deadline,
            &cancelled,
            &output_exceeded,
            &output_failed,
        );
        let stdout_ok = stdout_relay.join().is_ok_and(|result| result.is_ok());
        let stderr_ok = stderr_relay.join().is_ok_and(|result| result.is_ok());
        if !stdout_ok || !stderr_ok {
            output_failed.store(true, Ordering::Relaxed);
        }
        if output_exceeded.load(Ordering::Relaxed) {
            end_reason = EndReason::OutputLimitExceeded;
            state = ReceiveState::Failed;
            child_status = BackendState::Failed;
        } else if output_failed.load(Ordering::Relaxed) {
            end_reason = EndReason::Failed;
            state = ReceiveState::Failed;
            child_status = BackendState::Failed;
        }
        for id in signal_ids {
            signal_hook::low_level::unregister(id);
        }
        let (files, bytes, cleanup) = if matches!(end_reason, EndReason::OutputLimitExceeded) {
            let cleanup = if staging.discard().is_ok() {
                "complete"
            } else {
                "incomplete"
            };
            (0, 0, cleanup)
        } else {
            match staging.finalize() {
                Ok(result) => (result.files, result.bytes, "complete"),
                Err(_) => (0, 0, "incomplete"),
            }
        };
        let block_reason = match end_reason {
            EndReason::Cancelled => Some(BlockReason::Cancelled),
            EndReason::Expired => Some(BlockReason::SessionExpired),
            EndReason::OutputLimitExceeded => Some(BlockReason::OutputLimitExceeded),
            EndReason::Failed => Some(BlockReason::BackendFailed),
            EndReason::BackendExited if cleanup != "complete" => Some(BlockReason::BackendFailed),
            EndReason::BackendExited => None,
        };
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

    fn supervise(
        child: &mut std::process::Child,
        staging: &ReceiveStaging,
        deadline: Instant,
        cancelled: &AtomicBool,
        output_exceeded: &AtomicBool,
        output_failed: &AtomicBool,
    ) -> (EndReason, ReceiveState, BackendState) {
        loop {
            if output_exceeded.load(Ordering::Relaxed) {
                let _ = terminate_child(child, Duration::from_secs(2));
                return (
                    EndReason::OutputLimitExceeded,
                    ReceiveState::Failed,
                    BackendState::Failed,
                );
            }
            if output_failed.load(Ordering::Relaxed) {
                let _ = terminate_child(child, Duration::from_secs(2));
                return (
                    EndReason::Failed,
                    ReceiveState::Failed,
                    BackendState::Failed,
                );
            }
            if cancelled.load(Ordering::Relaxed) {
                let status = terminate_child(child, Duration::from_secs(2));
                return (
                    EndReason::Cancelled,
                    ReceiveState::Cancelled,
                    if status {
                        BackendState::Available
                    } else {
                        BackendState::Failed
                    },
                );
            }
            if Instant::now() >= deadline {
                let status = terminate_child(child, Duration::from_secs(2));
                return (
                    EndReason::Expired,
                    ReceiveState::Expired,
                    if status {
                        BackendState::Available
                    } else {
                        BackendState::Failed
                    },
                );
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
                    let _ = terminate_child(child, Duration::from_secs(2));
                    return (
                        EndReason::Failed,
                        ReceiveState::Failed,
                        BackendState::Failed,
                    );
                }
            }
            if staging.usage().is_err() {
                let _ = terminate_child(child, Duration::from_secs(2));
                return (
                    EndReason::OutputLimitExceeded,
                    ReceiveState::Failed,
                    BackendState::Failed,
                );
            }
            std::thread::sleep(Duration::from_millis(100));
        }
    }

    fn terminate_child(child: &mut std::process::Child, grace: Duration) -> bool {
        // Owly handles SIGINT by closing its server and mDNS registration. It
        // runs in a new process group so a future helper cannot outlive us.
        let pid = child.id() as libc::pid_t;
        // SAFETY: the negative pid targets only the process group created for
        // this child by `install_child_limits`.
        let _ = unsafe { libc::kill(-pid, libc::SIGINT) };
        let deadline = Instant::now() + grace;
        loop {
            if let Ok(Some(status)) = child.try_wait() {
                return status.success();
            }
            if Instant::now() >= deadline {
                let _ = unsafe { libc::kill(-pid, libc::SIGKILL) };
                let _ = child.kill();
                return child.wait().is_ok_and(|status| status.success());
            }
            std::thread::sleep(Duration::from_millis(20));
        }
    }

    fn spawn_terminal_relay<R: io::Read + Send + 'static>(
        reader: R,
        bytes: Arc<std::sync::atomic::AtomicUsize>,
        exceeded: Arc<AtomicBool>,
        failed: Arc<AtomicBool>,
    ) -> std::thread::JoinHandle<io::Result<()>> {
        std::thread::spawn(move || {
            let mut writer = match open_nonblocking_terminal(libc::STDERR_FILENO) {
                Ok(writer) => writer,
                Err(error) => {
                    failed.store(true, Ordering::Relaxed);
                    return Err(error);
                }
            };
            crate::airdrop::process::relay_bounded(
                reader,
                &mut writer,
                &bytes,
                &exceeded,
                &failed,
                MAX_RECEIVE_TERMINAL_OUTPUT,
            )
        })
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
