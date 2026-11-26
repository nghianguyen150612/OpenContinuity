//! Bounded subprocess helpers for read-only probes of a trusted system tool.

use std::ffi::OsStr;
use std::fs::{File, OpenOptions};
use std::io::{self, Read};
use std::os::fd::AsRawFd;
use std::os::unix::fs::{MetadataExt, OpenOptionsExt};
use std::path::Path;
use std::process::{Child, Command, ExitStatus, Stdio};
use std::thread;
use std::time::{Duration, Instant};

#[derive(Debug)]
pub struct CommandOutput {
    pub status: ExitStatus,
    pub stdout: Vec<u8>,
    _stderr: Vec<u8>,
    pub stdout_truncated: bool,
    pub stderr_truncated: bool,
    pub timed_out: bool,
}

// System administrators, the loader, and system libraries remain trusted.
// Holding the checked ELF open prevents pathname replacement from selecting a
// different inode at exec. This is not containment of a malicious executable.
fn checked_executable(path: &Path) -> io::Result<File> {
    let denied = || io::Error::new(io::ErrorKind::PermissionDenied, "untrusted diagnostic tool");
    // Resolve distro directory aliases, then require every resolved ancestor
    // (including /) to be a root-owned directory without non-root write bits.
    for parent in path.ancestors().skip(1) {
        let resolved = std::fs::canonicalize(parent)?;
        for ancestor in resolved.ancestors() {
            let m = std::fs::symlink_metadata(ancestor)?;
            if !m.is_dir() || m.uid() != 0 || m.mode() & 0o022 != 0 {
                return Err(denied());
            }
        }
    }
    let mut file = OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_NOFOLLOW)
        .open(path)?;
    let m = file.metadata()?;
    if !super::trusted_executable_metadata(m.uid(), m.mode(), m.is_file(), false) {
        return Err(denied());
    }
    let caps = unsafe {
        libc::fgetxattr(
            file.as_raw_fd(),
            c"security.capability".as_ptr(),
            std::ptr::null_mut(),
            0,
        )
    };
    if caps >= 0 || io::Error::last_os_error().raw_os_error() != Some(libc::ENODATA) {
        return Err(denied());
    }
    let mut magic = [0; 4];
    file.read_exact(&mut magic)?;
    if magic != *b"\x7fELF" {
        return Err(denied());
    }
    Ok(file)
}

/// No shell, inherited loader variables, or caller-selected PATH. The checked
/// descriptor stays open until spawn has executed the ELF through procfs.
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
    let file = checked_executable(executable)?;
    let mut child = Command::new(format!("/proc/self/fd/{}", file.as_raw_fd()))
        .args(arguments)
        .env_clear()
        .env("PATH", "/usr/bin:/usr/sbin")
        .env("LC_ALL", "C")
        .current_dir("/")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()?;
    collect_child(&mut child, timeout, output_limit_per_stream)
}

fn nonblocking(fd: i32) -> io::Result<()> {
    let flags = unsafe { libc::fcntl(fd, libc::F_GETFL) };
    if flags < 0 || unsafe { libc::fcntl(fd, libc::F_SETFL, flags | libc::O_NONBLOCK) } < 0 {
        return Err(io::Error::last_os_error());
    }
    Ok(())
}

fn read_available(
    reader: &mut impl Read,
    bytes: &mut Vec<u8>,
    limit: usize,
    truncated: &mut bool,
) -> io::Result<bool> {
    let mut scratch = [0; 4096];
    // Bound work per tick as well as retained memory so an endless writer
    // cannot prevent the deadline from being checked.
    for _ in 0..16 {
        match reader.read(&mut scratch) {
            Ok(0) => return Ok(true),
            Ok(n) => {
                let room = limit.saturating_sub(bytes.len());
                bytes.extend_from_slice(&scratch[..n.min(room)]);
                *truncated |= n > room;
            }
            Err(e) if e.kind() == io::ErrorKind::WouldBlock => break,
            Err(e) if e.kind() == io::ErrorKind::Interrupted => continue,
            Err(e) => return Err(e),
        }
    }
    Ok(false)
}

fn collect_child(child: &mut Child, timeout: Duration, limit: usize) -> io::Result<CommandOutput> {
    let result = collect_pipes(child, timeout, limit);
    if result.is_err() {
        let _ = child.kill();
        let _ = child.try_wait();
    }
    result
}

