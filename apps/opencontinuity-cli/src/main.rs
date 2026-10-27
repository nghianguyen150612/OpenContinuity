use opencontinuity_core::DoctorReport;
use std::{env, process::ExitCode};

const HELP: &str = "OpenContinuity — native Apple ecosystem compatibility for Linux and beyond\n\nUsage: opencontinuity [--help] [--version] [doctor [--json]]\n\nCommands:\n  doctor       Report this build's capability states without scanning devices\n  --help       Print this help\n  --version    Print version\n";

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
        _ => usage_error(),
    }
}

fn usage_error() -> ExitCode {
    eprintln!("{HELP}");
    ExitCode::from(2)
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
}
