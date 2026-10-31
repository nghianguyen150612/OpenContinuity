use opencontinuity_core::{
    DoctorReport,
    research::{MockDiscoveryAdapter, ResearchFixture, read_only_report},
    watch_auth_research::{
        EnrollmentState, HostAuthRequest, HostEnrollment, ReplayGuard, WatchAuthFixture,
        WatchAuthResponse, WatchEnrollment, verify_fixture, verify_host_request,
        verify_watch_response,
    },
    watch_auth_transport::{
        Direction as AuthTransportDirection, SignedEnvelope, TransferAssembler,
        encode_frames as encode_auth_frames,
    },
    watch_endpoint::{WatchEndpointFixture, simulate},
};
use p256::ecdsa::SigningKey;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    env, fs,
    io::Read,
    process::ExitCode,
    time::{Instant, SystemTime, UNIX_EPOCH},
};

mod bluez;
mod device_inventory;
mod gatt;
mod qualification_diagnostics;

const HELP: &str = concat!(
    "OpenContinuity — native Apple ecosystem compatibility for Linux and beyond\n\n",
    "Usage: opencontinuity [--help] [--version] [doctor [--json]]\n",
    "       opencontinuity research watch --fixture <path> [--json]\n",
    "       opencontinuity research watch-endpoint --fixture <path> [--json]\n",
    "       opencontinuity research watch-auth --fixture <path> [--json]\n",
    "       opencontinuity research watch-auth-transport prepare --fixture <path> --output <path>\n",
    "       opencontinuity research watch-auth-transport verify-response --fixture <path> --input <path> [--json]\n",
    "       opencontinuity research watch-auth-gatt serve --adapter hci0 --duration-seconds 60 --enrollments <path> --host-key-file <path> --confirm-host-sha256 <64-lowercase-hex> --confirm-watch-sha256 <64-lowercase-hex> --experimental [--json]\n",
    "       opencontinuity research watch-auth-gatt generate-host-key --host-key-file <new-path> --experimental [--json]\n",
    "       opencontinuity research watch-auth-gatt export-host-pin --host-enrollment-id <id> --host-identity <id> --linux-uid <uid> --host-key-file <path> --experimental [--json]\n",
    "       opencontinuity research watch-auth-gatt check-enrollment --enrollments <path> --host-key-file <path> --experimental [--json]\n",
    "       opencontinuity research watch-ble --capabilities [--json]\n",
    "       opencontinuity devices list --json\n",
    "       opencontinuity research watch-gatt serve --adapter hci0 --duration-seconds 60 --experimental [--json]\n\n",
    "Commands:\n",
    "  doctor         Report this build's capability states without scanning devices\n",
    "  devices list   Read one snapshot of BlueZ known devices (never scans)\n",
    "  research       Read fixtures, inspect BlueZ, or run an explicit experimental GATT server\n",
    "  --help         Print this help\n",
    "  --version      Print version\n\n",
    "Watch fixtures are offline mocks. watch-auth verifies test-only signed transcripts and never authorizes Linux. ",
    "watch-auth-transport is a test-only serialized transport integration harness; it never authorizes Linux. ",
    "watch-ble --capabilities is read-only. watch-gatt serve registers a temporary synthetic service and advertisement; ",
    "it requires --experimental and never authorizes Linux.\n",
);

fn main() -> ExitCode {
    let mut args = env::args().skip(1);
    match args.next().as_deref() {
        Some("--help" | "-h") | None => {
            if args.next().is_some() {
                return usage_error();
            }
            print!("{HELP}");
            ExitCode::SUCCESS
        }
        Some("--version" | "-V") => {
            if args.next().is_some() {
                return usage_error();
            }
            println!("opencontinuity {}", env!("CARGO_PKG_VERSION"));
            ExitCode::SUCCESS
        }
        Some("doctor") => match args.next().as_deref() {
            None => {
                println!("OpenContinuity doctor ({}):", env!("CARGO_PKG_VERSION"));
                for capability in DoctorReport::current(env!("CARGO_PKG_VERSION")).capabilities {
                    println!(
                        "  {:<28} {:<13} {}",
                        capability.id,
                        format!("{:?}", capability.state).to_lowercase(),
                        capability.summary
                    );
                }
                println!(
                    "  Watch trust stages: unknown (no device scan or trust evaluation performed)"
                );
                ExitCode::SUCCESS
            }
            Some("--json") => {
                if args.next().is_some() {
                    return usage_error();
                }
                match serde_json::to_string_pretty(&DoctorReport::current(env!(
                    "CARGO_PKG_VERSION"
                ))) {
                    Ok(json) => {
                        println!("{json}");
                        ExitCode::SUCCESS
                    }
                    Err(error) => {
                        eprintln!("failed to serialize doctor report: {error}");
                        ExitCode::FAILURE
                    }
                }
            }
            _ => usage_error(),
        },
        Some("research") => run_research(args.collect()),
        Some("devices") => run_devices(args.collect()),
        _ => usage_error(),
    }
}

fn run_devices(args: Vec<String>) -> ExitCode {
    if args.len() != 2 || args[0] != "list" || args[1] != "--json" {
        return usage_error();
    }
    let report = device_inventory::read_snapshot();
    let status = report.status;
    match serde_json::to_vec_pretty(&report) {
        Ok(bytes) if bytes.len() < device_inventory::MAX_OUTPUT_BYTES => {
            println!("{}", String::from_utf8_lossy(&bytes));
            if status == device_inventory::InventoryStatus::Ok {
                ExitCode::SUCCESS
            } else {
                ExitCode::FAILURE
            }
        }
        _ => {
            let failure = device_inventory::DeviceInventory::failed(
                device_inventory::InventoryStatus::MalformedData,
            );
            println!(
                "{}",
                serde_json::to_string(&failure).expect("fixed status serializes")
            );
            ExitCode::FAILURE
        }
    }
}

fn run_research(args: Vec<String>) -> ExitCode {
    match args.first().map(String::as_str) {
        Some("watch") => run_watch_research(&args),
        Some("watch-endpoint") => run_watch_endpoint_research(&args),
        Some("watch-auth") => run_watch_auth_research(&args),
        Some("watch-auth-transport") => run_watch_auth_transport(&args),
        Some("watch-auth-gatt") => run_watch_auth_gatt(&args),
        Some("watch-ble") => run_watch_ble_research(&args),
        Some("watch-gatt") => run_watch_gatt_experiment(&args),
        _ => usage_error(),
    }
}

