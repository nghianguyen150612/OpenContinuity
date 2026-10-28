use opencontinuity_core::{
    DoctorReport,
    research::{MockDiscoveryAdapter, ResearchFixture, read_only_report},
    watch_endpoint::{WatchEndpointFixture, simulate},
};
use std::{env, fs, io::Read, process::ExitCode};

const HELP: &str = "OpenContinuity — native Apple ecosystem compatibility for Linux and beyond\n\nUsage: opencontinuity [--help] [--version] [doctor [--json]]\n       opencontinuity research watch --fixture <path> [--json]\n       opencontinuity research watch-endpoint --fixture <path> [--json]\n\nCommands:\n  doctor         Report this build's capability states without scanning devices\n  research       Read sanitized offline protocol research fixtures\n  --help         Print this help\n  --version      Print version\n\nResearch commands use local mock fixtures only. They do not access Bluetooth or a network, authenticate an Apple Watch, or unlock Linux.\n";

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
        _ => usage_error(),
    }
}

fn run_research(args: Vec<String>) -> ExitCode {
    match args.first().map(String::as_str) {
        Some("watch") => run_watch_research(&args),
        Some("watch-endpoint") => run_watch_endpoint_research(&args),
        _ => usage_error(),
    }
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
}
