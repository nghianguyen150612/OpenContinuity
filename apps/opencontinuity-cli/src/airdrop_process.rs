//! Bounded subprocess helpers for read-only probes of an installed backend.

use std::ffi::OsStr;
use std::io::{self, Read};
use std::path::Path;
use std::process::{Child, Command, ExitStatus, Stdio};
use std::thread;
use std::time::{Duration, Instant};

pub const MAX_RECEIVE_TERMINAL_OUTPUT: usize = 64 * 1024;

#[derive(Debug)]
pub struct CommandOutput {
    pub status: ExitStatus,
    pub stdout: Vec<u8>,
    _stderr: Vec<u8>,
    pub stdout_truncated: bool,
    pub stderr_truncated: bool,
    pub timed_out: bool,
}

struct Captured {
    bytes: Vec<u8>,
    truncated: bool,
}

fn drain_bounded<R: Read>(mut reader: R, limit: usize) -> io::Result<Captured> {
    let mut captured = Vec::with_capacity(limit.min(8192));
    let mut scratch = [0_u8; 4096];
    let mut truncated = false;
    loop {
        let n = reader.read(&mut scratch)?;
        if n == 0 {
            break;
        }
        let room = limit.saturating_sub(captured.len());
        captured.extend_from_slice(&scratch[..n.min(room)]);
        truncated |= n > room;
        // Continue draining after the cap so a noisy child cannot block on a
        // full pipe. Memory stays bounded by `limit` per stream.
    }
    Ok(Captured {
        bytes: captured,
        truncated,
    })
}

/// Forward interactive backend text to the local terminal without retaining
/// it. Both child streams share one byte budget; readers keep draining after
/// the cap so the child cannot block on a full pipe, and the caller stops the
/// process as soon as `exceeded` is set.
pub fn relay_bounded<R: Read, W: io::Write>(
    mut reader: R,
    mut writer: W,
    bytes_written: &std::sync::atomic::AtomicUsize,
    exceeded: &std::sync::atomic::AtomicBool,
    failed: &std::sync::atomic::AtomicBool,
    limit: usize,
) -> io::Result<()> {
    let mut scratch = [0_u8; 2048];
    loop {
        let count = reader.read(&mut scratch)?;
        if count == 0 {
            return Ok(());
        }
        let mut current = bytes_written.load(std::sync::atomic::Ordering::Relaxed);
        let allowed = loop {
            let allowed = limit.saturating_sub(current).min(count);
            match bytes_written.compare_exchange_weak(
                current,
                current + allowed,
                std::sync::atomic::Ordering::Relaxed,
                std::sync::atomic::Ordering::Relaxed,
            ) {
                Ok(_) => break allowed,
                Err(observed) => current = observed,
            }
        };
        if allowed > 0 && writer.write_all(&scratch[..allowed]).is_err() {
            failed.store(true, std::sync::atomic::Ordering::Relaxed);
            // Continue draining the child pipe. The supervisor sees `exceeded`
            // and shuts down the process without waiting on terminal output.
        }
        if count > allowed {
            exceeded.store(true, std::sync::atomic::Ordering::Relaxed);
        }
    }
}

/// Run a fixed executable and argument vector with drained, capped output and
/// a wall-clock deadline. No shell or inherited environment expansion is used.
pub fn run_bounded<I, S>(
    executable: &Path,
    arguments: I,
    timeout: Duration,
    output_limit_per_stream: usize,
) -> io::Result<CommandOutput>
where
    I: IntoIterator<Item = S>,
    S: AsRef<OsStr>,
{
    if !executable.is_absolute() || output_limit_per_stream == 0 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "invalid bounded command configuration",
        ));
    }
    let mut child = Command::new(executable)
        .args(arguments)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()?;
    collect_child(&mut child, timeout, output_limit_per_stream)
}

