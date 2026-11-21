//! Read-only AirDrop/AWDL backend diagnostics.

#[cfg(target_os = "linux")]
use opencontinuity_core::airdrop::BlockReason;
use opencontinuity_core::airdrop::{AirDropReport, BackendState, PlaneState};
#[cfg(target_os = "linux")]
use opencontinuity_core::airdrop::{ObservationSource, RadioReport};
#[cfg(target_os = "linux")]
use serde_json::Value;
#[cfg(target_os = "linux")]
use std::collections::HashSet;
#[cfg(target_os = "linux")]
use std::path::{Path, PathBuf};
use std::process::ExitCode;
#[cfg(target_os = "linux")]
use std::time::Duration;

#[cfg(target_os = "linux")]
#[path = "airdrop_process.rs"]
mod process;

#[cfg(target_os = "linux")]
const OWLY_VERSION: &str = "0.4.2";
#[cfg(target_os = "linux")]
const OWLY_DOCTOR_OUTPUT_LIMIT: usize = 64 * 1024;

const USAGE: &str = "Usage:\n  opencontinuity airdrop doctor [--json]\n  opencontinuity airdrop backend status [--json]\n  Doctor/status are read-only. This build does not implement file receiving. OpenContinuity never activates AWDL or changes radio configuration.";