fn run_watch_gatt_experiment(args: &[String]) -> ExitCode {
    if args.get(1).map(String::as_str) != Some("serve") {
        return usage_error();
    }
    let mut adapter = None;
    let mut duration = None;
    let mut experimental = false;
    let mut json_output = false;
    let mut index = 2;
    while index < args.len() {
        match args[index].as_str() {
            "--adapter" if index + 1 < args.len() && adapter.is_none() => {
                adapter = Some(args[index + 1].clone());
                index += 2;
            }
            "--duration-seconds" if index + 1 < args.len() && duration.is_none() => {
                duration = args[index + 1].parse::<u16>().ok();
                if duration.is_none() {
                    return usage_error();
                }
                index += 2;
            }
            "--experimental" if !experimental => {
                experimental = true;
                index += 1;
            }
            "--json" if !json_output => {
                json_output = true;
                index += 1;
            }
            _ => return usage_error(),
        }
    }
    let (Some(adapter), Some(duration)) = (adapter, duration) else {
        return usage_error();
    };
    if let Err(error) = gatt::validate_options(&adapter, duration, experimental) {
        eprintln!("{error}");
        return ExitCode::from(2);
    }
    let interrupted = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
    if let Err(error) =
        signal_hook::flag::register(signal_hook::consts::SIGTERM, interrupted.clone())
    {
        eprintln!("could not install SIGTERM cleanup handler: {error}");
        return ExitCode::FAILURE;
    }
    let signal = interrupted.clone();
    if let Err(error) =
        ctrlc::set_handler(move || signal.store(true, std::sync::atomic::Ordering::Relaxed))
    {
        eprintln!("could not install SIGINT/SIGTERM cleanup handler: {error}");
        return ExitCode::FAILURE;
    }
    match gatt::serve(&adapter, duration, interrupted) {
        Ok(report) => {
            if json_output {
                println!(
                    "{}",
                    serde_json::to_string_pretty(&report).expect("report serializes")
                );
            } else {
                println!(
                    "Experimental BlueZ GATT server stopped: {}",
                    report.shutdown_reason.unwrap_or("unknown")
                );
                println!("  adapter: {}", report.adapter);
                println!("  runtime: {} seconds", report.runtime_seconds);
                println!(
                    "  controller LE peripheral capability: {}",
                    report.controller_le_peripheral_capability
                );
                println!("  authorization: never evaluated");
            }
            ExitCode::SUCCESS
        }
        Err(error) => {
            if json_output {
                println!(
                    "{}",
                    serde_json::to_string_pretty(&error.report).expect("report serializes")
                );
            } else {
                eprintln!(
                    "BlueZ GATT experiment failed at {}: {}",
                    error.report.stage,
                    error.report.error.as_deref().unwrap_or("unknown error")
                );
                eprintln!("  authorization: never evaluated");
            }
            ExitCode::FAILURE
        }
    }
}

fn run_watch_ble_research(args: &[String]) -> ExitCode {
    let mut capabilities = false;
    let mut json_output = false;
    for argument in args.iter().skip(1) {
        match argument.as_str() {
            "--capabilities" if !capabilities => capabilities = true,
            "--json" if !json_output => json_output = true,
            _ => return usage_error(),
        }
    }
    if !capabilities {
        return usage_error();
    }
    let report = bluez::inspect();
    if json_output {
        match serde_json::to_string_pretty(&report) {
            Ok(output) => println!("{output}"),
            Err(error) => {
                eprintln!("failed to serialize BlueZ capability report: {error}");
                return ExitCode::FAILURE;
            }
        }
    } else {
        println!("BlueZ read-only capability discovery:");
        println!("  discovery: {}", report.discovery);
        println!(
            "  BlueZ version: {}",
            report.bluez_version.as_deref().unwrap_or("unknown")
        );
        for adapter in &report.adapters {
            println!(
                "  adapter: {} (powered: {:?})",
                adapter.object_path, adapter.powered
            );
            println!("    GattManager1: {:?}", adapter.gatt_manager);
            println!(
                "    LEAdvertisingManager1: {:?}",
                adapter.le_advertising_manager
            );
        }
        println!("  hardware Watch observations: not run");
        println!("  authentication test: not run");
        println!("  authorization: never evaluated");
    }
    ExitCode::SUCCESS
}

fn run_watch_research(args: &[String]) -> ExitCode {
    let mut path = None;
    let mut json_output = false;
    let mut index = 1;
    while index < args.len() {
        match args[index].as_str() {
            "--fixture" if index + 1 < args.len() && path.is_none() => {
                path = Some(args[index + 1].clone());
                index += 2;
            }
            "--json" if !json_output => {
                json_output = true;
                index += 1;
            }
            _ => return usage_error(),
        }
    }
    let Some(path) = path else {
        return usage_error();
    };

    let result = (|| {
        let input = fs::read_to_string(&path)
            .map_err(|error| format!("could not read fixture {path}: {error}"))?;
        let fixture = ResearchFixture::from_json(&input).map_err(|error| error.to_string())?;
        let mut adapter = MockDiscoveryAdapter::new(fixture).map_err(|error| error.to_string())?;
        read_only_report(&mut adapter).map_err(|error| error.to_string())
    })();

    match result {
        Ok(report) if json_output => match serde_json::to_string_pretty(&report) {
            Ok(output) => {
                println!("{output}");
                ExitCode::SUCCESS
            }
            Err(error) => {
                eprintln!("failed to serialize research report: {error}");
                ExitCode::FAILURE
            }
        },
        Ok(report) => {
            println!("Watch protocol research fixture:");
            println!("  observations: {}", report.observations.len());
            println!("  authorization: never evaluated");
            println!("  source: offline mock fixture (no Bluetooth access)");
            ExitCode::SUCCESS
        }
        Err(error) => {
            eprintln!("{error}");
            ExitCode::FAILURE
        }
    }
}

fn usage_error() -> ExitCode {
    eprintln!("{HELP}");
    ExitCode::from(2)
}

fn run_watch_endpoint_research(args: &[String]) -> ExitCode {
    let mut path = None;
    let mut json_output = false;
    let mut index = 1;
    while index < args.len() {
        match args[index].as_str() {
            "--fixture" if index + 1 < args.len() && path.is_none() => {
                path = Some(args[index + 1].clone());
                index += 2;
            }
            "--json" if !json_output => {
                json_output = true;
                index += 1;
            }
            _ => return usage_error(),
        }
    }
    let Some(path) = path else {
        return usage_error();
    };

    let result = (|| {
        let file = fs::File::open(&path)
            .map_err(|error| format!("could not read fixture {path}: {error}"))?;
        let mut input = String::new();
        file.take((opencontinuity_core::watch_endpoint::MAX_MESSAGE_BYTES + 1) as u64)
            .read_to_string(&mut input)
            .map_err(|error| format!("could not read fixture {path}: {error}"))?;
        let fixture = WatchEndpointFixture::from_json(&input).map_err(|error| error.to_string())?;
        simulate(&fixture).map_err(|error| error.to_string())
    })();

    match result {
        Ok(report) if json_output => match serde_json::to_string_pretty(&report) {
            Ok(output) => {
                println!("{output}");
                ExitCode::SUCCESS
            }
            Err(error) => {
                eprintln!("failed to serialize endpoint report: {error}");
                ExitCode::FAILURE
            }
        },
        Ok(report) => {
            println!("Watch endpoint research simulation:");
            println!(
                "  transport: {:?} ({:?})",
                report.transport.kind, report.transport.state
            );
            println!("  authentication: {:?}", report.authentication.result);
            println!("  evidence: {}", report.authentication.evidence);
            println!("  authorization: never evaluated");
            ExitCode::SUCCESS
        }
        Err(error) => {
            eprintln!("{error}");
            ExitCode::FAILURE
        }
    }
}

