//! Fail-closed cgroup v2 containment foundation for the experimental receiver.
//!
#![allow(dead_code)] // Intentionally compiled and tested, but not wired into Receive startup.

//! This module is intentionally not wired into Receive startup. A deployment
//! must explicitly provide an exclusive delegated subtree, and Receive remains
//! blocked until the production supervisor adopts and validates this boundary.

use std::ffi::{CStr, CString, OsString};
use std::fs::{self, File, OpenOptions};
use std::io::{self, Read, Write};
use std::os::fd::{AsRawFd, FromRawFd};
use std::os::unix::ffi::OsStrExt;
use std::os::unix::fs::OpenOptionsExt;
use std::os::unix::process::CommandExt;
use std::path::{Component, Path, PathBuf};
use std::process::{Child, Command};
use std::time::{Duration, Instant};

const CGROUP2_SUPER_MAGIC: libc::c_long = 0x6367_7270;
const OWNER_CGROUP: &str = ".opencontinuity-receive-owner";
const ACTIVE_SESSION_CGROUP: &str = ".opencontinuity-receive-active";
const POPULATION_POLL: Duration = Duration::from_millis(20);
const MAX_TERMINATION_WAIT: Duration = Duration::from_secs(10);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ContainmentError {
    Unsupported,
    InvalidDelegation,
    PermissionDenied,
    AlreadyPopulated,
    CreateFailed,
    PlacementFailed,
    ObservationFailed,
    KillFailed,
    PopulationTimeout,
    CleanupFailed,
}

