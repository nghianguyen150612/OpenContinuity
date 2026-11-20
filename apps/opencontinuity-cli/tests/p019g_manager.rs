use std::io::BufRead;
use std::path::Path;
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

const INSTANCE: &str = "01010101010101010101010101010101";
const SESSION: &str = "02020202020202020202020202020202";
const EPOCH: &str = "03030303030303030303030303030303";

fn manager_binary() -> &'static str {
    env!("CARGO_BIN_EXE_p019g-manager")
}

fn wait_exit(child: &mut Child, timeout: Duration) -> std::process::ExitStatus {
    let deadline = Instant::now() + timeout;
    loop {
        if let Some(status) = child.try_wait().unwrap() {
            return status;
        }
        assert!(
            Instant::now() < deadline,
            "child process did not exit in time"
        );
        std::thread::yield_now();
    }
}

fn serve_command(socket: &Path, supervisor_pid: u32) -> Command {
    let mut command = Command::new(manager_binary());
    command
        .arg("serve")
        .arg("--socket")
        .arg(socket)
        .arg("--supervisor-pid")
        .arg(supervisor_pid.to_string())
        .arg("--supervisor-uid")
        .arg(unsafe { libc::geteuid() }.to_string())
        .arg("--instance")
        .arg(INSTANCE)
        .arg("--session")
        .arg(SESSION)
        .arg("--epoch")
        .arg(EPOCH)
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    command
}

fn supervisor_command(socket: &Path, hold: bool) -> Command {
    let mut command = Command::new(manager_binary());
    command
        .arg("test-supervisor")
        .arg("--socket")
        .arg(socket)
        .arg("--instance")
        .arg(INSTANCE)
        .arg("--session")
        .arg(SESSION)
        .arg("--epoch")
        .arg(EPOCH)
        .arg("--sequence")
        .arg("1");
    if hold {
        command.arg("--hold");
    }
    command
}

#[test]
fn manager_authenticates_real_supervisor_and_observes_pidfd_death() {
    let root = tempfile::tempdir().unwrap();
    let socket = root.path().join("guardian.sock");
    let mut supervisor = supervisor_command(&socket, false)
        .stdout(Stdio::piped())
        .spawn()
        .unwrap();
    let mut guardian = serve_command(&socket, supervisor.id()).spawn().unwrap();

    let mut output = String::new();
    std::io::BufReader::new(supervisor.stdout.take().unwrap())
        .read_line(&mut output)
        .unwrap();
    assert!(output.contains("supervisor acknowledged"));
    assert!(wait_exit(&mut supervisor, Duration::from_secs(2)).success());
    assert!(!wait_exit(&mut guardian, Duration::from_secs(2)).success());
    assert!(socket.with_extension("p019g-reserved").exists());
}

#[test]
fn guardian_crash_does_not_kill_supervisor_and_keeps_stale_reservation() {
    let root = tempfile::tempdir().unwrap();
    let socket = root.path().join("guardian.sock");
    let mut supervisor = supervisor_command(&socket, true)
        .stdout(Stdio::piped())
        .spawn()
        .unwrap();
    let mut guardian = serve_command(&socket, supervisor.id()).spawn().unwrap();

    let mut output = String::new();
    std::io::BufReader::new(supervisor.stdout.take().unwrap())
        .read_line(&mut output)
        .unwrap();
    assert!(output.contains("supervisor acknowledged"));
    guardian.kill().unwrap();
    let _ = guardian.wait();
    assert!(supervisor.try_wait().unwrap().is_none());
    supervisor.kill().unwrap();
    let _ = supervisor.wait();
    assert!(socket.with_extension("p019g-reserved").exists());

    // A restart cannot infer that the old session is safe from its pathname.
    let restart = serve_command(&socket, unsafe { libc::getpid() as u32 })
        .status()
        .unwrap();
    assert!(!restart.success());
}