fn run_watch_auth_research(args: &[String]) -> ExitCode {
    let mut path = None;
    let mut json_output = false;
    let mut index = 1;
    while index < args.len() {
        match args[index].as_str() {
            "--fixture" if index + 1 < args.len() && path.is_none() => {
                path = Some(args[index + 1].clone());
                index += 2;
            }
            "--json" if !json_output => {
                json_output = true;
                index += 1;
            }
            _ => return usage_error(),
        }
    }
    let Some(path) = path else {
        return usage_error();
    };
    let result = (|| {
        let mut input = String::new();
        fs::File::open(&path)
            .map_err(|error| format!("could not read fixture {path}: {error}"))?
            .take(16 * 1024 + 1)
            .read_to_string(&mut input)
            .map_err(|error| format!("could not read fixture {path}: {error}"))?;
        if input.len() > 16 * 1024 {
            return Err("watch-auth fixture exceeds 16 KiB".to_owned());
        }
        let fixture = WatchAuthFixture::from_json(&input).map_err(|error| error.to_string())?;
        verify_fixture(&fixture).map_err(|error| error.to_string())
    })();
    match result {
        Ok(report) if json_output => match serde_json::to_string_pretty(&report) {
            Ok(output) => {
                println!("{output}");
                ExitCode::SUCCESS
            }
            Err(error) => {
                eprintln!("failed to serialize Watch auth research report: {error}");
                ExitCode::FAILURE
            }
        },
        Ok(report) => {
            println!("Watch auth research fixture:");
            println!(
                "  host request signature verified: {}",
                report.host_request_signature_verified
            );
            println!(
                "  Watch response signature verified: {}",
                report.watch_response_signature_verified
            );
            println!("  decision: {:?}", report.decision);
            println!("  result: {}", report.result);
            println!("  authorization: never evaluated");
            ExitCode::SUCCESS
        }
        Err(error) => {
            eprintln!("{error}");
            ExitCode::FAILURE
        }
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct P009EnrollmentPins {
    schema_version: u8,
    expires_at_unix_ms: u64,
    host_enrollment: HostEnrollment,
    watch_enrollment: WatchEnrollment,
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct P009HostPinDocument {
    schema_version: u8,
    expires_at_unix_ms: u64,
    host_enrollment: HostEnrollment,
    host_fingerprint_sha256: String,
}

fn run_watch_auth_gatt(args: &[String]) -> ExitCode {
    match args.get(1).map(String::as_str) {
        Some("export-host-pin") => return run_export_host_pin(args),
        Some("check-enrollment") => return run_check_enrollment(args),
        Some("generate-host-key") => return run_generate_host_key(args),
        Some("serve") => {}
        _ => return usage_error(),
    }
    if args.get(1).map(String::as_str) != Some("serve") {
        return usage_error();
    }
    let mut adapter = None;
    let mut duration = None;
    let mut enrollment_path = None;
    let mut host_key_path = None;
    let mut confirmed_host_fingerprint = None;
    let mut confirmed_watch_fingerprint = None;
    let mut experimental = false;
    let mut json_output = false;
    let mut index = 2;
    while index < args.len() {
        match args[index].as_str() {
            "--adapter" if index + 1 < args.len() && adapter.is_none() => {
                adapter = Some(args[index + 1].clone());
                index += 2;
            }
            "--duration-seconds" if index + 1 < args.len() && duration.is_none() => {
                duration = args[index + 1].parse::<u16>().ok();
                if duration.is_none() {
                    return usage_error();
                }
                index += 2;
            }
            "--enrollments" if index + 1 < args.len() && enrollment_path.is_none() => {
                enrollment_path = Some(args[index + 1].clone());
                index += 2;
            }
            "--host-key-file" if index + 1 < args.len() && host_key_path.is_none() => {
                host_key_path = Some(args[index + 1].clone());
                index += 2;
            }
            "--confirm-host-sha256"
                if index + 1 < args.len() && confirmed_host_fingerprint.is_none() =>
            {
                confirmed_host_fingerprint = Some(args[index + 1].clone());
                index += 2;
            }
            "--confirm-watch-sha256"
                if index + 1 < args.len() && confirmed_watch_fingerprint.is_none() =>
            {
                confirmed_watch_fingerprint = Some(args[index + 1].clone());
                index += 2;
            }
            "--experimental" if !experimental => {
                experimental = true;
                index += 1;
            }
            "--json" if !json_output => {
                json_output = true;
                index += 1;
            }
            _ => return usage_error(),
        }
    }
    let (
        Some(adapter),
        Some(duration),
        Some(enrollment_path),
        Some(host_key_path),
        Some(confirmed_host_fingerprint),
        Some(confirmed_watch_fingerprint),
    ) = (
        adapter,
        duration,
        enrollment_path,
        host_key_path,
        confirmed_host_fingerprint,
        confirmed_watch_fingerprint,
    )
    else {
        eprintln!(
            "P009A serve requires exact host and Watch SHA-256 fingerprint confirmations from an independently verified exchange"
        );
        return usage_error();
    };
    if let Err(error) = gatt::validate_options(&adapter, duration, experimental) {
        eprintln!("{error}");
        return ExitCode::from(2);
    }
    if duration > gatt::MAX_AUTH_RUNTIME_SECONDS {
        eprintln!(
            "authenticated research runtime must not exceed the {}-second P008 request lifetime",
            gatt::MAX_AUTH_RUNTIME_SECONDS
        );
        return ExitCode::from(2);
    }
    let config = match build_auth_transport_config(&enrollment_path, &host_key_path) {
        Ok(config) => config,
        Err(error) => {
            eprintln!("P009 signed request preflight failed: {error}");
            eprintln!(
                "  live signed approval remains blocked without independently provisioned test-only keys"
            );
            return ExitCode::from(2);
        }
    };
    let host_public = config
        .host_signing_key
        .verifying_key()
        .to_encoded_point(false);
    let host_fingerprint = hex_digest(&Sha256::digest(host_public.as_bytes()));
    let watch_public = decode_hex(&config.watch_enrollment.public_key_sec1_hex)
        .expect("enrollment validation checked Watch public-key encoding");
    let watch_fingerprint = hex_digest(&Sha256::digest(watch_public));
    if let Err(error) =
        validate_fingerprint_confirmation("host", &confirmed_host_fingerprint, &host_fingerprint)
            .and_then(|()| {
                validate_fingerprint_confirmation(
                    "Watch",
                    &confirmed_watch_fingerprint,
                    &watch_fingerprint,
                )
            })
    {
        eprintln!(
            "P009A enrollment fingerprint confirmation failed: {error}; no BlueZ service was started and no request was signed"
        );
        return ExitCode::from(2);
    }
    if !json_output {
        println!(
            "P009A test_only enrollment; verify both fingerprints out of band before starting."
        );
        println!("  host SHA-256: {}", format_fingerprint(&host_fingerprint));
        println!(
            "  Watch SHA-256: {}",
            format_fingerprint(&watch_fingerprint)
        );
        println!(
            "  enrollment expires at Unix ms: {}",
            config.enrollment_expires_at_unix_ms
        );
        println!(
            "  host identity / Linux UID: {} / {}",
            config.host_enrollment.host_identity, config.host_enrollment.linux_uid
        );
        println!(
            "  Watch enrollment: {}",
            config.watch_enrollment.enrollment_id
        );
    }
    let interrupted = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
    if let Err(error) =
        signal_hook::flag::register(signal_hook::consts::SIGTERM, interrupted.clone())
    {
        eprintln!("could not install SIGTERM cleanup handler: {error}");
        return ExitCode::FAILURE;
    }
    let signal = interrupted.clone();
    if let Err(error) =
        ctrlc::set_handler(move || signal.store(true, std::sync::atomic::Ordering::Relaxed))
    {
        eprintln!("could not install SIGINT cleanup handler: {error}");
        return ExitCode::FAILURE;
    }
    match gatt::serve_auth(&adapter, duration, interrupted, config) {
        Ok(report) => {
            if json_output {
                println!(
                    "{}",
                    serde_json::to_string_pretty(&report).expect("report serializes")
                );
            } else {
                println!("Experimental authenticated research transport stopped.");
                println!("  adapter: {}", report.transport.adapter);
                println!("  runtime: {} seconds", report.transport.runtime_seconds);
                println!("  request/response protocol: OC-WATCH-AUTH-TRANSPORT/1");
                println!(
                    "  host SHA-256: {}",
                    format_fingerprint(&report.host_public_key_fingerprint_sha256)
                );
                println!(
                    "  Watch SHA-256: {}",
                    format_fingerprint(&report.watch_public_key_fingerprint_sha256)
                );
                match report.research_decision {
                    Some(decision) => println!("  signed research decision: {decision}"),
                    None => println!("  signed research decision: not received"),
                }
                println!("  authorization: never evaluated");
            }
            ExitCode::SUCCESS
        }
        Err(error) => {
            if json_output {
                println!(
                    "{}",
                    serde_json::to_string_pretty(&error.report).expect("report serializes")
                );
            } else {
                eprintln!(
                    "Authenticated research GATT experiment failed at {}: {}",
                    error.report.stage,
                    error.report.error.as_deref().unwrap_or("unknown error")
                );
                eprintln!("  authorization: never evaluated");
            }
            ExitCode::FAILURE
        }
    }
}

#[derive(Serialize)]
struct P009GeneratedHostKeyReport {
    status: &'static str,
    test_only: bool,
    host_key_file: String,
    host_fingerprint_sha256: String,
    authorization: &'static str,
}

fn run_generate_host_key(args: &[String]) -> ExitCode {
    let mut host_key_path = None;
    let mut experimental = false;
    let mut json_output = false;
    let mut index = 2;
    while index < args.len() {
        match args[index].as_str() {
            "--host-key-file" if index + 1 < args.len() && host_key_path.is_none() => {
                host_key_path = Some(args[index + 1].clone());
                index += 2;
            }
            "--experimental" if !experimental => {
                experimental = true;
                index += 1;
            }
            "--json" if !json_output => {
                json_output = true;
                index += 1;
            }
            _ => return usage_error(),
        }
    }
    let Some(host_key_path) = host_key_path else {
        return usage_error();
    };
    if !experimental {
        eprintln!("test host-key generation requires explicit --experimental opt-in");
        return ExitCode::from(2);
    }
    match create_test_host_key_file(&host_key_path) {
        Ok(fingerprint) => {
            let report = P009GeneratedHostKeyReport {
                status: "created_test_only_host_key",
                test_only: true,
                host_key_file: host_key_path,
                host_fingerprint_sha256: fingerprint,
                authorization: "never_evaluated",
            };
            if json_output {
                println!(
                    "{}",
                    serde_json::to_string_pretty(&report).expect("report serializes")
                );
            } else {
                println!("Created a temporary test_only P-256 host signing key.");
                println!(
                    "  file: {} (mode 0600; private scalar not printed)",
                    report.host_key_file
                );
                println!(
                    "  SHA-256: {}",
                    format_fingerprint(&report.host_fingerprint_sha256)
                );
                println!("  authorization: never evaluated");
            }
            ExitCode::SUCCESS
        }
        Err(error) => {
            eprintln!("could not create test host key: {error}");
            ExitCode::from(2)
        }
    }
}

fn create_test_host_key_file(path: &str) -> Result<String, String> {
    #[cfg(target_os = "linux")]
    {
        use std::{fs::OpenOptions, io::Write, os::unix::fs::OpenOptionsExt};

        let mut options = OpenOptions::new();
        options
            .write(true)
            .create_new(true)
            .mode(0o600)
            .custom_flags(libc::O_CLOEXEC | libc::O_NOFOLLOW);
        let mut file = options
            .open(path)
            .map_err(|error| format!("cannot create a new mode-0600 file at {path}: {error}"))?;
        let key = (|| -> Result<SigningKey, String> {
            for _ in 0..8 {
                let mut scalar = [0u8; 32];
                if let Err(error) = getrandom::getrandom(&mut scalar) {
                    scalar.fill(0);
                    return Err(format!("OS random source unavailable: {error}"));
                }
                let key = SigningKey::from_slice(&scalar).ok();
                scalar.fill(0);
                if let Some(key) = key {
                    return Ok(key);
                }
            }
            Err("OS random source did not produce a valid P-256 scalar".into())
        })();
        let key = match key {
            Ok(key) => key,
            Err(error) => {
                drop(file);
                let _ = fs::remove_file(path);
                return Err(error);
            }
        };
        let mut encoded_scalar = encode_hex(&key.to_bytes()).into_bytes();
        let public_key = key.verifying_key().to_encoded_point(false);
        let fingerprint = hex_digest(&Sha256::digest(public_key.as_bytes()));
        let write_result = file
            .write_all(&encoded_scalar)
            .and_then(|()| file.sync_all());
        encoded_scalar.fill(0);
        if let Err(error) = write_result {
            drop(file);
            let _ = fs::remove_file(path);
            return Err(format!("could not finish writing test key file: {error}"));
        }
        Ok(fingerprint)
    }
    #[cfg(not(target_os = "linux"))]
    {
        let _ = path;
        Err("temporary P009A host key generation is supported only on Linux".into())
    }
}

#[derive(Serialize)]
struct P009EnrollmentCheckReport {
    status: &'static str,
    host_identity: String,
    linux_uid: u32,
    watch_identity: String,
    host_fingerprint_sha256: String,
    watch_fingerprint_sha256: String,
    expires_at_unix_ms: u64,
    test_only: bool,
    authorization: &'static str,
}

fn run_check_enrollment(args: &[String]) -> ExitCode {
    let mut enrollment_path = None;
    let mut host_key_path = None;
    let mut experimental = false;
    let mut json_output = false;
    let mut index = 2;
    while index < args.len() {
        match args[index].as_str() {
            "--enrollments" if index + 1 < args.len() && enrollment_path.is_none() => {
                enrollment_path = Some(args[index + 1].clone());
                index += 2;
            }
            "--host-key-file" if index + 1 < args.len() && host_key_path.is_none() => {
                host_key_path = Some(args[index + 1].clone());
                index += 2;
            }
            "--experimental" if !experimental => {
                experimental = true;
                index += 1;
            }
            "--json" if !json_output => {
                json_output = true;
                index += 1;
            }
            _ => return usage_error(),
        }
    }
    let (Some(enrollment_path), Some(host_key_path)) = (enrollment_path, host_key_path) else {
        return usage_error();
    };
    if !experimental {
        eprintln!("enrollment check requires explicit --experimental opt-in");
        return ExitCode::from(2);
    }
    match build_auth_transport_config(&enrollment_path, &host_key_path) {
        Ok(config) => {
            let host_public = config
                .host_signing_key
                .verifying_key()
                .to_encoded_point(false);
            let host_fingerprint = hex_digest(&Sha256::digest(host_public.as_bytes()));
            let watch_key = match decode_hex(&config.watch_enrollment.public_key_sec1_hex) {
                Ok(key) => key,
                Err(error) => {
                    eprintln!("invalid Watch public pin: {error}");
                    return ExitCode::from(2);
                }
            };
            let report = P009EnrollmentCheckReport {
                status: "valid_test_only_enrollment",
                host_identity: config.host_enrollment.host_identity.clone(),
                linux_uid: config.host_enrollment.linux_uid,
                watch_identity: config.watch_enrollment.watch_identity.clone(),
                host_fingerprint_sha256: host_fingerprint,
                watch_fingerprint_sha256: hex_digest(&Sha256::digest(watch_key)),
                expires_at_unix_ms: config.enrollment_expires_at_unix_ms,
                test_only: true,
                authorization: "never_evaluated",
            };
            if json_output {
                println!(
                    "{}",
                    serde_json::to_string_pretty(&report).expect("report serializes")
                );
            } else {
                println!(
                    "P009A test enrollment is complete. Verify each fingerprint with the Watch operator before starting."
                );
                println!(
                    "host: {} / Linux UID {}",
                    report.host_identity, report.linux_uid
                );
                println!(
                    "host SHA-256: {}",
                    format_fingerprint(&report.host_fingerprint_sha256)
                );
                println!("Watch: {}", report.watch_identity);
                println!(
                    "Watch SHA-256: {}",
                    format_fingerprint(&report.watch_fingerprint_sha256)
                );
                println!("expires at Unix ms: {}", report.expires_at_unix_ms);
                println!("confirmation values (64 lowercase hex):");
                println!("  --confirm-host-sha256 {}", report.host_fingerprint_sha256);
                println!(
                    "  --confirm-watch-sha256 {}",
                    report.watch_fingerprint_sha256
                );
                println!("test_only: true; authorization: never evaluated");
            }
            ExitCode::SUCCESS
        }
        Err(error) => {
            eprintln!("P009A enrollment check failed: {error}");
            ExitCode::from(2)
        }
    }
}

fn run_export_host_pin(args: &[String]) -> ExitCode {
    let mut enrollment_id = None;
    let mut host_identity = None;
    let mut linux_uid = None;
    let mut host_key_path = None;
    let mut experimental = false;
    let mut json_output = false;
    let mut index = 2;
    while index < args.len() {
        match args[index].as_str() {
            "--host-enrollment-id" if index + 1 < args.len() && enrollment_id.is_none() => {
                enrollment_id = Some(args[index + 1].clone());
                index += 2;
            }
            "--host-identity" if index + 1 < args.len() && host_identity.is_none() => {
                host_identity = Some(args[index + 1].clone());
                index += 2;
            }
            "--linux-uid" if index + 1 < args.len() && linux_uid.is_none() => {
                linux_uid = args[index + 1].parse::<u32>().ok();
                if linux_uid.is_none() {
                    return usage_error();
                }
                index += 2;
            }
            "--host-key-file" if index + 1 < args.len() && host_key_path.is_none() => {
                host_key_path = Some(args[index + 1].clone());
                index += 2;
            }
            "--experimental" if !experimental => {
                experimental = true;
                index += 1;
            }
            "--json" if !json_output => {
                json_output = true;
                index += 1;
            }
            _ => return usage_error(),
        }
    }
    let (Some(enrollment_id), Some(host_identity), Some(linux_uid), Some(host_key_path)) =
        (enrollment_id, host_identity, linux_uid, host_key_path)
    else {
        return usage_error();
    };
    if !experimental {
        eprintln!("host pin export requires explicit --experimental opt-in");
        return ExitCode::from(2);
    }
    let result = (|| -> Result<P009HostPinDocument, String> {
        validate_research_identifier(&enrollment_id)?;
        validate_research_identifier(&host_identity)?;
        if linux_uid != current_linux_uid()? {
            return Err("--linux-uid must match the effective UID running this command".into());
        }
        let signing_key = load_test_host_signing_key(&host_key_path)?;
        let public_key = signing_key.verifying_key().to_encoded_point(false);
        let public_key_sec1_hex = encode_hex(public_key.as_bytes());
        let host_enrollment = HostEnrollment {
            enrollment_id,
            host_identity,
            linux_uid,
            public_key_sec1_hex,
            key_algorithm: opencontinuity_core::watch_auth_research::KeyAlgorithm::P256EcdsaSha256,
            key_version: 1,
            state: EnrollmentState::Active,
            test_only: true,
        };
        let now_ms = unix_now_ms()?;
        let expires_at_unix_ms = now_ms
            .checked_add(gatt::MAX_TEST_ENROLLMENT_LIFETIME_MS)
            .ok_or_else(|| "test host pin expiry overflow".to_owned())?;
        let fingerprint = hex_digest(&Sha256::digest(public_key.as_bytes()));
        Ok(P009HostPinDocument {
            schema_version: 1,
            expires_at_unix_ms,
            host_enrollment,
            host_fingerprint_sha256: fingerprint,
        })
    })();
    match result {
        Ok(document) => {
            if json_output {
                println!(
                    "{}",
                    serde_json::to_string_pretty(&document).expect("host pin serializes")
                );
            } else {
                println!(
                    "P009A test-only host pin; public data only. Verify this SHA-256 fingerprint through a separate channel before importing it on the Watch."
                );
                println!(
                    "host: {} ({})",
                    document.host_enrollment.host_identity, document.host_enrollment.enrollment_id
                );
                println!("Linux UID: {}", document.host_enrollment.linux_uid);
                println!(
                    "SHA-256 fingerprint: {}",
                    format_fingerprint(&document.host_fingerprint_sha256)
                );
                println!("expires at Unix ms: {}", document.expires_at_unix_ms);
                println!("public host pin JSON follows:");
                println!(
                    "{}",
                    serde_json::to_string_pretty(&document).expect("host pin serializes")
                );
                println!("authorization: never evaluated");
            }
            ExitCode::SUCCESS
        }
        Err(error) => {
            eprintln!("could not export host pin: {error}");
            ExitCode::from(2)
        }
    }
}

fn unix_now_ms() -> Result<u64, String> {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis().min(u64::MAX as u128) as u64)
        .map_err(|_| "system clock is before the Unix epoch".to_owned())
}

fn current_linux_uid() -> Result<u32, String> {
    #[cfg(target_os = "linux")]
    {
        // SAFETY: geteuid has no pointer arguments or memory-safety preconditions.
        Ok(unsafe { libc::geteuid() })
    }
    #[cfg(not(target_os = "linux"))]
    {
        Err("P009A host enrollment is available only on Linux".into())
    }
}

fn hex_digest(bytes: &[u8]) -> String {
    encode_hex(bytes)
}

fn validate_fingerprint_confirmation(
    key_label: &str,
    confirmed: &str,
    expected: &str,
) -> Result<(), String> {
    if confirmed.len() != 64
        || !confirmed
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    {
        return Err(format!(
            "{key_label} confirmation must be 64 lowercase hexadecimal characters"
        ));
    }
    if confirmed != expected {
        return Err(format!(
            "{key_label} fingerprint does not match the active test enrollment"
        ));
    }
    Ok(())
}

fn format_fingerprint(hex: &str) -> String {
    hex.as_bytes()
        .chunks(4)
        .map(|chunk| std::str::from_utf8(chunk).unwrap_or("????"))
        .collect::<Vec<_>>()
        .join(":")
}

fn validate_research_identifier(value: &str) -> Result<(), String> {
    if value.is_empty()
        || value.len() > 128
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_' | b':' | b'-'))
    {
        return Err(
            "research identity must be 1–128 ASCII letters, digits, '.', '_', ':' or '-'".into(),
        );
    }
    Ok(())
}

fn build_auth_transport_config(
    enrollments_path: &str,
    host_key_path: &str,
) -> Result<gatt::AuthTransportConfig, String> {
    let mut input = String::new();
    fs::File::open(enrollments_path)
        .map_err(|error| format!("could not read enrollment pins: {error}"))?
        .take(16 * 1024 + 1)
        .read_to_string(&mut input)
        .map_err(|error| format!("could not read enrollment pins: {error}"))?;
    if input.len() > 16 * 1024 {
        return Err("enrollment file exceeds 16 KiB".into());
    }
    let pins: P009EnrollmentPins = serde_json::from_str(&input)
        .map_err(|error| format!("invalid enrollment file: {error}"))?;
    if pins.schema_version != 2 {
        return Err("unsupported enrollment schema".into());
    }
    let enrollment_now_ms = unix_now_ms()?;
    if pins.expires_at_unix_ms <= enrollment_now_ms {
        return Err(
            "test enrollment is expired; export and pin a fresh test-only enrollment".into(),
        );
    }
    if pins.expires_at_unix_ms - enrollment_now_ms > gatt::MAX_TEST_ENROLLMENT_LIFETIME_MS {
        return Err("test enrollment exceeds the one-hour validity limit".into());
    }
    if pins.host_enrollment.linux_uid != current_linux_uid()? {
        return Err(
            "enrollment Linux UID does not match the effective UID running this command".into(),
        );
    }
    validate_research_identifier(&pins.host_enrollment.enrollment_id)?;
    validate_research_identifier(&pins.host_enrollment.host_identity)?;
    validate_research_identifier(&pins.watch_enrollment.enrollment_id)?;
    validate_research_identifier(&pins.watch_enrollment.watch_identity)?;
    validate_research_identifier(&pins.watch_enrollment.bound_host_enrollment_id)?;
    validate_research_identifier(&pins.watch_enrollment.bound_host_identity)?;
    if !pins.host_enrollment.test_only || !pins.watch_enrollment.test_only {
        return Err("P009 currently accepts test-only enrollment records only".into());
    }
    if pins.host_enrollment.state != EnrollmentState::Active
        || pins.watch_enrollment.state != EnrollmentState::Active
        || pins.host_enrollment.key_algorithm
            != opencontinuity_core::watch_auth_research::KeyAlgorithm::P256EcdsaSha256
        || pins.watch_enrollment.key_algorithm
            != opencontinuity_core::watch_auth_research::KeyAlgorithm::P256EcdsaSha256
        || pins.host_enrollment.key_version != 1
        || pins.watch_enrollment.key_version != 1
        || pins.watch_enrollment.bound_host_enrollment_id != pins.host_enrollment.enrollment_id
        || pins.watch_enrollment.bound_host_identity != pins.host_enrollment.host_identity
        || pins.watch_enrollment.bound_linux_uid != pins.host_enrollment.linux_uid
    {
        return Err("Watch enrollment is inactive or is not bound to this host/account".into());
    }
    let signing_key = load_test_host_signing_key(host_key_path)?;
    let actual_host_key = encode_hex(
        signing_key
            .verifying_key()
            .to_encoded_point(false)
            .as_bytes(),
    );
    if actual_host_key != pins.host_enrollment.public_key_sec1_hex {
        return Err("host key file does not match the independently pinned host public key".into());
    }
    let watch_key = decode_hex(&pins.watch_enrollment.public_key_sec1_hex)?;
    if encode_hex(&watch_key) != pins.watch_enrollment.public_key_sec1_hex {
        return Err("Watch public key must use canonical lowercase hexadecimal SEC1 bytes".into());
    }
    p256::ecdsa::VerifyingKey::from_sec1_bytes(&watch_key).map_err(|_| {
        "Watch enrollment public key is incomplete or invalid P-256 SEC1".to_owned()
    })?;
    Ok(gatt::AuthTransportConfig {
        host_signing_key: signing_key,
        host_enrollment: pins.host_enrollment,
        watch_enrollment: pins.watch_enrollment,
        enrollment_expires_at_unix_ms: pins.expires_at_unix_ms,
    })
}

fn load_test_host_signing_key(path: &str) -> Result<SigningKey, String> {
    #[cfg(unix)]
    let mut file = {
        use std::os::unix::fs::OpenOptionsExt;
        fs::OpenOptions::new()
            .read(true)
            .custom_flags(libc::O_NOFOLLOW | libc::O_CLOEXEC)
            .open(path)
            .map_err(|error| format!("could not safely open host key file: {error}"))?
    };
    #[cfg(not(unix))]
    let mut file = fs::File::open(path)
        .map_err(|error| format!("could not safely open host key file: {error}"))?;
    let metadata = file
        .metadata()
        .map_err(|error| format!("could not inspect opened host key file: {error}"))?;
    if !metadata.file_type().is_file() {
        return Err("host signing key must be a regular file".into());
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::{MetadataExt, PermissionsExt};
        if metadata.permissions().mode() & 0o077 != 0 {
            return Err(
                "host signing key file must not be accessible to group or other users".into(),
            );
        }
        if metadata.uid() != current_linux_uid()? {
            return Err(
                "host key file must be owned by the effective Linux UID running this command"
                    .into(),
            );
        }
    }
    let mut encoded = Vec::new();
    file.by_ref()
        .take(65)
        .read_to_end(&mut encoded)
        .map_err(|error| format!("could not read host key file: {error}"))?;
    if encoded.len() != 64 {
        return Err(
            "host key file must contain exactly 32 bytes as 64 lowercase hexadecimal characters"
                .into(),
        );
    }
    let text = std::str::from_utf8(&encoded)
        .map_err(|_| "host key file is not lowercase hexadecimal".to_owned())?;
    let raw = decode_hex(text)?;
    SigningKey::from_slice(&raw)
        .map_err(|_| "host key file is not a valid P-256 private scalar".into())
}

#[derive(Serialize)]
#[serde(deny_unknown_fields)]
struct P009RequestTransportFixture {
    schema_version: u8,
    transfer_id_hex: String,
    request_envelope_hex: String,
    request_frames_hex: Vec<String>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct P009ResponseTransportFixture {
    schema_version: u8,
    transfer_id_hex: String,
    response_envelope_hex: String,
    response_frames_hex: Vec<String>,
}

#[derive(Serialize)]
#[serde(deny_unknown_fields)]
struct P009TransportReport {
    schema_version: u8,
    protocol: &'static str,
    request_signature_verified: bool,
    response_signature_verified: bool,
    action: &'static str,
    decision: &'static str,
    result: &'static str,
    authorization: &'static str,
}

fn run_watch_auth_transport(args: &[String]) -> ExitCode {
    match args.get(1).map(String::as_str) {
        Some("prepare") => {
            if args.len() != 6 || args[2] != "--fixture" || args[4] != "--output" {
                return usage_error();
            }
            let fixture_path = args[3].clone();
            let output_path = args[5].clone();
            match prepare_p009_transport_fixture(&fixture_path) {
                Ok(output) => match serde_json::to_vec_pretty(&output) {
                    Ok(bytes) => match fs::write(&output_path, bytes) {
                        Ok(()) => {
                            println!(
                                "Serialized Rust host request frames written to {output_path}"
                            );
                            ExitCode::SUCCESS
                        }
                        Err(error) => {
                            eprintln!("could not write transport fixture: {error}");
                            ExitCode::FAILURE
                        }
                    },
                    Err(error) => {
                        eprintln!("could not encode transport fixture: {error}");
                        ExitCode::FAILURE
                    }
                },
                Err(error) => {
                    eprintln!("P009 transport fixture preparation failed: {error}");
                    ExitCode::FAILURE
                }
            }
        }
        Some("verify-response") => {
            if (args.len() != 6 && args.len() != 7)
                || args[2] != "--fixture"
                || args[4] != "--input"
                || (args.len() == 7 && args[6] != "--json")
            {
                return usage_error();
            }
            let fixture_path = args[3].clone();
            let input_path = args[5].clone();
            let json_output = args.len() == 7;
            match verify_p009_transport_response(&fixture_path, &input_path) {
                Ok(report) if json_output => match serde_json::to_string_pretty(&report) {
                    Ok(json) => {
                        println!("{json}");
                        ExitCode::SUCCESS
                    }
                    Err(error) => {
                        eprintln!("could not serialize P009 report: {error}");
                        ExitCode::FAILURE
                    }
                },
                Ok(report) => {
                    println!("P009 serialized transport round trip:");
                    println!(
                        "  request signature verified: {}",
                        report.request_signature_verified
                    );
                    println!(
                        "  response signature verified: {}",
                        report.response_signature_verified
                    );
                    println!("  action: {}", report.action);
                    println!("  decision: {}", report.decision);
                    println!("  result: {}", report.result);
                    println!("  authorization: never evaluated");
                    ExitCode::SUCCESS
                }
                Err(error) => {
                    eprintln!("P009 response verification failed: {error}");
                    ExitCode::FAILURE
                }
            }
        }
        _ => usage_error(),
    }
}

fn read_p008_fixture(path: &str) -> Result<WatchAuthFixture, String> {
    let mut input = String::new();
    fs::File::open(path)
        .map_err(|error| format!("could not read fixture {path}: {error}"))?
        .take(16 * 1024 + 1)
        .read_to_string(&mut input)
        .map_err(|error| format!("could not read fixture {path}: {error}"))?;
    if input.len() > 16 * 1024 {
        return Err("watch-auth fixture exceeds 16 KiB".into());
    }
    WatchAuthFixture::from_json(&input).map_err(|error| error.to_string())
}

fn prepare_p009_transport_fixture(path: &str) -> Result<P009RequestTransportFixture, String> {
    let fixture = read_p008_fixture(path)?;
    if !fixture.host_enrollment.test_only || !fixture.watch_enrollment.test_only {
        return Err("software transport integration accepts test-only enrollments only".into());
    }
    let canonical = decode_hex(&fixture.request.canonical_hex)?;
    let request =
        HostAuthRequest::parse_canonical(&canonical).map_err(|error| error.to_string())?;
    let signature = decode_hex(&fixture.request.signature_raw_hex)?;
    let verified = verify_host_request(
        Some(&fixture.host_enrollment),
        request,
        &signature,
        &fixture.watch_enrollment.enrollment_id,
        fixture.now_ms,
        &mut ReplayGuard::default(),
    )
    .map_err(|error| error.to_string())?;
    let envelope = SignedEnvelope::new(AuthTransportDirection::HostToWatch, &canonical, &signature)
        .map_err(|error| format!("invalid signed envelope: {error:?}"))?;
    let transfer_id = [0xa5; 8];
    let frames = encode_auth_frames(
        AuthTransportDirection::HostToWatch,
        transfer_id,
        &envelope,
        23,
    )
    .map_err(|error| format!("could not encode request frames: {error:?}"))?;
    let _ = verified; // Token construction proves the host request is authentic and fresh in this fixture.
    Ok(P009RequestTransportFixture {
        schema_version: 1,
        transfer_id_hex: encode_hex(&transfer_id),
        request_envelope_hex: encode_hex(&envelope.as_bytes()),
        request_frames_hex: frames.iter().map(|frame| encode_hex(frame)).collect(),
    })
}

fn verify_p009_transport_response(
    fixture_path: &str,
    response_path: &str,
) -> Result<P009TransportReport, String> {
    let fixture = read_p008_fixture(fixture_path)?;
    if !fixture.host_enrollment.test_only || !fixture.watch_enrollment.test_only {
        return Err("software transport integration accepts test-only enrollments only".into());
    }
    let mut response_bytes = Vec::new();
    fs::File::open(response_path)
        .map_err(|error| format!("could not read response frames: {error}"))?
        .take(32 * 1024 + 1)
        .read_to_end(&mut response_bytes)
        .map_err(|error| format!("could not read response frames: {error}"))?;
    if response_bytes.len() > 32 * 1024 {
        return Err("response transport fixture exceeds 32 KiB".into());
    }
    let input: P009ResponseTransportFixture = serde_json::from_slice(&response_bytes)
        .map_err(|error| format!("invalid response transport fixture: {error}"))?;
    if input.schema_version != 1 {
        return Err("unsupported transport fixture version".into());
    }
    let request_canonical = decode_hex(&fixture.request.canonical_hex)?;
    let request =
        HostAuthRequest::parse_canonical(&request_canonical).map_err(|error| error.to_string())?;
    let request_signature = decode_hex(&fixture.request.signature_raw_hex)?;
    let verified_request = verify_host_request(
        Some(&fixture.host_enrollment),
        request,
        &request_signature,
        &fixture.watch_enrollment.enrollment_id,
        fixture.now_ms,
        &mut ReplayGuard::default(),
    )
    .map_err(|error| format!("host request signature or binding failed: {error}"))?;

    let transfer_id_bytes = decode_hex(&input.transfer_id_hex)?;
    let transfer_id: [u8; 8] = transfer_id_bytes
        .as_slice()
        .try_into()
        .map_err(|_| "transport session id must be eight bytes".to_owned())?;
    let now = Instant::now();
    let mut assembler = TransferAssembler::new(
        transfer_id,
        AuthTransportDirection::WatchToHost,
        "watch-ci-client",
        now,
    );
    for frame_hex in &input.response_frames_hex {
        let frame = decode_hex(frame_hex)?;
        assembler
            .push(
                "watch-ci-client",
                AuthTransportDirection::WatchToHost,
                &frame,
                23,
                now,
            )
            .map_err(|error| format!("invalid response frame: {error:?}"))?;
    }
    let envelope = assembler
        .complete("watch-ci-client", transfer_id, now)
        .map_err(|error| format!("response transfer did not complete: {error:?}"))?;
    if encode_hex(&envelope.as_bytes()) != input.response_envelope_hex {
        return Err("response bytes differ from the reconstructed transport payload".into());
    }
    let response = WatchAuthResponse::parse_canonical(envelope.canonical())
        .map_err(|error| error.to_string())?;
    verify_watch_response(
        &fixture.host_enrollment,
        Some(&fixture.watch_enrollment),
        &verified_request,
        &response,
        envelope.signature(),
        fixture.now_ms,
        &mut ReplayGuard::default(),
    )
    .map_err(|error| format!("Watch signature or response binding failed: {error}"))?;
    let decision = match response.decision {
        opencontinuity_core::watch_auth_research::Decision::Approved => "approved",
        opencontinuity_core::watch_auth_research::Decision::Rejected => "rejected",
    };
    Ok(P009TransportReport {
        schema_version: 1,
        protocol: opencontinuity_core::watch_auth_transport::PROTOCOL_NAME,
        request_signature_verified: true,
        response_signature_verified: true,
        action: "research_confirmation",
        decision,
        result: if decision == "approved" {
            "research_approval_verified"
        } else {
            "research_rejection_verified"
        },
        authorization: "never_evaluated",
    })
}

fn decode_hex(input: &str) -> Result<Vec<u8>, String> {
    if input.is_empty() || input.len() % 2 != 0 || !input.bytes().all(|b| !b.is_ascii_uppercase()) {
        return Err("hex field must be nonempty, even-length, and lowercase".into());
    }
    input
        .as_bytes()
        .chunks_exact(2)
        .map(|pair| {
            let high = hex_nibble(pair[0]).ok_or_else(|| "invalid hexadecimal data".to_owned())?;
            let low = hex_nibble(pair[1]).ok_or_else(|| "invalid hexadecimal data".to_owned())?;
            Ok((high << 4) | low)
        })
        .collect()
}

fn encode_hex(bytes: &[u8]) -> String {
    const DIGITS: &[u8; 16] = b"0123456789abcdef";
    let mut output = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        output.push(DIGITS[(byte >> 4) as usize] as char);
        output.push(DIGITS[(byte & 0x0f) as usize] as char);
    }
    output
}

fn hex_nibble(byte: u8) -> Option<u8> {
    match byte {
        b'0'..=b'9' => Some(byte - b'0'),
        b'a'..=b'f' => Some(byte - b'a' + 10),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn doctor_output_contract_is_valid_json() {
        let output =
            serde_json::to_string(&DoctorReport::current(env!("CARGO_PKG_VERSION"))).unwrap();
        let value: serde_json::Value = serde_json::from_str(&output).unwrap();
        assert_eq!(value["project"], "OpenContinuity");
        assert_eq!(value["schema_version"], 1);
        assert_eq!(value["capabilities"].as_array().unwrap().len(), 3);
        assert_eq!(
            value["watch_auto_unlock_trust"]["device_detection"],
            "unknown"
        );
    }

    #[test]
    fn fixture_report_is_read_only_json() {
        let fixture = r#"{"schema_version":1,"observations":[{"observation_id":"sample-1","untrusted_name":"Nearby label","rssi_dbm":-61,"manufacturer_company_ids":[76]}]}"#;
        let parsed = ResearchFixture::from_json(fixture).unwrap();
        let mut adapter = MockDiscoveryAdapter::new(parsed).unwrap();
        let report = read_only_report(&mut adapter).unwrap();
        let value = serde_json::to_value(report).unwrap();
        assert_eq!(value["authorization"], "never_evaluated");
        assert_eq!(value["source"], "mock_fixture");
        assert_eq!(value["observations"][0]["rssi_dbm"], -61);
        assert!(value.get("trusted_identity").is_none());
    }

    #[test]
    fn fingerprint_confirmation_requires_exact_canonical_sha256_hex() {
        let expected = "a".repeat(64);
        assert!(validate_fingerprint_confirmation("host", &expected, &expected).is_ok());
        assert!(
            validate_fingerprint_confirmation("host", &expected.to_uppercase(), &expected).is_err()
        );
        assert!(validate_fingerprint_confirmation("host", &"b".repeat(64), &expected).is_err());
        assert!(validate_fingerprint_confirmation("host", "aa:bb", &expected).is_err());
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn generated_host_key_is_new_private_mode_0600_and_never_overwrites() {
        use std::os::unix::fs::PermissionsExt;

        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let path = env::temp_dir().join(format!(
            "opencontinuity-p009a-host-key-{}-{unique}",
            std::process::id()
        ));
        let path_text = path.to_string_lossy().into_owned();
        let fingerprint = create_test_host_key_file(&path_text).unwrap();
        let metadata = fs::metadata(&path).unwrap();
        let private_scalar_hex = fs::read(&path).unwrap();
        assert_eq!(metadata.permissions().mode() & 0o777, 0o600);
        assert_eq!(private_scalar_hex.len(), 64);
        assert!(
            private_scalar_hex
                .iter()
                .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(byte))
        );
        assert!(!private_scalar_hex.contains(&b'\n'));
        let scalar = decode_hex(std::str::from_utf8(&private_scalar_hex).unwrap()).unwrap();
        let key = SigningKey::from_slice(&scalar).unwrap();
        assert_eq!(
            fingerprint,
            hex_digest(&Sha256::digest(
                key.verifying_key().to_encoded_point(false).as_bytes()
            ))
        );

        let before = private_scalar_hex.clone();
        assert!(create_test_host_key_file(&path_text).is_err());
        assert_eq!(fs::read(&path).unwrap(), before);
        fs::remove_file(path).unwrap();
    }
}
