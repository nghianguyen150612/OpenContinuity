//! Explicit opt-in, bounded metadata for local P006/P009 trial lifecycle review.
use serde::Serialize;
use std::sync::{
    Mutex,
    atomic::{AtomicBool, Ordering},
};
use std::time::Instant;

const MAX_EVENTS: usize = 15; // keep room for a terminal record
const MAX_JSON_BYTES: usize = 2 * 1024;

/// Best-effort single write through a separate nonblocking file description.
///
/// Opening /proc/self/fd/N rather than dup() avoids changing O_NONBLOCK on the
/// process-wide stderr open-file description. A single <=2 KiB line is below
/// Linux PIPE_BUF, so a full pipe rejects it atomically instead of blocking or
/// emitting half a JSON record. If procfs/reopening or writing fails, diagnostics
/// are simply absent; Bluetooth cleanup and the CLI's exit must still proceed.
#[cfg(target_os = "linux")]
fn write_line_best_effort(fd: libc::c_int, line: &[u8]) -> bool {
    write_line_best_effort_from("/proc/self/fd", fd, line)
}

#[cfg(target_os = "linux")]
fn write_line_best_effort_from(proc_fd_dir: &str, fd: libc::c_int, line: &[u8]) -> bool {
    if line.len() > MAX_JSON_BYTES + 1 {
        return false;
    }
    let Ok(path) = std::ffi::CString::new(format!("{proc_fd_dir}/{fd}")) else {
        return false;
    };
    // O_APPEND keeps a regular-file stderr redirection from being overwritten
    // at offset zero when the procfs path gets a new file description.
    let handle = unsafe {
        libc::open(
            path.as_ptr(),
            libc::O_WRONLY | libc::O_NONBLOCK | libc::O_CLOEXEC | libc::O_APPEND,
        )
    };
    if handle < 0 {
        return false;
    }
    // One syscall: <= PIPE_BUF on Linux, atomic for pipes including a newline.
    let written = write_while_blocking_sigpipe(handle, line);
    let _ = unsafe { libc::close(handle) };
    written == line.len() as isize
}

/// Prevent a broken stderr pipe from delivering the default terminating
/// SIGPIPE to this process. The signal mask is thread-local and restored before
/// returning; no process-wide signal disposition is changed.
#[cfg(target_os = "linux")]
fn write_while_blocking_sigpipe(fd: libc::c_int, line: &[u8]) -> libc::ssize_t {
    let mut sigpipe_set = unsafe { std::mem::zeroed::<libc::sigset_t>() };
    if unsafe { libc::sigemptyset(&mut sigpipe_set) } != 0
        || unsafe { libc::sigaddset(&mut sigpipe_set, libc::SIGPIPE) } != 0
    {
        return -1;
    }

    let mut previous_mask = unsafe { std::mem::zeroed::<libc::sigset_t>() };
    if unsafe { libc::pthread_sigmask(libc::SIG_BLOCK, &sigpipe_set, &mut previous_mask) } != 0 {
        return -1;
    }

    let mut pending_before = unsafe { std::mem::zeroed::<libc::sigset_t>() };
    let pending_status = unsafe { libc::sigpending(&mut pending_before) };
    let was_pending = if pending_status == 0 {
        unsafe { libc::sigismember(&pending_before, libc::SIGPIPE) }
    } else {
        -1
    };
    if pending_status != 0 || was_pending < 0 {
        let _ = unsafe {
            libc::pthread_sigmask(libc::SIG_SETMASK, &previous_mask, std::ptr::null_mut())
        };
        return -1;
    }

    let written = unsafe { libc::write(fd, line.as_ptr().cast(), line.len()) };
    let broken_pipe =
        written < 0 && std::io::Error::last_os_error().raw_os_error() == Some(libc::EPIPE);
    if broken_pipe && was_pending == 0 {
        // Linux queues the synchronous SIGPIPE for this writing thread. Consume
        // only a newly generated signal before restoring its prior mask.
        let mut pending_after = unsafe { std::mem::zeroed::<libc::sigset_t>() };
        if unsafe { libc::sigpending(&mut pending_after) } == 0
            && unsafe { libc::sigismember(&pending_after, libc::SIGPIPE) } == 1
        {
            loop {
                let mut info = unsafe { std::mem::zeroed::<libc::siginfo_t>() };
                let timeout = libc::timespec {
                    tv_sec: 0,
                    tv_nsec: 0,
                };
                let received = unsafe { libc::sigtimedwait(&sigpipe_set, &mut info, &timeout) };
                if received == libc::SIGPIPE {
                    break;
                }
                if received < 0
                    && std::io::Error::last_os_error().raw_os_error() == Some(libc::EINTR)
                {
                    continue;
                }
                break;
            }
        }
    }

    let restored =
        unsafe { libc::pthread_sigmask(libc::SIG_SETMASK, &previous_mask, std::ptr::null_mut()) }
            == 0;
    if restored { written } else { -1 }
}