fn collect_child(
    child: &mut Child,
    timeout: Duration,
    output_limit_per_stream: usize,
) -> io::Result<CommandOutput> {
    let stdout = child
        .stdout
        .take()
        .ok_or_else(|| io::Error::other("stdout pipe unavailable"))?;
    let stderr = child
        .stderr
        .take()
        .ok_or_else(|| io::Error::other("stderr pipe unavailable"))?;
    let stdout_reader = thread::spawn(move || drain_bounded(stdout, output_limit_per_stream));
    let stderr_reader = thread::spawn(move || drain_bounded(stderr, output_limit_per_stream));

    let deadline = Instant::now() + timeout;
    let mut timed_out = false;
    let status = loop {
        if let Some(status) = child.try_wait()? {
            break status;
        }
        if Instant::now() >= deadline {
            timed_out = true;
            let _ = child.kill();
            break child.wait()?;
        }
        thread::sleep(Duration::from_millis(10));
    };
    let stdout = stdout_reader
        .join()
        .map_err(|_| io::Error::other("stdout reader failed to join"))??;
    let stderr = stderr_reader
        .join()
        .map_err(|_| io::Error::other("stderr reader failed to join"))??;
    Ok(CommandOutput {
        status,
        stdout: stdout.bytes,
        _stderr: stderr.bytes,
        stdout_truncated: stdout.truncated,
        stderr_truncated: stderr.truncated,
        timed_out,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use std::time::Duration;

    const FIXTURE_MODE: &str = "OC_P019_PROCESS_FIXTURE";

    /// Invoked in a child process by the subprocess tests below.
    #[test]
    fn process_fixture_child() {
        match std::env::var(FIXTURE_MODE).as_deref() {
            Ok("flood") => {
                let mut out = std::io::stdout().lock();
                for _ in 0..256 {
                    out.write_all(&[b'x'; 4096]).unwrap();
                }
                std::process::exit(0);
            }
            Ok("hang") => loop {
                thread::sleep(Duration::from_secs(1));
            },
            _ => {}
        }
    }

    fn test_exe() -> std::path::PathBuf {
        std::env::current_exe().unwrap()
    }

    fn fixture_command(mode: &str) -> Command {
        let mut command = Command::new(test_exe());
        command
            .args([
                "--exact",
                "airdrop::process::tests::process_fixture_child",
                "--nocapture",
            ])
            .env(FIXTURE_MODE, mode);
        command
    }

    #[test]
    fn drains_output_after_bound_without_backpressure_or_memory_growth() {
        // Reuse the production collector with a child fixture while preserving
        // the same dual-pipe reader logic as a real Owly doctor invocation.
        let mut child = fixture_command("flood")
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap();
        let output = collect_child(&mut child, Duration::from_secs(2), 128).unwrap();
        assert!(output.status.success());
        assert_eq!(output.stdout.len(), 128);
        assert!(output.stdout_truncated);
        assert_eq!(output._stderr.len(), 0);
        assert!(!output.timed_out);
    }

    #[test]
    fn deadline_kills_a_hung_child_and_joins_pipe_drainers() {
        let mut child = fixture_command("hang")
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap();
        let output = collect_child(&mut child, Duration::from_millis(50), 128).unwrap();
        assert!(output.timed_out);
        assert!(!output.status.success());
    }

    #[test]
    fn rejects_relative_executable_paths() {
        assert!(
            run_bounded(
                Path::new("owly"),
                std::iter::empty::<&OsStr>(),
                Duration::from_secs(1),
                10
            )
            .is_err()
        );
    }

    #[test]
    fn terminal_relay_caps_shared_output_and_continues_draining() {
        use std::sync::atomic::{AtomicBool, AtomicUsize};
        let bytes = AtomicUsize::new(0);
        let exceeded = AtomicBool::new(false);
        let failed = AtomicBool::new(false);
        let mut output = Vec::new();
        relay_bounded(&[b'x'; 32][..], &mut output, &bytes, &exceeded, &failed, 12).unwrap();
        assert_eq!(output.len(), 12);
        assert_eq!(bytes.load(std::sync::atomic::Ordering::Relaxed), 12);
        assert!(exceeded.load(std::sync::atomic::Ordering::Relaxed));
        assert!(!failed.load(std::sync::atomic::Ordering::Relaxed));
    }
}