pub fn run(args: &[String]) -> ExitCode {
    match args.first().map(String::as_str) {
        Some("doctor") if valid_json_suffix(&args[1..]) => print_report(),
        Some("backend")
            if args.get(1).map(String::as_str) == Some("status")
                && valid_json_suffix(&args[2..]) =>
        {
            print_report()
        }
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

    let Some(executable) = backend_path_or_missing(&mut report, find_fixed_executable("owly"))
    else {
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
    apply_radio_status(&mut report);
    // Owly's CLI has no machine-readable advertisement-ready or per-transfer
    // progress API. Do not infer those states from process availability.
    report.advertisement = PlaneState::Unknown;
    report.live_airdrop_interoperability = BackendState::Unknown;
    report
}

#[cfg(target_os = "linux")]
fn supported_owly_version(version: &str) -> bool {
    version == OWLY_VERSION
}

#[cfg(target_os = "linux")]
fn backend_path_or_missing(
    report: &mut AirDropReport,
    executable: Option<PathBuf>,
) -> Option<PathBuf> {
    if executable.is_none() {
        report.backend = BackendState::Blocked;
        report.control_plane = PlaneState::Blocked;
        report.block_reason = Some(BlockReason::BackendNotInstalled);
    }
    executable
}

#[cfg(target_os = "linux")]
fn apply_radio_status(report: &mut AirDropReport) {
    if report.radio.awdl_interface_state == BackendState::Ready
        && report.radio.awdl_link_local_address_present
    {
        report.backend = BackendState::Ready;
        report.data_plane = PlaneState::Available;
        report.block_reason = None;
    } else {
        report.backend = BackendState::Blocked;
        report.data_plane = PlaneState::Blocked;
        report.block_reason = Some(BlockReason::RadioNotPrepared);
    }
    // This build has no operational Receive path, even when diagnostics
    // observe an already prepared interface.
    report.receiver_available = false;
}

#[cfg(target_os = "linux")]
fn has_duplicate_keys(bytes: &[u8]) -> bool {
    let mut objects: Vec<HashSet<String>> = Vec::new();
    let mut index = 0;
    while index < bytes.len() {
        match bytes[index] {
            b'{' => {
                objects.push(HashSet::new());
                index += 1;
            }
            b'}' => {
                objects.pop();
                index += 1;
            }
            b'"' => {
                let start = index;
                index += 1;
                while index < bytes.len() {
                    match bytes[index] {
                        b'\\' => index = (index + 2).min(bytes.len()),
                        b'"' => {
                            index += 1;
                            break;
                        }
                        _ => index += 1,
                    }
                }
                let mut after = index;
                while bytes.get(after).is_some_and(u8::is_ascii_whitespace) {
                    after += 1;
                }
                if bytes.get(after) == Some(&b':') {
                    if let Some(keys) = objects.last_mut() {
                        if let Ok(key) = serde_json::from_slice::<String>(&bytes[start..index]) {
                            if !keys.insert(key) {
                                return true;
                            }
                        }
                    }
                }
            }
            _ => index += 1,
        }
    }
    false
}

/// Owly 0.4.2 Capabilities/Card/ChannelUse at upstream 6f2e8ea.
#[cfg(target_os = "linux")]
fn doctor_ready(output: &process::CommandOutput) -> Result<bool, ()> {
    if output.timed_out
        || output.stdout_truncated
        || output.stderr_truncated
        || !output.status.success()
    {
        return Err(());
    }
    if has_duplicate_keys(&output.stdout) {
        return Err(());
    }
    let json: Value = serde_json::from_slice(&output.stdout).map_err(|_| ())?;
    validate_doctor(&json)
}

#[cfg(target_os = "linux")]
fn validate_doctor(json: &Value) -> Result<bool, ()> {
    let object = json.as_object().ok_or(())?;
    if object.len() != 10
        || object.keys().any(|key| {
            ![
                "station", "driver", "device", "status", "mode", "channel", "hardware", "issues",
                "missing", "card",
            ]
            .contains(&key.as_str())
        })
    {
        return Err(());
    }
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
        let Some(entry) = entry.as_object() else {
            return Err(());
        };
        if entry.len() != 2
            || !entry
                .keys()
                .all(|key| ["id", "say"].contains(&key.as_str()))
            || !matches!(
                entry.get("id").and_then(Value::as_str),
                Some("hardware" | "backend" | "radio")
            )
            || !entry.get("say").is_some_and(Value::is_string)
        {
            return Err(());
        }
    }
    let card = object.get("card").ok_or(())?;
    if !card.is_null() {
        let Some(card_object) = card.as_object() else {
            return Err(());
        };
        if card_object.len() != 9
            || card_object.keys().any(|key| {
                ![
                    "phy",
                    "ibss",
                    "ap",
                    "active_monitor",
                    "monitor_beside_station",
                    "ibss_beside_station",
                    "self_managed_reg",
                    "regdom",
                    "channels",
                ]
                .contains(&key.as_str())
            })
        {
            return Err(());
        }
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
            let Some(channel_object) = channel.as_object() else {
                return Err(());
            };
            if ![3, 4].contains(&channel_object.len())
                || channel_object
                    .keys()
                    .any(|key| !["channel", "freq", "transmit", "why"].contains(&key.as_str()))
            {
                return Err(());
            }
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
    Ok(missing.is_empty()
        && hardware
        && object.get("status").and_then(Value::as_str) == Some("tested")
        && card.is_object())
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

#[cfg(all(test, target_os = "linux"))]
mod diagnostic_tests {
    use super::*;

    #[test]
    fn missing_backend_defaults_to_unknown_and_receive_unavailable() {
        let mut report = AirDropReport::unknown();
        let executable = backend_path_or_missing(&mut report, None);
        assert!(executable.is_none());
        assert_eq!(report.backend, BackendState::Blocked);
        assert_eq!(report.control_plane, PlaneState::Blocked);
        assert_eq!(report.block_reason, Some(BlockReason::BackendNotInstalled));
        assert!(!report.receiver_available);
        assert_eq!(report.authorization, "never_evaluated");
        assert_eq!(report.physical_qualification, "not_qualified");
    }

    #[test]
    fn unsupported_radio_is_reported_as_blocked() {
        let mut report = AirDropReport::unknown();
        report.backend = BackendState::Ready;
        report.radio.state = BackendState::Unsupported;
        report.radio.awdl_interface_state = BackendState::Unsupported;
        apply_radio_status(&mut report);
        assert_eq!(report.backend, BackendState::Blocked);
        assert_eq!(report.data_plane, PlaneState::Blocked);
        assert_eq!(report.block_reason, Some(BlockReason::RadioNotPrepared));
        assert!(!report.receiver_available);
    }

    #[test]
    fn local_awdl_readiness_never_enables_receive_or_qualifies_interoperability() {
        let mut report = AirDropReport::unknown();
        report.radio.awdl_interface_state = BackendState::Ready;
        report.radio.awdl_link_local_address_present = true;
        apply_radio_status(&mut report);
        assert_eq!(report.backend, BackendState::Ready);
        assert!(!report.receiver_available);
        assert_eq!(report.live_airdrop_interoperability, BackendState::Unknown);
        assert_eq!(report.authorization, "never_evaluated");
        assert_eq!(report.physical_qualification, "not_qualified");
    }

    #[test]
    fn strict_doctor_key_check_detects_escaped_duplicate_keys() {
        assert!(has_duplicate_keys(
            br#"{"schemaVersion":1,"schema\u0056ersion":1}"#
        ));
        assert!(!has_duplicate_keys(
            br#"{"message":"\\\"schemaVersion\\\":1","nested":{"x":1}}"#
        ));
    }
}