pub fn is_valid_trial_id(value: &str) -> bool {
    value.len() == 32
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

#[derive(Clone, Copy, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Phase {
    Start,
    AdapterReady,
    ApplicationRegistration,
    AdvertisementRegistration,
    Session,
    Frame,
    SignedRequestRead,
    SignedResponseFrame,
    Acknowledgement,
    SyntheticExchange,
    SignedExchange,
    ConsentDecision,
    Cancellation,
    Expiration,
    Disconnect,
    Failure,
    CleanupAdvertisement,
    CleanupApplication,
    Terminal,
}

#[derive(Clone, Copy, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Outcome {
    Attempted,
    Confirmed,
    Failed,
    Observed,
    Cancelled,
    Expired,
    Unknown,
    Approved,
    Rejected,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Event {
    sequence: u8,
    elapsed_ms: u32,
    phase: Phase,
    outcome: Outcome,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Report {
    schema_version: u8,
    kind: &'static str,
    source: &'static str,
    trial_id: Option<String>,
    authorization: &'static str,
    incomplete: bool,
    frame_write_count: u16,
    signed_write_count: u16,
    signed_request_frame_count: u16,
    signed_response_frame_count: u16,
    acknowledgement_count: u16,
    events: Vec<Event>,
}

pub struct Trace {
    started: Instant,
    inner: Mutex<Report>,
    missed_events: AtomicBool,
}

impl Trace {
    pub fn new() -> Self {
        Self {
            started: Instant::now(),
            inner: Mutex::new(Report {
                schema_version: 1,
                kind: "gatt_trial_diagnostics",
                source: "host_runtime_observed",
                trial_id: None,
                authorization: "never_evaluated",
                incomplete: false,
                frame_write_count: 0,
                signed_write_count: 0,
                signed_request_frame_count: 0,
                signed_response_frame_count: 0,
                acknowledgement_count: 0,
                events: Vec::with_capacity(MAX_EVENTS + 1),
            }),
            missed_events: AtomicBool::new(false),
        }
    }

    pub fn set_trial_id(&self, bytes: &[u8]) {
        if bytes.len() != 16 {
            self.missed_events.store(true, Ordering::Relaxed);
            return;
        }
        let mut value = String::with_capacity(32);
        for byte in bytes {
            use std::fmt::Write as _;
            let _ = write!(&mut value, "{byte:02x}");
        }
        if let Ok(mut report) = self.inner.try_lock() {
            report.trial_id = Some(value);
        } else {
            self.missed_events.store(true, Ordering::Relaxed);
        }
    }

    /// In-memory fixed metadata only. Never performs I/O from a GATT callback.
    pub fn record(&self, phase: Phase, mut outcome: Outcome) {
        let Ok(mut report) = self.inner.try_lock() else {
            self.missed_events.store(true, Ordering::Relaxed);
            return;
        };
        match phase {
            Phase::Frame => {
                if report.frame_write_count == u16::MAX {
                    report.incomplete = true;
                    return;
                }
                report.frame_write_count = report.frame_write_count.saturating_add(1);
                if report.frame_write_count > 1 {
                    return;
                }
            }
            Phase::SignedExchange => {
                if report.signed_write_count == u16::MAX {
                    report.incomplete = true;
                    return;
                }
                report.signed_write_count = report.signed_write_count.saturating_add(1);
                if report.signed_write_count > 1 {
                    return;
                }
            }
            Phase::SignedRequestRead => {
                if matches!(outcome, Outcome::Observed) {
                    if report.signed_request_frame_count == u16::MAX {
                        report.incomplete = true;
                        return;
                    }
                    report.signed_request_frame_count =
                        report.signed_request_frame_count.saturating_add(1);
                    return;
                }
                outcome = Outcome::Failed;
            }
            Phase::SignedResponseFrame => {
                if report.signed_response_frame_count == u16::MAX {
                    report.incomplete = true;
                    return;
                }
                report.signed_response_frame_count =
                    report.signed_response_frame_count.saturating_add(1);
                return;
            }
            Phase::Acknowledgement => {
                if report.acknowledgement_count == u16::MAX {
                    report.incomplete = true;
                    return;
                }
                report.acknowledgement_count = report.acknowledgement_count.saturating_add(1);
                return;
            }
            _ => {}
        }
        if report.events.len() >= MAX_EVENTS || matches!(phase, Phase::Terminal) {
            report.incomplete = true;
            return;
        }
        let sequence = report.events.len() as u8;
        report.events.push(Event {
            sequence,
            elapsed_ms: self.started.elapsed().as_millis().min(u32::MAX as u128) as u32,
            phase,
            outcome,
        });
    }

    pub fn terminal(&self, outcome: Outcome) {
        if let Ok(mut report) = self.inner.lock() {
            if report.events.len() == MAX_EVENTS {
                report.incomplete = true;
                report.events.pop();
                // Retained sequence numbers are re-numbered to remain contiguous.
                for (index, event) in report.events.iter_mut().enumerate() {
                    event.sequence = index as u8;
                }
            }
            let sequence = report.events.len() as u8;
            report.events.push(Event {
                sequence,
                elapsed_ms: self.started.elapsed().as_millis().min(u32::MAX as u128) as u32,
                phase: Phase::Terminal,
                outcome,
            });
        }
    }

    #[cfg(test)]
    pub fn snapshot_json(&self) -> Option<Vec<u8>> {
        serde_json::to_vec(&*self.inner.lock().ok()?).ok()
    }

    pub fn set_trial_id_hex(&self, value: &str) -> bool {
        if !is_valid_trial_id(value) {
            return false;
        }
        if let Ok(mut report) = self.inner.try_lock() {
            report.trial_id = Some(value.to_owned());
            true
        } else {
            self.missed_events.store(true, Ordering::Relaxed);
            false
        }
    }
}

impl Drop for Trace {
    fn drop(&mut self) {
        let Ok(report) = self.inner.get_mut() else {
            return;
        };
        report.incomplete |= self.missed_events.load(Ordering::Relaxed);
        if !report
            .events
            .last()
            .is_some_and(|event| matches!(event.phase, Phase::Terminal))
        {
            if report.events.len() >= MAX_EVENTS {
                report.incomplete = true;
                report.events.pop();
            }
            let outcome = if report
                .events
                .iter()
                .any(|event| matches!(event.outcome, Outcome::Failed))
            {
                Outcome::Failed
            } else {
                Outcome::Unknown
            };
            report.events.push(Event {
                sequence: report.events.len() as u8,
                elapsed_ms: self.started.elapsed().as_millis().min(u32::MAX as u128) as u32,
                phase: Phase::Terminal,
                outcome,
            });
        }
        if let Ok(mut bytes) = serde_json::to_vec(report) {
            if bytes.len() > MAX_JSON_BYTES {
                report.incomplete = true;
                let terminal = report
                    .events
                    .last()
                    .is_some_and(|event| matches!(event.phase, Phase::Terminal))
                    .then(|| report.events.pop())
                    .flatten();
                report.events.truncate(8);
                for (index, event) in report.events.iter_mut().enumerate() {
                    event.sequence = index as u8;
                }
                if let Some(mut terminal) = terminal {
                    terminal.sequence = report.events.len() as u8;
                    report.events.push(terminal);
                }
                if let Ok(compact) = serde_json::to_vec(report) {
                    bytes = compact;
                }
            }
            if bytes.len() <= MAX_JSON_BYTES {
                bytes.push(b'\n');
                #[cfg(target_os = "linux")]
                let _ = write_line_best_effort(libc::STDERR_FILENO, &bytes);
                // GATT runtime is Linux-only. Never introduce a blocking
                // diagnostic fallback on unsupported targets.
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(target_os = "linux")]
    const STDERR_CHILD_MODE: &str = "OC_P018_STDERR_CHILD_MODE";
    #[cfg(target_os = "linux")]
    const STDERR_TEST_LINE: &[u8] = b"{\"kind\":\"gatt_trial_diagnostics\"}\n";

    #[cfg(target_os = "linux")]
    fn pipe(flags: libc::c_int) -> (std::os::fd::OwnedFd, std::os::fd::OwnedFd) {
        use std::os::fd::FromRawFd;

        let mut fds = [-1; 2];
        assert_eq!(unsafe { libc::pipe2(fds.as_mut_ptr(), flags) }, 0);
        unsafe {
            (
                std::os::fd::OwnedFd::from_raw_fd(fds[0]),
                std::os::fd::OwnedFd::from_raw_fd(fds[1]),
            )
        }
    }

    #[cfg(target_os = "linux")]
    fn spawn_stderr_child(mode: &str, stderr: std::process::Stdio) -> std::process::Child {
        std::process::Command::new(std::env::current_exe().unwrap())
            .args([
                "--exact",
                "gatt_trial_diagnostics::tests::stderr_subprocess_child",
                "--nocapture",
            ])
            .env(STDERR_CHILD_MODE, mode)
            .stdout(std::process::Stdio::null())
            .stderr(stderr)
            .spawn()
            .unwrap()
    }

    #[cfg(target_os = "linux")]
    fn wait_bounded(child: &mut std::process::Child) -> std::process::ExitStatus {
        use std::time::Duration;

        let deadline = Instant::now() + Duration::from_secs(2);
        loop {
            if let Some(status) = child.try_wait().unwrap() {
                return status;
            }
            if Instant::now() >= deadline {
                let _ = child.kill();
                let _ = child.wait();
                panic!("stderr child stalled");
            }
            std::thread::sleep(Duration::from_millis(5));
        }
    }

    /// Re-executed by the subprocess tests. `process::exit` avoids test-harness
    /// writes to intentionally closed or full stderr pipes.
    #[cfg(target_os = "linux")]
    #[test]
    fn stderr_subprocess_child() {
        let Ok(mode) = std::env::var(STDERR_CHILD_MODE) else {
            return;
        };
        // Rust normally ignores SIGPIPE. Restore its default only in this
        // isolated child so the regression proves the writer handles EPIPE.
        assert_ne!(
            unsafe { libc::signal(libc::SIGPIPE, libc::SIG_DFL) },
            libc::SIG_ERR
        );
        let before = unsafe { libc::fcntl(libc::STDERR_FILENO, libc::F_GETFL) };
        let child_ok = match mode.as_str() {
            "normal" | "closed" | "full" => {
                let trace = Trace::new();
                assert!(trace.set_trial_id_hex("0123456789abcdef0123456789abcdef"));
                trace.record(Phase::Start, Outcome::Attempted);
                trace.terminal(Outcome::Confirmed);
                drop(trace);
                true
            }
            "missing_procfs" => !write_line_best_effort_from(
                "/proc/self/oc_p018_missing_fd_dir/fd",
                libc::STDERR_FILENO,
                STDERR_TEST_LINE,
            ),
            "invalid_fd" => !write_line_best_effort(-1, STDERR_TEST_LINE),
            _ => false,
        };
        let after = unsafe { libc::fcntl(libc::STDERR_FILENO, libc::F_GETFL) };
        std::process::exit(i32::from(!child_ok || before != after));
    }

    #[test]
    fn metadata_is_allowlisted_bounded_and_terminal_sequence_is_contiguous() {
        let trace = Trace::new();
        for _ in 0..100 {
            trace.record(Phase::Session, Outcome::Observed);
        }
        trace.terminal(Outcome::Confirmed);
        let report = trace.inner.lock().unwrap();
        assert!(report.incomplete);
        assert!(report.events.len() <= MAX_EVENTS);
        assert!(
            report
                .events
                .last()
                .is_some_and(|event| matches!(event.phase, Phase::Terminal))
        );
        for (index, event) in report.events.iter().enumerate() {
            assert_eq!(event.sequence as usize, index);
        }
        assert_eq!(report.authorization, "never_evaluated");
        assert!(serde_json::to_vec(&*report).unwrap().len() <= MAX_JSON_BYTES);
    }

    #[test]
    fn report_contains_no_free_form_fields() {
        let trace = Trace::new();
        trace.record(Phase::Session, Outcome::Confirmed);
        let value = serde_json::to_value(&*trace.inner.lock().unwrap()).unwrap();
        assert!(value.get("adapter").is_none());
        assert!(value.get("device").is_none());
        assert!(value.get("payload").is_none());
    }

    #[test]
    fn trial_label_override_is_lowercase_hex_and_local_metadata_only() {
        let trace = Trace::new();
        assert!(!trace.set_trial_id_hex("0123456789ABCDEF0123456789ABCDEF"));
        assert!(!trace.set_trial_id_hex("too-short"));
        assert!(trace.set_trial_id_hex("0123456789abcdef0123456789abcdef"));
        let value: serde_json::Value =
            serde_json::from_slice(&trace.snapshot_json().unwrap()).unwrap();
        assert_eq!(value["trialId"], "0123456789abcdef0123456789abcdef");
        assert_eq!(value["authorization"], "never_evaluated");
    }

    #[test]
    fn unavailable_trial_label_is_explicit_null_in_incomplete_schema() {
        let trace = Trace::new();
        trace.set_trial_id(&[]);
        trace.record(Phase::Start, Outcome::Attempted);
        let value: serde_json::Value =
            serde_json::from_slice(&trace.snapshot_json().unwrap()).unwrap();
        assert!(value["trialId"].is_null());
    }
    #[cfg(target_os = "linux")]
    #[test]
    fn subprocess_emits_exactly_one_complete_record_on_normal_stderr_pipe() {
        use std::io::Read;
        use std::process::Stdio;

        let (read_fd, write_fd) = pipe(libc::O_CLOEXEC);
        let mut child = spawn_stderr_child("normal", Stdio::from(std::fs::File::from(write_fd)));
        let mut received = Vec::new();
        std::fs::File::from(read_fd)
            .read_to_end(&mut received)
            .unwrap();
        let status = wait_bounded(&mut child);
        assert!(status.success());
        assert_eq!(received.last(), Some(&b'\n'));
        assert_eq!(received.iter().filter(|byte| **byte == b'\n').count(), 1);
        let report: serde_json::Value = serde_json::from_slice(&received).unwrap();
        assert_eq!(report["kind"], "gatt_trial_diagnostics");
        assert_eq!(report["trialId"], "0123456789abcdef0123456789abcdef");
        assert_eq!(report["authorization"], "never_evaluated");
        assert_eq!(report["events"][1]["phase"], "terminal");
        assert_eq!(report["events"][1]["outcome"], "confirmed");
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn closed_reader_pipe_does_not_sigpipe_terminate_the_subprocess() {
        use std::process::Stdio;

        let (read_fd, write_fd) = pipe(libc::O_CLOEXEC);
        drop(read_fd);
        let mut child = spawn_stderr_child("closed", Stdio::from(std::fs::File::from(write_fd)));
        let status = wait_bounded(&mut child);
        assert_eq!(status.code(), Some(0), "closed-pipe child: {status:?}");
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn full_stderr_pipe_does_not_stall_subprocess_exit_or_change_flags() {
        use std::io::Read;
        use std::os::fd::AsRawFd;
        use std::process::Stdio;

        let (read_fd, write_fd) = pipe(libc::O_CLOEXEC | libc::O_NONBLOCK);
        let fill = [b'x'; 4096];
        loop {
            let written =
                unsafe { libc::write(write_fd.as_raw_fd(), fill.as_ptr().cast(), fill.len()) };
            if written < 0 {
                assert_eq!(
                    std::io::Error::last_os_error().raw_os_error(),
                    Some(libc::EAGAIN)
                );
                break;
            }
            assert_eq!(written, fill.len() as isize);
        }

        // Keep the reader open, but do not drain capacity before child exit.
        let mut child = spawn_stderr_child("full", Stdio::from(std::fs::File::from(write_fd)));
        let status = wait_bounded(&mut child);
        assert!(status.success());
        let mut pipe_contents = Vec::new();
        std::fs::File::from(read_fd)
            .read_to_end(&mut pipe_contents)
            .unwrap();
        assert!(pipe_contents.iter().all(|byte| *byte == b'x'));
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn missing_procfs_and_failed_reopen_drop_diagnostics_in_subprocesses() {
        use std::process::Stdio;

        for mode in ["missing_procfs", "invalid_fd"] {
            let mut child = spawn_stderr_child(mode, Stdio::null());
            assert!(wait_bounded(&mut child).success(), "{mode} child failed");
        }
    }
}