fn collect_pipes(child: &mut Child, timeout: Duration, limit: usize) -> io::Result<CommandOutput> {
    let mut out = child
        .stdout
        .take()
        .ok_or_else(|| io::Error::other("stdout unavailable"))?;
    let mut err = child
        .stderr
        .take()
        .ok_or_else(|| io::Error::other("stderr unavailable"))?;
    nonblocking(out.as_raw_fd())?;
    nonblocking(err.as_raw_fd())?;
    let deadline = Instant::now() + timeout;
    let mut status = None;
    let (mut stdout, mut stderr) = (Vec::new(), Vec::new());
    let (mut stdout_truncated, mut stderr_truncated) = (false, false);
    loop {
        let out_done = read_available(&mut out, &mut stdout, limit, &mut stdout_truncated)?;
        let err_done = read_available(&mut err, &mut stderr, limit, &mut stderr_truncated)?;
        if status.is_none() {
            status = child.try_wait()?;
        }
        if out_done && err_done {
            if let Some(status) = status {
                return Ok(CommandOutput {
                    status,
                    stdout,
                    _stderr: stderr,
                    stdout_truncated,
                    stderr_truncated,
                    timed_out: false,
                });
            }
        }
        if Instant::now() >= deadline {
            // Close both pipes on return, including when a descendant holds
            // them open. No detached reader threads survive this call.
            let _ = child.kill();
            let stop = Instant::now() + Duration::from_secs(2);
            while status.is_none() && Instant::now() < stop {
                status = child.try_wait()?;
                thread::sleep(Duration::from_millis(10));
            }
            return match status {
                Some(status) => Ok(CommandOutput {
                    status,
                    stdout,
                    _stderr: stderr,
                    stdout_truncated,
                    stderr_truncated,
                    timed_out: true,
                }),
                None => Err(io::Error::new(
                    io::ErrorKind::TimedOut,
                    "child termination unknown",
                )),
            };
        }
        thread::sleep(Duration::from_millis(10));
    }
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
            Ok("doctor") => {
                print!("{}", std::env::var("OC_P019_DOCTOR_JSON").unwrap());
                std::process::exit(
                    std::env::var("OC_P019_DOCTOR_EXIT")
                        .unwrap()
                        .parse()
                        .unwrap(),
                );
            }
            Ok("descendant") => {
                fixture_command("hold").spawn().unwrap();
                std::process::exit(0);
            }
            Ok("hold") => {
                thread::sleep(Duration::from_secs(1));
                std::process::exit(0);
            }
            Ok("nonzero") => std::process::exit(7),
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
    fn doctor_process_contract_uses_production_parser() {
        let valid = serde_json::json!({
            "station": "wlan0", "driver": "mt7925e", "device": null,
            "status": "tested", "mode": "shared", "channel": 44,
            "card": {"phy":"phy0", "ibss":true, "ap":true,
                "active_monitor":false, "monitor_beside_station":true,
                "ibss_beside_station":true, "self_managed_reg":false,
                "regdom":"US", "channels":[{"channel":6,"freq":2437,"transmit":true}]},
            "hardware": true, "missing": [], "issues": []
        });
        let mut cases = vec![(valid.clone(), 0, Some(true)), (valid.clone(), 7, None)];
        for key in [
            "station", "driver", "device", "status", "mode", "channel", "card", "hardware",
            "missing", "issues",
        ] {
            let mut bad = valid.clone();
            bad.as_object_mut().unwrap().remove(key);
            cases.push((bad, 0, None));
        }
        for (key, bad_value) in [
            ("status", serde_json::json!("future")),
            ("mode", serde_json::json!("future")),
            (
                "missing",
                serde_json::json!([{"id":"unexpected","say":"x"}]),
            ),
            ("issues", serde_json::json!([1])),
            ("hardware", serde_json::json!("true")),
            ("card", serde_json::json!({})),
        ] {
            let mut bad = valid.clone();
            bad[key] = bad_value;
            cases.push((bad, 0, None));
        }
        let mut unprepared = valid.clone();
        unprepared["missing"] = serde_json::json!([{"id":"radio","say":"not prepared"}]);
        cases.push((unprepared, 0, Some(false)));
        let mut untested = valid.clone();
        untested["status"] = serde_json::json!("untested");
        cases.push((untested, 0, Some(false)));
        for (json, code, expected) in cases {
            let mut output = run_bounded(
                Path::new("/usr/bin/dash"),
                [
                    "-c",
                    r#"printf '%s' "$1"; exit "$2""#,
                    "fixture",
                    &json.to_string(),
                    &code.to_string(),
                ],
                Duration::from_secs(2),
                65536,
            )
            .unwrap();
            assert_eq!(
                crate::airdrop::doctor_ready(&output).ok(),
                expected,
                "{json}"
            );
            for flag in 0..3 {
                output.timed_out = flag == 0;
                output.stdout_truncated = flag == 1;
                output.stderr_truncated = flag == 2;
                assert!(crate::airdrop::doctor_ready(&output).is_err());
            }
            output.timed_out = false;
            output.stdout_truncated = false;
            output.stderr_truncated = false;
            output.stdout = b"{bad json".to_vec();
            assert!(crate::airdrop::doctor_ready(&output).is_err());
        }
        let mut output = run_bounded(
            Path::new("/usr/bin/dash"),
            ["-c", "printf '%s' '{\"station\":null,\"station\":null}'"],
            Duration::from_secs(2),
            65536,
        )
        .unwrap();
        assert!(crate::airdrop::doctor_ready(&output).is_err());
        output.stdout = b"{\"station\":null,\"sta\\u0074ion\":null}".to_vec();
        assert!(crate::airdrop::doctor_ready(&output).is_err());
    }

    #[test]
    fn descendant_pipes_do_not_extend_deadline() {
        let mut child = fixture_command("descendant")
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap();
        let start = Instant::now();
        let output = collect_child(&mut child, Duration::from_millis(100), 128).unwrap();
        assert!(output.timed_out);
        assert!(start.elapsed() < Duration::from_secs(1));
    }

    #[test]
    fn caller_environment_is_not_inherited() {
        let output = run_bounded(
            Path::new("/usr/bin/dash"),
            [
                "-c",
                "test -z \"$HOME$LD_PRELOAD$DBUS_SYSTEM_BUS_ADDRESS\" && test \"$LC_ALL\" = C",
            ],
            Duration::from_secs(2),
            128,
        )
        .unwrap();
        assert!(output.status.success());
    }

    #[test]
    fn writable_parent_is_rejected() {
        assert!(checked_executable(Path::new("/tmp/nonexistent-diagnostic")).is_err());
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
}