impl ContainmentError {
    pub fn code(self) -> &'static str {
        match self {
            Self::Unsupported => "unsupported",
            Self::InvalidDelegation => "invalid_delegation",
            Self::PermissionDenied => "permission_denied",
            Self::AlreadyPopulated => "already_populated",
            Self::CreateFailed => "create_failed",
            Self::PlacementFailed => "placement_failed",
            Self::ObservationFailed => "observation_failed",
            Self::KillFailed => "kill_failed",
            Self::PopulationTimeout => "population_timeout",
            Self::CleanupFailed => "cleanup_failed",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Capability {
    pub available: bool,
    pub reason: &'static str,
}

/// A validated, dedicated cgroup v2 subtree. The supervisor must remain
/// outside this subtree. It is exclusive: the subtree must be empty at open.
pub struct DelegatedSubtree {
    root: File,
    owner: Option<File>,
    owner_name: CString,
}

impl DelegatedSubtree {
    /// Read the explicit operator supplied delegation. No discovery fallback,
    /// privilege escalation, mount change, or attempt to create the root.
    pub fn from_environment() -> Result<Self, ContainmentError> {
        Self::open(&configured_root(std::env::var_os(
            "OPENCONTINUITY_AIRDROP_CGROUP_ROOT",
        ))?)
    }

    pub fn preflight() -> Capability {
        match Self::from_environment() {
            Ok(mut subtree) => match subtree.release() {
                Ok(()) => Capability {
                    available: true,
                    reason: "delegated_subtree_verified",
                },
                Err(error) => Capability {
                    available: false,
                    reason: error.code(),
                },
            },
            Err(error) => Capability {
                available: false,
                reason: error.code(),
            },
        }
    }

    /// Open an existing, exclusive delegated cgroup v2 directory. All
    /// lifecycle operations after this use the pinned directory descriptor.
    pub fn open(path: &Path) -> Result<Self, ContainmentError> {
        if !path.is_absolute() || path.components().any(|c| matches!(c, Component::ParentDir)) {
            return Err(ContainmentError::InvalidDelegation);
        }
        let root = open_dir(path).map_err(map_open_error)?;
        let metadata = root
            .metadata()
            .map_err(|_| ContainmentError::InvalidDelegation)?;
        #[cfg(target_os = "linux")]
        {
            use std::os::unix::fs::MetadataExt;
            let effective_uid = unsafe { libc::geteuid() };
            if !owner_and_mode_are_safe(metadata.uid(), metadata.mode(), effective_uid) {
                return Err(ContainmentError::PermissionDenied);
            }
            if !is_cgroup2(root.as_raw_fd()) {
                return Err(ContainmentError::InvalidDelegation);
            }
        }
        let hierarchy_root = verify_supervisor_outside(&root)?;
        verify_migration_boundary(&root, &hierarchy_root)?;
        let effective_uid = unsafe { libc::geteuid() };
        verify_writable_control(root.as_raw_fd(), "cgroup.procs", effective_uid)?;
        verify_writable_control(root.as_raw_fd(), "cgroup.kill", effective_uid)?;
        verify_readable_control(root.as_raw_fd(), "cgroup.events", effective_uid)?;
        verify_readable_control(root.as_raw_fd(), "cgroup.controllers", effective_uid)?;
        if populated(root.as_raw_fd()).map_err(|_| ContainmentError::ObservationFailed)? {
            return Err(ContainmentError::AlreadyPopulated);
        }
        if !direct_children(root.as_raw_fd())
            .map_err(|_| ContainmentError::ObservationFailed)?
            .is_empty()
        {
            return Err(ContainmentError::InvalidDelegation);
        }
        // Atomic owner marker prevents two OpenContinuity supervisors from
        // concurrently treating one otherwise-empty delegation as exclusive.
        // A stale marker after a crash fails closed and requires operator
        // inspection; it is never used to justify deleting a populated tree.
        let owner_name = CString::new(OWNER_CGROUP).expect("static cgroup name");
        let result = unsafe { libc::mkdirat(root.as_raw_fd(), owner_name.as_ptr(), 0o700) };
        if result != 0 {
            return Err(ContainmentError::InvalidDelegation);
        }
        let owner = match openat_dir(root.as_raw_fd(), &owner_name) {
            Ok(owner) => owner,
            Err(_) => {
                let _ = unlink_empty(root.as_raw_fd(), &owner_name);
                return Err(ContainmentError::CreateFailed);
            }
        };
        if !directory_is_safely_owned(&owner, effective_uid) {
            drop(owner);
            let _ = unlink_empty(root.as_raw_fd(), &owner_name);
            return Err(ContainmentError::PermissionDenied);
        }
        Ok(Self {
            root,
            owner: Some(owner),
            owner_name,
        })
    }

    /// Atomically reserve the one active session cgroup under the exclusive
    /// delegated root. The fixed name makes concurrent or stale sessions fail
    /// closed instead of creating parallel writer sets.
    pub fn create_session(&self) -> Result<CgroupSession, ContainmentError> {
        let owner_fd = self
            .owner
            .as_ref()
            .ok_or(ContainmentError::InvalidDelegation)?
            .try_clone()
            .map_err(|_| ContainmentError::CreateFailed)?;
        let root_fd = self
            .root
            .try_clone()
            .map_err(|_| ContainmentError::CreateFailed)?;
        set_cloexec(root_fd.as_raw_fd()).map_err(|_| ContainmentError::CreateFailed)?;
        set_cloexec(owner_fd.as_raw_fd()).map_err(|_| ContainmentError::CreateFailed)?;
        if self.is_populated()?
            || direct_children(self.root.as_raw_fd())
                .map_err(|_| ContainmentError::ObservationFailed)?
                .as_slice()
                != [OWNER_CGROUP]
            || !direct_children(owner_fd.as_raw_fd())
                .map_err(|_| ContainmentError::ObservationFailed)?
                .is_empty()
        {
            return Err(ContainmentError::AlreadyPopulated);
        }
        let name_c = CString::new(ACTIVE_SESSION_CGROUP).expect("static cgroup name");
        let result = unsafe { libc::mkdirat(owner_fd.as_raw_fd(), name_c.as_ptr(), 0o700) };
        if result != 0 {
            return Err(
                if io::Error::last_os_error().kind() == io::ErrorKind::AlreadyExists {
                    ContainmentError::AlreadyPopulated
                } else {
                    ContainmentError::CreateFailed
                },
            );
        }
        let directory = match openat_dir(owner_fd.as_raw_fd(), &name_c) {
            Ok(directory) => directory,
            Err(_) => {
                if unlink_empty(owner_fd.as_raw_fd(), &name_c).is_err() {
                    return Err(ContainmentError::CleanupFailed);
                }
                return Err(ContainmentError::CreateFailed);
            }
        };
        if !directory_is_safely_owned(&directory, unsafe { libc::geteuid() }) {
            drop(directory);
            if unlink_empty(owner_fd.as_raw_fd(), &name_c).is_err() {
                return Err(ContainmentError::CleanupFailed);
            }
            return Err(ContainmentError::PermissionDenied);
        }
        let procs = match openat_control(directory.as_raw_fd(), "cgroup.procs", true) {
            Ok(procs) => procs,
            Err(_) => {
                drop(directory);
                if unlink_empty(owner_fd.as_raw_fd(), &name_c).is_err() {
                    return Err(ContainmentError::CleanupFailed);
                }
                return Err(ContainmentError::PermissionDenied);
            }
        };
        if !control_is_safely_owned(&procs, unsafe { libc::geteuid() }) {
            drop(procs);
            drop(directory);
            if unlink_empty(owner_fd.as_raw_fd(), &name_c).is_err() {
                return Err(ContainmentError::CleanupFailed);
            }
            return Err(ContainmentError::PermissionDenied);
        }
        let session = CgroupSession {
            root: root_fd,
            owner: owner_fd,
            name: name_c,
            directory,
            procs,
        };
        if session.is_populated()? {
            return Err(ContainmentError::AlreadyPopulated);
        }
        Ok(session)
    }
}

impl DelegatedSubtree {
    fn owner_fd(&self) -> libc::c_int {
        self.owner
            .as_ref()
            .expect("delegation lease is live")
            .as_raw_fd()
    }

    /// Release the atomic reservation only after the complete delegated
    /// subtree is empty and the owner node has no remaining children.
    pub fn release(&mut self) -> Result<(), ContainmentError> {
        let root_children = direct_children(self.root.as_raw_fd())
            .map_err(|_| ContainmentError::ObservationFailed)?;
        if self.is_populated()? || root_children.as_slice() != [OWNER_CGROUP] {
            return Err(ContainmentError::AlreadyPopulated);
        }
        // Once the recursive root reports empty, remove any empty cgroups a
        // process created while moving within the delegated subtree.
        remove_child_tree(self.owner_fd()).map_err(|_| ContainmentError::CleanupFailed)?;
        if !direct_children(self.owner_fd())
            .map_err(|_| ContainmentError::ObservationFailed)?
            .is_empty()
        {
            return Err(ContainmentError::CleanupFailed);
        }
        drop(self.owner.take());
        let result = unsafe {
            libc::unlinkat(
                self.root.as_raw_fd(),
                self.owner_name.as_ptr(),
                libc::AT_REMOVEDIR,
            )
        };
        if result != 0 {
            return Err(ContainmentError::CleanupFailed);
        }
        Ok(())
    }
}

impl Drop for DelegatedSubtree {
    fn drop(&mut self) {
        // No implicit kill or deletion. Forgetting to release deliberately
        // leaves the owner cgroup as a fail-closed stale reservation.
    }
}

/// One child cgroup. Membership is observed at the delegated root, not merely
/// this session node, because processes can migrate within the delegated tree.
pub struct CgroupSession {
    root: File,
    owner: File,
    name: CString,
    directory: File,
    procs: File,
}

impl CgroupSession {
    /// Install a pre-exec hook that moves the forked child into the session
    /// before exec. If writing "0" fails, spawn returns an error and the
    /// backend executable is never entered.
    fn place_before_exec(&self, command: &mut Command) {
        let fd = self.procs.as_raw_fd();
        // SAFETY: this child-only hook uses write(2) on a pinned cgroup.procs
        // descriptor. The descriptor remains open until exec; no shell is used.
        unsafe {
            command.pre_exec(move || write_current_to_cgroup(fd));
        }
    }

    pub fn spawn(&self, mut command: Command) -> io::Result<Child> {
        self.place_before_exec(&mut command);
        command.spawn()
    }

    pub fn session_name(&self) -> &CStr {
        &self.name
    }

    /// Recursive cgroup.events population at the exclusive delegated root.
    pub fn is_populated(&self) -> Result<bool, ContainmentError> {
        populated(self.root.as_raw_fd()).map_err(|_| ContainmentError::ObservationFailed)
    }

    /// cgroup.kill is issued on the whole delegated subtree. This is required
    /// because a descendant can migrate from the session node to another
    /// location inside that subtree.
    pub fn kill_subtree(&self) -> Result<(), ContainmentError> {
        let kill = openat_control(self.root.as_raw_fd(), "cgroup.kill", true)
            .map_err(|_| ContainmentError::KillFailed)?;
        let mut kill = kill;
        kill.write_all(b"1")
            .map_err(|_| ContainmentError::KillFailed)
    }

    /// Wait for recursive emptiness with a fixed deadline. Empty means the
    /// kernel reports populated=0 for the entire dedicated delegated subtree.
    pub fn wait_empty(&self, timeout: Duration) -> Result<(), ContainmentError> {
        let deadline = Instant::now() + timeout.min(MAX_TERMINATION_WAIT);
        loop {
            if !self.is_populated()? {
                return Ok(());
            }
            if Instant::now() >= deadline {
                return Err(ContainmentError::PopulationTimeout);
            }
            std::thread::sleep(POPULATION_POLL);
        }
    }

    /// Kill the whole owned subtree and wait boundedly for kernel-confirmed
    /// emptiness. This never signals a PID/PGID.
    pub fn terminate(&self, timeout: Duration) -> Result<(), ContainmentError> {
        self.kill_subtree()?;
        self.wait_empty(timeout)
    }

    /// Remove the session cgroup only after recursive population is zero.
    /// Nested empty cgroups are removed using fd-relative operations.
    pub fn cleanup(self) -> Result<(), ContainmentError> {
        self.wait_empty(Duration::ZERO)?;
        remove_child_tree(self.directory.as_raw_fd())
            .map_err(|_| ContainmentError::CleanupFailed)?;
        drop(self.procs);
        drop(self.directory);
        let result = unsafe {
            libc::unlinkat(
                self.owner.as_raw_fd(),
                self.name.as_ptr(),
                libc::AT_REMOVEDIR,
            )
        };
        if result != 0 {
            return Err(ContainmentError::CleanupFailed);
        }
        Ok(())
    }
}

fn set_cloexec(fd: libc::c_int) -> io::Result<()> {
    let flags = unsafe { libc::fcntl(fd, libc::F_GETFD) };
    if flags < 0 {
        return Err(io::Error::last_os_error());
    }
    if unsafe { libc::fcntl(fd, libc::F_SETFD, flags | libc::FD_CLOEXEC) } < 0 {
        return Err(io::Error::last_os_error());
    }
    Ok(())
}

fn write_current_to_cgroup(fd: libc::c_int) -> io::Result<()> {
    let result = unsafe { libc::write(fd, b"0".as_ptr().cast(), 1) };
    if result == 1 {
        Ok(())
    } else if result < 0 {
        Err(io::Error::last_os_error())
    } else {
        Err(io::Error::other("short cgroup placement write"))
    }
}

fn open_dir(path: &Path) -> io::Result<File> {
    if !path.is_absolute() || path.components().any(|c| matches!(c, Component::ParentDir)) {
        return Err(io::Error::from(io::ErrorKind::InvalidInput));
    }
    let mut directory = OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_DIRECTORY | libc::O_CLOEXEC | libc::O_NOFOLLOW)
        .open("/")?;
    for component in path.components() {
        match component {
            Component::RootDir | Component::CurDir => {}
            Component::Normal(name) => {
                let name = CString::new(name.as_bytes())
                    .map_err(|_| io::Error::from(io::ErrorKind::InvalidInput))?;
                directory = openat_dir(directory.as_raw_fd(), &name)?;
            }
            Component::ParentDir | Component::Prefix(_) => {
                return Err(io::Error::from(io::ErrorKind::InvalidInput));
            }
        }
    }
    Ok(directory)
}

fn openat_dir(parent: libc::c_int, name: &CStr) -> io::Result<File> {
    let fd = unsafe {
        libc::openat(
            parent,
            name.as_ptr(),
            libc::O_RDONLY | libc::O_DIRECTORY | libc::O_CLOEXEC | libc::O_NOFOLLOW,
        )
    };
    if fd < 0 {
        Err(io::Error::last_os_error())
    } else {
        Ok(unsafe { File::from_raw_fd(fd) })
    }
}

fn unlink_empty(parent: libc::c_int, name: &CStr) -> io::Result<()> {
    let result = unsafe { libc::unlinkat(parent, name.as_ptr(), libc::AT_REMOVEDIR) };
    if result == 0 {
        Ok(())
    } else {
        Err(io::Error::last_os_error())
    }
}

fn openat_control(parent: libc::c_int, name: &str, write: bool) -> io::Result<File> {
    let name = CString::new(name).expect("static control name");
    let flags = if write {
        libc::O_WRONLY
    } else {
        libc::O_RDONLY
    };
    let fd = unsafe {
        libc::openat(
            parent,
            name.as_ptr(),
            flags | libc::O_CLOEXEC | libc::O_NOFOLLOW | libc::O_NONBLOCK,
        )
    };
    if fd < 0 {
        Err(io::Error::last_os_error())
    } else {
        Ok(unsafe { File::from_raw_fd(fd) })
    }
}

fn verify_writable_control(
    parent: libc::c_int,
    name: &str,
    effective_uid: u32,
) -> Result<(), ContainmentError> {
    let file = openat_control(parent, name, true).map_err(|e| {
        if e.kind() == io::ErrorKind::PermissionDenied {
            ContainmentError::PermissionDenied
        } else {
            ContainmentError::Unsupported
        }
    })?;
    if !control_is_safely_owned(&file, effective_uid) {
        return Err(ContainmentError::PermissionDenied);
    }
    Ok(())
}

fn verify_readable_control(
    parent: libc::c_int,
    name: &str,
    effective_uid: u32,
) -> Result<(), ContainmentError> {
    let file = openat_control(parent, name, false).map_err(|_| ContainmentError::Unsupported)?;
    if !control_is_safely_owned(&file, effective_uid) {
        return Err(ContainmentError::PermissionDenied);
    }
    Ok(())
}

fn verify_migration_boundary(root: &File, hierarchy_root: &File) -> Result<(), ContainmentError> {
    let invalid = |_| ContainmentError::InvalidDelegation;
    let dotdot = CString::new("..").expect("static path component");
    let mut cursor = root.try_clone().map_err(invalid)?;
    for _ in 0..1024 {
        if same_directory(&cursor, hierarchy_root).map_err(invalid)? {
            return Err(ContainmentError::InvalidDelegation);
        }
        let parent = openat_dir(cursor.as_raw_fd(), &dotdot).map_err(invalid)?;
        if !is_cgroup2(parent.as_raw_fd()) || same_directory(&cursor, &parent).map_err(invalid)? {
            return Err(ContainmentError::InvalidDelegation);
        }
        verify_parent_control_is_inaccessible(openat_control(
            parent.as_raw_fd(),
            "cgroup.procs",
            true,
        ))?;
        if same_directory(&parent, hierarchy_root).map_err(invalid)? {
            return Ok(());
        }
        cursor = parent;
    }
    Err(ContainmentError::InvalidDelegation)
}

fn owner_and_mode_are_safe(owner: u32, mode: u32, effective_uid: u32) -> bool {
    owner == effective_uid && mode & 0o022 == 0
}

fn directory_is_safely_owned(directory: &File, effective_uid: u32) -> bool {
    use std::os::unix::fs::MetadataExt;
    directory.metadata().is_ok_and(|metadata| {
        owner_and_mode_are_safe(metadata.uid(), metadata.mode(), effective_uid)
    })
}

fn control_is_safely_owned(control: &File, effective_uid: u32) -> bool {
    use std::os::unix::fs::MetadataExt;
    control.metadata().is_ok_and(|metadata| {
        owner_and_mode_are_safe(metadata.uid(), metadata.mode(), effective_uid)
    })
}

fn verify_parent_control_is_inaccessible(result: io::Result<File>) -> Result<(), ContainmentError> {
    match result {
        Err(error) if error.kind() == io::ErrorKind::PermissionDenied => Ok(()),
        Ok(_) | Err(_) => Err(ContainmentError::InvalidDelegation),
    }
}

fn populated(root: libc::c_int) -> io::Result<bool> {
    let mut file = openat_control(root, "cgroup.events", false)?;
    let mut contents = String::new();
    file.read_to_string(&mut contents)?;
    parse_population(&contents)
}

fn direct_children(root: libc::c_int) -> io::Result<Vec<String>> {
    let root_path = PathBuf::from(format!("/proc/self/fd/{root}"));
    let mut children = Vec::new();
    for entry in fs::read_dir(root_path)? {
        let entry = entry?;
        if entry.file_type()?.is_dir() {
            children.push(entry.file_name().to_string_lossy().into_owned());
        }
    }
    Ok(children)
}

fn remove_child_tree(directory: libc::c_int) -> io::Result<()> {
    let scan = PathBuf::from(format!("/proc/self/fd/{directory}"));
    let children = fs::read_dir(scan)?
        .map(|entry| entry.map(|e| (e.file_name(), e.file_type())))
        .collect::<io::Result<Vec<_>>>()?;
    for (name, file_type) in children {
        if !file_type?.is_dir() {
            continue;
        }
        let name_c = CString::new(name.as_bytes()).map_err(io::Error::other)?;
        let child = openat_dir(directory, &name_c)?;
        if populated(child.as_raw_fd())? {
            return Err(io::Error::other("nested cgroup repopulated during cleanup"));
        }
        remove_child_tree(child.as_raw_fd())?;
        drop(child);
        let result = unsafe { libc::unlinkat(directory, name_c.as_ptr(), libc::AT_REMOVEDIR) };
        if result != 0 {
            return Err(io::Error::last_os_error());
        }
    }
    Ok(())
}

#[cfg(target_os = "linux")]
fn is_cgroup2(fd: libc::c_int) -> bool {
    // SAFETY: statfs writes to a correctly sized initialized structure.
    let mut stat: libc::statfs = unsafe { std::mem::zeroed() };
    unsafe {
        libc::fstatfs(fd, &mut stat) == 0 && stat.f_type as libc::c_long == CGROUP2_SUPER_MAGIC
    }
}

#[cfg(not(target_os = "linux"))]
fn is_cgroup2(_: libc::c_int) -> bool {
    false
}

fn configured_root(value: Option<OsString>) -> Result<PathBuf, ContainmentError> {
    value
        .map(PathBuf::from)
        .ok_or(ContainmentError::Unsupported)
}

fn verify_supervisor_outside(delegation: &File) -> Result<File, ContainmentError> {
    let cgroup_membership =
        fs::read_to_string("/proc/self/cgroup").map_err(|_| ContainmentError::Unsupported)?;
    let current = cgroup_membership
        .lines()
        .find_map(|line| line.strip_prefix("0::"))
        .ok_or(ContainmentError::Unsupported)?;
    let mountinfo =
        fs::read_to_string("/proc/self/mountinfo").map_err(|_| ContainmentError::Unsupported)?;
    let current = Path::new(current);
    for (mount_root, mount_point) in mountinfo.lines().filter_map(parse_cgroup2_mount) {
        // A bind-mounted subtree hides ancestors needed to establish the
        // migration boundary. Require a full hierarchy view.
        if mount_root != Path::new("/") || !current.starts_with(&mount_root) {
            continue;
        }
        let relative = current
            .strip_prefix(&mount_root)
            .map_err(|_| ContainmentError::InvalidDelegation)?;
        let mount_dir = open_dir(&mount_point).map_err(|_| ContainmentError::Unsupported)?;
        let current_dir = open_dir(&mount_point.join(relative))
            .map_err(|_| ContainmentError::InvalidDelegation)?;
        if is_same_or_descendant(delegation, &current_dir, &mount_dir)
            .map_err(|_| ContainmentError::Unsupported)?
        {
            return Err(ContainmentError::InvalidDelegation);
        }
        return Ok(mount_dir);
    }
    Err(ContainmentError::Unsupported)
}

fn is_same_or_descendant(delegation: &File, current: &File, mount_root: &File) -> io::Result<bool> {
    let parent_name = CString::new("..").expect("static path component");
    let mut cursor = current.try_clone()?;
    loop {
        if same_directory(delegation, &cursor)? {
            return Ok(true);
        }
        if same_directory(mount_root, &cursor)? {
            return Ok(false);
        }
        let parent = openat_dir(cursor.as_raw_fd(), &parent_name)?;
        if !is_cgroup2(parent.as_raw_fd()) || same_directory(&parent, &cursor)? {
            return Err(io::Error::other("cannot verify hierarchy ancestry"));
        }
        cursor = parent;
    }
}

fn same_directory(left: &File, right: &File) -> io::Result<bool> {
    use std::os::unix::fs::MetadataExt;
    let left = left.metadata()?;
    let right = right.metadata()?;
    Ok(left.dev() == right.dev() && left.ino() == right.ino())
}

fn parse_cgroup2_mount(line: &str) -> Option<(PathBuf, PathBuf)> {
    let fields = line.split_whitespace().collect::<Vec<_>>();
    let separator = fields.iter().position(|field| *field == "-")?;
    if fields.get(separator + 1)? != &"cgroup2" {
        return None;
    }
    Some((
        PathBuf::from(unescape_mount(fields.get(3)?)),
        PathBuf::from(unescape_mount(fields.get(4)?)),
    ))
}

fn unescape_mount(value: &str) -> String {
    value
        .replace("\\040", " ")
        .replace("\\011", "\t")
        .replace("\\012", "\n")
        .replace("\\134", "\\")
}

fn map_open_error(error: io::Error) -> ContainmentError {
    if error.kind() == io::ErrorKind::PermissionDenied {
        ContainmentError::PermissionDenied
    } else {
        ContainmentError::Unsupported
    }
}

/// Shared state machine for both kernel and fake controller tests.
pub trait Controller {
    type Session;

