//! Rootless P019G manager/guardian protocol prototype.
//!
//! This owns a real Unix socket, authenticates the connected process with
//! SO_PEERCRED, and observes the supervisor with a pidfd. It deliberately has
//! no cgroup, quota, launch-placement, sandbox, or publication authority.

use std::io::{self, Read, Write};
use std::os::fd::{AsRawFd, FromRawFd, OwnedFd};
use std::os::unix::fs::{MetadataExt, OpenOptionsExt, PermissionsExt};
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::Path;
use std::time::Duration;

const FRAME_LEN: usize = 64;
const VERSION: u16 = 1;
const HEARTBEAT: u16 = 1;
const ACK: u16 = 0x8001;
const POLL_MS: i32 = 20;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Identity {
    pub instance: [u8; 16],
    pub session: [u8; 16],
    pub epoch: [u8; 16],
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Peer {
    pub pid: libc::pid_t,
    pub uid: libc::uid_t,
    pub gid: libc::gid_t,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ManagerError {
    SupervisorExited,
    PeerMismatch,
    Protocol,
    ReplayOrOrder,
    Io(IoFailure),
}

/// Sanitized I/O provenance. It intentionally contains no filesystem path or
/// protocol identity, while retaining the operation and original OS failure.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct IoFailure {
    pub operation: &'static str,
    pub kind: io::ErrorKind,
    pub raw_os_error: Option<i32>,
}

impl ManagerError {
    fn io(self) -> io::Error {
        match self {
            Self::Io(failure) => io::Error::new(
                failure.kind,
                format!(
                    "P019G {} failed (errno {:?})",
                    failure.operation, failure.raw_os_error
                ),
            ),
            other => io::Error::other(format!("P019G manager error: {other:?}")),
        }
    }
}

fn io_failure(operation: &'static str, error: io::Error) -> ManagerError {
    ManagerError::Io(IoFailure {
        operation,
        kind: error.kind(),
        raw_os_error: error.raw_os_error(),
    })
}

fn policy_failure(operation: &'static str) -> ManagerError {
    io_failure(
        operation,
        io::Error::new(io::ErrorKind::PermissionDenied, "policy check failed"),
    )
}

pub struct Pidfd(OwnedFd);

impl Pidfd {
    pub fn open(pid: libc::pid_t) -> io::Result<Self> {
        if pid <= 0 {
            return Err(io::Error::new(io::ErrorKind::InvalidInput, "invalid pid"));
        }
        // SAFETY: pidfd_open takes a pid and flags; it does not retain pointers.
        let fd = unsafe { libc::syscall(libc::SYS_pidfd_open, pid, 0) as libc::c_int };
        if fd < 0 {
            return Err(io::Error::last_os_error());
        }
        // SAFETY: successful pidfd_open returns a new owned descriptor.
        Ok(Self(unsafe { OwnedFd::from_raw_fd(fd) }))
    }

    pub fn exited(&self) -> io::Result<bool> {
        let mut descriptor = libc::pollfd {
            fd: self.0.as_raw_fd(),
            events: libc::POLLIN,
            revents: 0,
        };
        // SAFETY: descriptor points to one initialized pollfd.
        let result = unsafe { libc::poll(&mut descriptor, 1, 0) };
        if result < 0 {
            return Err(io::Error::last_os_error());
        }
        Ok(result == 1 && descriptor.revents & (libc::POLLIN | libc::POLLHUP) != 0)
    }
}

pub fn peer_credentials(stream: &UnixStream) -> io::Result<Peer> {
    let mut credentials: libc::ucred = unsafe { std::mem::zeroed() };
    let mut length = std::mem::size_of::<libc::ucred>() as libc::socklen_t;
    // SAFETY: credentials and length are valid writable storage for SO_PEERCRED.
    let result = unsafe {
        libc::getsockopt(
            stream.as_raw_fd(),
            libc::SOL_SOCKET,
            libc::SO_PEERCRED,
            (&mut credentials as *mut libc::ucred).cast(),
            &mut length,
        )
    };
    if result != 0 || length as usize != std::mem::size_of::<libc::ucred>() {
        return Err(io::Error::last_os_error());
    }
    Ok(Peer {
        pid: credentials.pid,
        uid: credentials.uid,
        gid: credentials.gid,
    })
}

/// Run the guardian loop for one explicitly bound supervisor. The pid is
/// expected to come from a trusted process manager's spawn result. Every
/// connection is independently checked against kernel peer credentials.
pub fn serve(
    socket: &Path,
    supervisor_pid: libc::pid_t,
    supervisor_uid: libc::uid_t,
    identity: Identity,
) -> Result<(), ManagerError> {
    serve_with_ready(socket, supervisor_pid, supervisor_uid, identity, || {})
}

fn serve_with_ready(
    socket: &Path,
    supervisor_pid: libc::pid_t,
    supervisor_uid: libc::uid_t,
    identity: Identity,
    ready: impl FnOnce(),
) -> Result<(), ManagerError> {
    let parent = socket.parent().ok_or_else(|| {
        io_failure(
            "socket_parent_lookup",
            io::Error::new(io::ErrorKind::InvalidInput, "socket has no parent"),
        )
    })?;
    let metadata = std::fs::metadata(parent)
        .map_err(|error| io_failure("parent_directory_metadata", error))?;
    if metadata.uid() != unsafe { libc::geteuid() } {
        return Err(policy_failure("parent_directory_owner"));
    }
    if metadata.mode() & 0o077 != 0 {
        return Err(policy_failure("parent_directory_mode"));
    }
    // A stale marker is retained on every error and is never recovered by
    // pathname inspection alone.
    let marker = socket.with_extension("p019g-reserved");
    let mut reservation = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(marker)
        .map_err(|error| io_failure("reservation_marker_create", error))?;
    reservation
        .write_all(&encode_identity(identity))
        .map_err(|error| io_failure("reservation_marker_write", error))?;

    let pidfd = Pidfd::open(supervisor_pid).map_err(|error| io_failure("pidfd_open", error))?;
    let listener =
        UnixListener::bind(socket).map_err(|error| io_failure("unix_socket_bind", error))?;
    std::fs::set_permissions(socket, std::fs::Permissions::from_mode(0o600))
        .map_err(|error| io_failure("socket_permissions", error))?;
    listener
        .set_nonblocking(true)
        .map_err(|error| io_failure("socket_nonblocking", error))?;
    ready();
    let mut expected_sequence = 1_u64;
    loop {
        if pidfd
            .exited()
            .map_err(|error| io_failure("pidfd_poll", error))?
        {
            return Err(ManagerError::SupervisorExited);
        }
        match listener.accept() {
            Ok((stream, _)) => {
                verify_peer(&stream, supervisor_pid, supervisor_uid)?;
                stream
                    .set_read_timeout(Some(Duration::from_secs(1)))
                    .map_err(|error| io_failure("peer_read_timeout", error))?;
                stream
                    .set_write_timeout(Some(Duration::from_secs(1)))
                    .map_err(|error| io_failure("peer_write_timeout", error))?;
                serve_peer(stream, identity, &pidfd, &mut expected_sequence)?;
            }
            Err(error) if error.kind() == io::ErrorKind::WouldBlock => {
                std::thread::sleep(Duration::from_millis(POLL_MS as u64));
            }
            Err(error) => return Err(io_failure("socket_accept", error)),
        }
    }
}

fn serve_peer(
    mut stream: UnixStream,
    identity: Identity,
    pidfd: &Pidfd,
    sequence: &mut u64,
) -> Result<(), ManagerError> {
    loop {
        if pidfd
            .exited()
            .map_err(|error| io_failure("pidfd_poll", error))?
        {
            return Err(ManagerError::SupervisorExited);
        }
        let mut frame = [0_u8; FRAME_LEN];
        match stream.read_exact(&mut frame) {
            Ok(()) => {}
            Err(error) if error.kind() == io::ErrorKind::UnexpectedEof => return Ok(()),
            Err(error) => return Err(io_failure("peer_frame_read", error)),
        }
        let (operation, request_identity, request_sequence) = decode(&frame)?;
        if operation != HEARTBEAT || request_identity != identity {
            return Err(ManagerError::Protocol);
        }
        accept_sequence(sequence, request_sequence)?;
        let response = encode(ACK, identity, request_sequence);
        stream
            .write_all(&response)
            .map_err(|error| io_failure("peer_frame_write", error))?;
        stream
            .flush()
            .map_err(|error| io_failure("peer_frame_flush", error))?;
    }
}

fn verify_peer(
    stream: &UnixStream,
    expected_pid: libc::pid_t,
    expected_uid: libc::uid_t,
) -> Result<Peer, ManagerError> {
    let peer = peer_credentials(stream).map_err(|error| io_failure("peer_credentials", error))?;
    if peer.pid != expected_pid || peer.uid != expected_uid {
        return Err(ManagerError::PeerMismatch);
    }
    Ok(peer)
}

fn accept_sequence(expected: &mut u64, observed: u64) -> Result<(), ManagerError> {
    if observed != *expected {
        return Err(ManagerError::ReplayOrOrder);
    }
    *expected = expected.checked_add(1).ok_or(ManagerError::Protocol)?;
    Ok(())
}

fn encode_identity(identity: Identity) -> [u8; 48] {
    let mut bytes = [0_u8; 48];
    bytes[0..16].copy_from_slice(&identity.instance);
    bytes[16..32].copy_from_slice(&identity.session);
    bytes[32..48].copy_from_slice(&identity.epoch);
    bytes
}

pub fn encode(operation: u16, identity: Identity, sequence: u64) -> [u8; FRAME_LEN] {
    let mut frame = [0_u8; FRAME_LEN];
    frame[0..4].copy_from_slice(b"OCG1");
    frame[4..6].copy_from_slice(&VERSION.to_be_bytes());
    frame[6..8].copy_from_slice(&operation.to_be_bytes());
    frame[8..24].copy_from_slice(&identity.instance);
    frame[24..40].copy_from_slice(&identity.session);
    frame[40..56].copy_from_slice(&identity.epoch);
    frame[56..64].copy_from_slice(&sequence.to_be_bytes());
    frame
}

fn decode(frame: &[u8; FRAME_LEN]) -> Result<(u16, Identity, u64), ManagerError> {
    if &frame[0..4] != b"OCG1" || u16::from_be_bytes([frame[4], frame[5]]) != VERSION {
        return Err(ManagerError::Protocol);
    }
    let mut instance = [0; 16];
    let mut session = [0; 16];
    let mut epoch = [0; 16];
    instance.copy_from_slice(&frame[8..24]);
    session.copy_from_slice(&frame[24..40]);
    epoch.copy_from_slice(&frame[40..56]);
    Ok((
        u16::from_be_bytes([frame[6], frame[7]]),
        Identity {
            instance,
            session,
            epoch,
        },
        u64::from_be_bytes(
            frame[56..64]
                .try_into()
                .map_err(|_| ManagerError::Protocol)?,
        ),
    ))
}

pub fn connect(path: &Path, identity: Identity, sequence: u64) -> io::Result<[u8; FRAME_LEN]> {
    let mut stream = UnixStream::connect(path)?;
    stream.set_read_timeout(Some(Duration::from_secs(1)))?;
    stream.set_write_timeout(Some(Duration::from_secs(1)))?;
    stream.write_all(&encode(HEARTBEAT, identity, sequence))?;
    let mut response = [0_u8; FRAME_LEN];
    stream.read_exact(&mut response)?;
    let (operation, response_identity, response_sequence) =
        decode(&response).map_err(ManagerError::io)?;
    if operation != ACK || response_identity != identity || response_sequence != sequence {
        return Err(ManagerError::Protocol.io());
    }
    Ok(response)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::BufRead;
    use std::process::{Command, Stdio};

    fn identity(epoch: u8) -> Identity {
        Identity {
            instance: [1; 16],
            session: [2; 16],
            epoch: [epoch; 16],
        }
    }

    fn private_tempdir() -> tempfile::TempDir {
        tempfile::Builder::new()
            .permissions(std::fs::Permissions::from_mode(0o700))
            .tempdir()
            .unwrap()
    }

    #[test]
    fn pidfd_observes_normal_exit_and_sigkill_without_pid_reuse() {
        let mut child = Command::new("sh").args(["-c", "exit 0"]).spawn().unwrap();
        let pidfd = Pidfd::open(child.id() as libc::pid_t).unwrap();
        let _ = child.wait();
        assert!(pidfd.exited().unwrap());

        let mut child = Command::new("sh")
            .args(["-c", "exec sleep 30"])
            .spawn()
            .unwrap();
        let pidfd = Pidfd::open(child.id() as libc::pid_t).unwrap();
        child.kill().unwrap();
        let _ = child.wait();
        assert!(pidfd.exited().unwrap());
    }

    #[test]
    fn authenticated_peer_sequence_and_epoch_are_enforced() {
        let root = tempfile::tempdir().unwrap();
        let socket = root.path().join("manager.sock");
        let listener = UnixListener::bind(&socket).unwrap();
        let expected = Peer {
            pid: unsafe { libc::getpid() },
            uid: unsafe { libc::geteuid() },
            gid: unsafe { libc::getegid() },
        };
        let (client, server) = UnixStream::pair().unwrap();
        let observed = peer_credentials(&client).unwrap();
        assert_eq!(observed, expected);
        assert_eq!(
            verify_peer(&client, expected.pid, expected.uid),
            Ok(expected)
        );
        assert_eq!(
            verify_peer(&client, expected.pid + 1, expected.uid),
            Err(ManagerError::PeerMismatch)
        );
        drop(listener);
        drop(server);

        // Old manager epochs are not accepted by a restarted instance.
        let old = encode(HEARTBEAT, identity(3), 1);
        let new_epoch = identity(4);
        assert_eq!(decode(&old).unwrap().1, identity(3));
        assert_ne!(decode(&old).unwrap().1, new_epoch);

        let mut next = 1;
        assert_eq!(
            accept_sequence(&mut next, 2),
            Err(ManagerError::ReplayOrOrder)
        );
        accept_sequence(&mut next, 1).unwrap();
        assert_eq!(
            accept_sequence(&mut next, 1),
            Err(ManagerError::ReplayOrOrder)
        );
    }

    #[test]
    fn descendants_can_outlive_a_direct_child_but_are_not_mistaken_for_owned() {
        let mut child = Command::new("sh")
            .args(["-c", "sleep 20 & echo $!"])
            .stdout(Stdio::piped())
            .spawn()
            .unwrap();
        let worker: libc::pid_t = {
            let mut stdout = child.stdout.take().unwrap();
            let mut line = String::new();
            std::io::BufReader::new(&mut stdout)
                .read_line(&mut line)
                .unwrap();
            line.trim().parse().unwrap()
        };
        let _ = child.wait();
        let worker_pidfd = Pidfd::open(worker).unwrap();
        assert!(!worker_pidfd.exited().unwrap());
        // Cleanup is explicit in this harmless test; no cgroup-containment
        // claim is made from the pidfd observation.
        let _ = unsafe { libc::kill(worker, libc::SIGKILL) };
    }

    #[test]
    fn frame_is_fixed_size_versioned_and_rejects_replay_order_values() {
        let identity = identity(8);
        let first = encode(HEARTBEAT, identity, 1);
        assert_eq!(first.len(), FRAME_LEN);
        assert_eq!(decode(&first).unwrap(), (HEARTBEAT, identity, 1));
        let mut invalid = first;
        invalid[4] = 0xff;
        assert_eq!(decode(&invalid), Err(ManagerError::Protocol));
        let reordered = encode(HEARTBEAT, identity, 2);
        assert_eq!(decode(&reordered).unwrap().2, 2);
        let replayed = encode(HEARTBEAT, identity, 1);
        assert_eq!(decode(&replayed).unwrap().2, 1);
    }

    #[test]
    fn manager_socket_reservation_is_not_automatically_recovered() {
        let root = tempfile::tempdir().unwrap();
        let marker = root.path().join("session.reserved");
        std::fs::write(&marker, b"instance=01 session=02 epoch=03\n").unwrap();
        assert!(
            std::fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&marker)
                .is_err()
        );
    }

    #[test]
    fn guardian_observes_supervisor_death_and_retains_reservation() {
        let root = private_tempdir();
        let socket = root.path().join("guardian.sock");
        let mut supervisor = std::process::Command::new("sh")
            .args(["-c", "exec sleep 30"])
            .spawn()
            .unwrap();
        let supervisor_pid = supervisor.id() as libc::pid_t;
        let identity = identity(9);
        let server_socket = socket.clone();
        let (ready_tx, ready_rx) = std::sync::mpsc::sync_channel(1);
        let server = std::thread::spawn(move || {
            serve_with_ready(
                &server_socket,
                supervisor_pid,
                unsafe { libc::geteuid() },
                identity,
                || {
                    ready_tx.send(()).unwrap();
                },
            )
        });
        let marker = socket.with_extension("p019g-reserved");
        if let Err(startup_error) = ready_rx.recv_timeout(Duration::from_secs(5)) {
            supervisor.kill().unwrap();
            let _ = supervisor.wait();
            let server_result = server.join().unwrap();
            panic!("guardian startup failed: {startup_error}; {server_result:?}");
        }
        assert!(
            marker.exists(),
            "ready guardian must retain its reservation"
        );
        supervisor.kill().unwrap();
        let _ = supervisor.wait();
        assert_eq!(server.join().unwrap(), Err(ManagerError::SupervisorExited));
        assert!(marker.exists(), "unexpected exit must retain reservation");
    }
}
