use std::{
    process::ExitCode,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    time::Duration,
};

const HELP: &str = "OpenContinuity background service\n\nUsage: opencontinuityd [--help] [--version]\n\nStarts a nonprivileged foreground process. P001 performs no scanning,\nauthentication changes, or device writes. Stop with Ctrl-C or SIGTERM.\n";

fn main() -> ExitCode {
    let mut args = std::env::args().skip(1);
    match args.next().as_deref() {
        Some("--help" | "-h") => {
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
            println!("opencontinuityd {}", env!("CARGO_PKG_VERSION"));
            ExitCode::SUCCESS
        }
        None => run_foreground(),
        _ => usage_error(),
    }
}

fn run_foreground() -> ExitCode {
    let running = Arc::new(AtomicBool::new(true));
    let signal_state = Arc::clone(&running);
    if let Err(error) = ctrlc::set_handler(move || signal_state.store(false, Ordering::SeqCst)) {
        eprintln!("could not install shutdown handler: {error}");
        return ExitCode::FAILURE;
    }
    println!(
        "opencontinuityd {} running in foreground (P001 safe idle mode)",
        env!("CARGO_PKG_VERSION")
    );
    while running.load(Ordering::SeqCst) {
        std::thread::sleep(Duration::from_millis(200));
    }
    println!("opencontinuityd stopped");
    ExitCode::SUCCESS
}

fn usage_error() -> ExitCode {
    eprintln!("{HELP}");
    ExitCode::from(2)
}