    fn create(&mut self) -> Result<Self::Session, ContainmentError>;
    fn populated(&mut self, session: &Self::Session) -> Result<bool, ContainmentError>;
    fn kill(&mut self, session: &Self::Session) -> Result<(), ContainmentError>;
    fn cleanup(&mut self, session: Self::Session) -> Result<(), ContainmentError>;
    fn release(&mut self) -> Result<(), ContainmentError>;
}

impl Controller for DelegatedSubtree {
    type Session = CgroupSession;

    fn create(&mut self) -> Result<Self::Session, ContainmentError> {
        self.create_session()
    }

    fn populated(&mut self, session: &Self::Session) -> Result<bool, ContainmentError> {
        session.is_populated()
    }

    fn kill(&mut self, session: &Self::Session) -> Result<(), ContainmentError> {
        session.kill_subtree()
    }

    fn cleanup(&mut self, session: Self::Session) -> Result<(), ContainmentError> {
        session.cleanup()
    }

    fn release(&mut self) -> Result<(), ContainmentError> {
        DelegatedSubtree::release(self)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SessionOutcome {
    /// Backend succeeded and the whole delegated subtree was empty before any
    /// cleanup. This is the only outcome that may pass publication eligibility.
    QuiescentSuccess,
    /// The subtree was killed/cleaned after cancellation, failure, expiry or
    /// unexpected descendants. Cleanup succeeded, but publication is forbidden.
    Stopped,
    Retained(ContainmentError),
}

pub fn writers_quiescent(outcome: SessionOutcome) -> bool {
    matches!(outcome, SessionOutcome::QuiescentSuccess)
}

/// Observe the whole delegation and stop it if execution failed or a writer
/// survived. The caller retains the session and lease through relay/reap checks.
pub fn quiesce<C: Controller>(
    controller: &mut C,
    session: &C::Session,
    normal_exit: bool,
    timeout: Duration,
) -> Result<bool, ContainmentError> {
    let populated = match controller.populated(session) {
        Ok(value) => value,
        Err(error) => {
            // A read failure does not make a potentially active tree safe.
            // Attempt a bounded stop, but retain the uncertain outcome.
            let _ = stop_tree(controller, session, timeout);
            return Err(error);
        }
    };
    if !normal_exit || populated {
        stop_tree(controller, session, timeout)?;
        return Ok(false);
    }
    Ok(true)
}

fn stop_tree<C: Controller>(
    controller: &mut C,
    session: &C::Session,
    timeout: Duration,
) -> Result<(), ContainmentError> {
    controller.kill(session)?;
    let deadline = Instant::now() + timeout.min(MAX_TERMINATION_WAIT);
    loop {
        if !controller.populated(session)? {
            return Ok(());
        }
        let remaining = deadline.saturating_duration_since(Instant::now());
        if remaining.is_zero() {
            return Err(ContainmentError::PopulationTimeout);
        }
        std::thread::sleep(POPULATION_POLL.min(remaining));
    }
}

/// Standalone settlement used by the containment foundation regressions.
pub fn settle<C: Controller>(
    controller: &mut C,
    session: C::Session,
    direct_child_succeeded: bool,
    cancelled: bool,
    expired: bool,
    timeout: Duration,
) -> SessionOutcome {
    let normal_completion = match quiesce(
        controller,
        &session,
        direct_child_succeeded && !cancelled && !expired,
        timeout,
    ) {
        Ok(normal) => normal,
        Err(error) => return SessionOutcome::Retained(error),
    };
    if let Err(error) = controller.cleanup(session) {
        return SessionOutcome::Retained(error);
    }
    if let Err(error) = controller.release() {
        return SessionOutcome::Retained(error);
    }
    if normal_completion {
        SessionOutcome::QuiescentSuccess
    } else {
        SessionOutcome::Stopped
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::VecDeque;

    #[derive(Debug, PartialEq, Eq)]
    struct FakeSession;

    #[derive(Default)]
    struct FakeController {
        create_error: Option<ContainmentError>,
        population: VecDeque<Result<bool, ContainmentError>>,
        default_populated: Option<bool>,
        kill_error: Option<ContainmentError>,
        cleanup_error: Option<ContainmentError>,
        release_error: Option<ContainmentError>,
        kills: usize,
        cleanups: usize,
        releases: usize,
    }

    impl Controller for FakeController {
        type Session = FakeSession;

        fn create(&mut self) -> Result<Self::Session, ContainmentError> {
            self.create_error.map_or(Ok(FakeSession), Err)
        }
        fn populated(&mut self, _: &Self::Session) -> Result<bool, ContainmentError> {
            self.population
                .pop_front()
                .unwrap_or(Ok(self.default_populated.unwrap_or(false)))
        }
        fn kill(&mut self, _: &Self::Session) -> Result<(), ContainmentError> {
            self.kills += 1;
            self.kill_error.map_or(Ok(()), Err)
        }
        fn cleanup(&mut self, _: Self::Session) -> Result<(), ContainmentError> {
            self.cleanups += 1;
            self.cleanup_error.map_or(Ok(()), Err)
        }
        fn release(&mut self) -> Result<(), ContainmentError> {
            self.releases += 1;
            self.release_error.map_or(Ok(()), Err)
        }
    }

    #[test]
    fn duplicated_containment_descriptors_are_close_on_exec() {
        let file = tempfile::tempfile().unwrap();
        let duplicate = file.try_clone().unwrap();
        set_cloexec(duplicate.as_raw_fd()).unwrap();
        let flags = unsafe { libc::fcntl(duplicate.as_raw_fd(), libc::F_GETFD) };
        assert!(flags >= 0 && flags & libc::FD_CLOEXEC != 0);
    }

    #[test]
    fn delegation_owner_and_parent_permissions_are_strict() {
        assert!(owner_and_mode_are_safe(1000, 0o750, 1000));
        assert!(!owner_and_mode_are_safe(0, 0o700, 1000));
        assert!(!owner_and_mode_are_safe(1000, 0o770, 1000));
        assert!(!owner_and_mode_are_safe(1000, 0o707, 1000));
        assert!(
            verify_parent_control_is_inaccessible(Err(io::Error::from(
                io::ErrorKind::PermissionDenied
            )))
            .is_ok()
        );
        assert_eq!(
            verify_parent_control_is_inaccessible(Err(io::Error::from(io::ErrorKind::NotFound))),
            Err(ContainmentError::InvalidDelegation)
        );
        let file = tempfile::tempfile().unwrap();
        assert_eq!(
            verify_parent_control_is_inaccessible(Ok(file)),
            Err(ContainmentError::InvalidDelegation)
        );
    }

    #[test]
    fn placement_hook_control_write_fails_closed() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("fake-cgroup.procs");
        fs::write(&path, b"").unwrap();
        let writable = OpenOptions::new().write(true).open(&path).unwrap();
        let root = open_dir(dir.path()).unwrap();
        let session = CgroupSession {
            root: root.try_clone().unwrap(),
            owner: root.try_clone().unwrap(),
            name: CString::new("fake-session").unwrap(),
            directory: root.try_clone().unwrap(),
            procs: writable,
        };
        let command = Command::new("/bin/true");
        let status = session.spawn(command).unwrap().wait().unwrap();
        assert!(status.success());
        assert_eq!(fs::read(&path).unwrap(), b"0");

        let readonly = OpenOptions::new().read(true).open(&path).unwrap();
        let session = CgroupSession {
            root: root.try_clone().unwrap(),
            owner: root.try_clone().unwrap(),
            name: CString::new("fake-session").unwrap(),
            directory: root,
            procs: readonly,
        };
        let command = Command::new("/bin/true");
        assert!(session.spawn(command).is_err());
    }

    #[test]
    fn missing_and_invalid_delegation_fail_closed() {
        assert_eq!(configured_root(None), Err(ContainmentError::Unsupported));
        assert!(matches!(
            DelegatedSubtree::open(Path::new("/")),
            Err(ContainmentError::PermissionDenied | ContainmentError::InvalidDelegation)
        ));
        assert!(matches!(
            DelegatedSubtree::open(Path::new("relative")),
            Err(ContainmentError::InvalidDelegation)
        ));
    }

    #[test]
    fn preflight_rejects_non_cgroup_directory_and_parent_escape_path() {
        let dir = tempfile::tempdir().unwrap();
        assert!(matches!(
            DelegatedSubtree::open(dir.path()),
            Err(ContainmentError::InvalidDelegation | ContainmentError::PermissionDenied)
        ));
        assert!(matches!(
            DelegatedSubtree::open(Path::new("/tmp/../tmp")),
            Err(ContainmentError::InvalidDelegation)
        ));
        let link = dir.path().join("cgroup-link");
        std::os::unix::fs::symlink(dir.path(), &link).unwrap();
        assert!(open_dir(&link).is_err());
    }

    #[test]
    fn publication_requires_successful_exit_and_initially_empty_subtree() {
        let mut success = FakeController::default();
        let session = success.create().unwrap();
        let outcome = settle(&mut success, session, true, false, false, Duration::ZERO);
        assert_eq!(outcome, SessionOutcome::QuiescentSuccess);
        assert!(writers_quiescent(outcome));

        for (child_ok, cancelled, expired, population) in [
            (false, false, false, Ok(false)),
            (true, true, false, Ok(false)),
            (true, false, true, Ok(false)),
            (true, false, false, Ok(true)),
        ] {
            let mut failed = FakeController {
                population: [population, Ok(false)].into(),
                ..Default::default()
            };
            let session = failed.create().unwrap();
            let outcome = settle(
                &mut failed,
                session,
                child_ok,
                cancelled,
                expired,
                Duration::from_millis(20),
            );
            assert!(!writers_quiescent(outcome));
        }
    }

    #[test]
    fn repeated_clean_sessions_release_each_session() {
        for _ in 0..3 {
            let mut fake = FakeController::default();
            let session = fake.create().unwrap();
            assert_eq!(
                settle(
                    &mut fake,
                    session,
                    true,
                    false,
                    false,
                    Duration::from_millis(10)
                ),
                SessionOutcome::QuiescentSuccess
            );
            assert_eq!(fake.cleanups, 1);
            assert_eq!(fake.kills, 0);
        }
    }

    #[test]
    fn descendant_surviving_leader_and_intra_subtree_migration_force_tree_kill() {
        // Root population remains true after leader exit, including a process
        // moved from the session cgroup into a sibling inside the delegated root.
        let mut fake = FakeController {
            population: [Ok(true), Ok(true), Ok(false)].into(),
            ..Default::default()
        };
        assert_eq!(
            settle(
                &mut fake,
                FakeSession,
                true,
                false,
                false,
                Duration::from_millis(100)
            ),
            SessionOutcome::Stopped
        );
        assert!(!writers_quiescent(SessionOutcome::Stopped));
        assert_eq!((fake.kills, fake.cleanups, fake.releases), (1, 1, 1));
    }

    #[test]
    fn cancellation_and_expiration_kill_then_wait_for_empty() {
        for (cancelled, expired) in [(true, false), (false, true)] {
            let mut fake = FakeController {
                population: [Ok(true), Ok(false)].into(),
                ..Default::default()
            };
            assert_eq!(
                settle(
                    &mut fake,
                    FakeSession,
                    false,
                    cancelled,
                    expired,
                    Duration::from_millis(100)
                ),
                SessionOutcome::Stopped
            );
            assert!(!writers_quiescent(SessionOutcome::Stopped));
            assert_eq!(fake.kills, 1);
        }
    }

    #[test]
    fn kill_failure_population_timeout_observation_and_cleanup_failure_retain() {
        let mut killed = FakeController {
            population: [Ok(true)].into(),
            kill_error: Some(ContainmentError::KillFailed),
            ..Default::default()
        };
        assert_eq!(
            settle(
                &mut killed,
                FakeSession,
                false,
                true,
                false,
                Duration::from_millis(10)
            ),
            SessionOutcome::Retained(ContainmentError::KillFailed)
        );
        assert_eq!(killed.cleanups, 0);

        let mut stuck = FakeController {
            population: [Ok(true)].into(),
            default_populated: Some(true),
            ..Default::default()
        };
        assert_eq!(
            settle(&mut stuck, FakeSession, false, false, false, Duration::ZERO),
            SessionOutcome::Retained(ContainmentError::PopulationTimeout)
        );
        assert_eq!(stuck.cleanups, 0);

        let mut unknown = FakeController {
            population: [Err(ContainmentError::ObservationFailed)].into(),
            ..Default::default()
        };
        assert_eq!(
            settle(
                &mut unknown,
                FakeSession,
                true,
                false,
                false,
                Duration::ZERO
            ),
            SessionOutcome::Retained(ContainmentError::ObservationFailed)
        );
        assert_eq!(unknown.cleanups, 0);

        let mut release = FakeController {
            release_error: Some(ContainmentError::CleanupFailed),
            ..Default::default()
        };
        assert_eq!(
            settle(
                &mut release,
                FakeSession,
                true,
                false,
                false,
                Duration::ZERO
            ),
            SessionOutcome::Retained(ContainmentError::CleanupFailed)
        );
        assert!(!writers_quiescent(SessionOutcome::Retained(
            ContainmentError::CleanupFailed
        )));

        let mut cleanup = FakeController {
            cleanup_error: Some(ContainmentError::CleanupFailed),
            ..Default::default()
        };
        assert_eq!(
            settle(
                &mut cleanup,
                FakeSession,
                true,
                false,
                false,
                Duration::ZERO
            ),
            SessionOutcome::Retained(ContainmentError::CleanupFailed)
        );
    }

    #[test]
    fn failed_create_or_child_placement_never_reports_clean() {
        let mut fake = FakeController {
            create_error: Some(ContainmentError::CreateFailed),
            ..Default::default()
        };
        assert_eq!(fake.create(), Err(ContainmentError::CreateFailed));
        let mut placement_failed = FakeController::default();
        let session = placement_failed.create().unwrap();
        // A pre-exec placement error means the backend never ran. Empty
        // membership can be cleaned safely, but it never permits publication.
        assert_eq!(
            settle(
                &mut placement_failed,
                session,
                false,
                false,
                false,
                Duration::ZERO
            ),
            SessionOutcome::Stopped
        );
        assert!(!writers_quiescent(SessionOutcome::Stopped));
    }

    /// Opt-in only. Supply an exclusive, empty delegated cgroup v2 subtree in
    /// OPENCONTINUITY_AIRDROP_CGROUP_ROOT. This test never creates/configures
    /// that root and never runs unless explicitly requested.
    #[test]
    #[ignore = "requires an explicitly delegated, exclusive cgroup v2 subtree"]
    fn real_kernel_cgroup_integration() {
        assert_eq!(
            std::env::var("OPENCONTINUITY_RUN_CGROUP_INTEGRATION").as_deref(),
            Ok("1"),
            "explicit opt-in required; also supply OPENCONTINUITY_AIRDROP_CGROUP_ROOT"
        );
        let mut subtree =
            DelegatedSubtree::from_environment().expect("valid explicit delegation required");
        let session = subtree.create_session().expect("session cgroup creation");
        let mut command = Command::new("/bin/sh");
        command.args(["-c", "exit 0"]);
        // With every ancestor write-protected, an outside same-UID
        // launcher must fail migration before backend exec.
        let placement = session.spawn(command);
        if let Ok(mut child) = placement {
            session.terminate(Duration::from_secs(2)).unwrap();
            let _ = child.try_wait();
            panic!("outside same-UID placement unexpectedly succeeded");
        }
        session.wait_empty(Duration::ZERO).unwrap();
        session.cleanup().unwrap();
        assert!(!subtree.is_populated().unwrap());
        subtree.release().unwrap();
        assert_eq!(
            placement.unwrap_err().kind(),
            io::ErrorKind::PermissionDenied
        );
    }
}

impl DelegatedSubtree {
    fn is_populated(&self) -> Result<bool, ContainmentError> {
        populated(self.root.as_raw_fd()).map_err(|_| ContainmentError::ObservationFailed)
    }
}

#[cfg(test)]
mod parsing_tests {
    use super::*;

    #[test]
    fn mount_parser_handles_cgroup2_and_escaped_mountpoints() {
        let line = "42 24 0:24 / /sys/fs/cgroup rw - cgroup2 cgroup rw";
        assert_eq!(
            parse_cgroup2_mount(line),
            Some((PathBuf::from("/"), PathBuf::from("/sys/fs/cgroup")))
        );
        assert_eq!(
            parse_cgroup2_mount("1 2 0:3 / / rw - ext4 /dev/root rw"),
            None
        );
        assert_eq!(
            unescape_mount("/run/user/1000/my\\040cg"),
            "/run/user/1000/my cg"
        );
    }

    #[test]
    fn malformed_population_never_looks_empty() {
        let cases = [
            "",
            "populated 2",
            "populated",
            "populated 0 trailing",
            "populated 0\npopulated 0",
        ];
        for value in cases {
            assert!(parse_population(value).is_err(), "{value}");
        }
        assert!(!parse_population("populated 0\nfrozen 0").unwrap());
        assert!(parse_population("populated 1").unwrap());
    }
}

fn parse_population(value: &str) -> io::Result<bool> {
    let mut result = None;
    for line in value.lines() {
        let mut fields = line.split_whitespace();
        if fields.next() == Some("populated") {
            if result.is_some() {
                return Err(io::Error::other("duplicate populated field"));
            }
            result = Some(match fields.next() {
                Some("0") if fields.next().is_none() => false,
                Some("1") if fields.next().is_none() => true,
                _ => return Err(io::Error::other("malformed populated field")),
            });
        }
    }
    result.ok_or_else(|| io::Error::other("missing populated field"))
}
