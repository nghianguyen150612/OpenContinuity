//! Fail-closed filesystem project-quota foundation for AirDrop Receive.
//!
//! The production Receive gate is closed before this module can be reached.
//! This code only verifies externally provisioned XFS or ext4 project quotas.

use std::ffi::{CStr, CString, OsString};
use std::fs::{self, File, OpenOptions};
use std::io;
use std::os::fd::{AsRawFd, FromRawFd};
use std::os::unix::ffi::OsStrExt;
use std::os::unix::fs::{MetadataExt, OpenOptionsExt};
use std::path::{Component, Path, PathBuf};

const XFS_SUPER_MAGIC: libc::c_long = 0x5846_5342;
const EXT4_SUPER_MAGIC: libc::c_long = 0xef53;
const FS_IOC_FSGETXATTR: libc::c_ulong = 0x801c_581f;
const FS_XFLAG_PROJINHERIT: u32 = 0x0000_0200;
const OWNER_DIRECTORY: &str = ".opencontinuity-airdrop-quota-owner";

const PRJQUOTA: libc::c_int = 2;
const XQM_PRJQUOTA: libc::c_int = 2;
const Q_XGETQUOTA: libc::c_int = 0x5803;
const Q_XGETQSTATV: libc::c_int = 0x5808;
const Q_GETQUOTA: libc::c_int = 0x800007;
const SUBCMD_SHIFT: u32 = 8;

const FS_QUOTA_PDQ_ACCT: u16 = 1 << 4;
const FS_QUOTA_PDQ_ENFD: u16 = 1 << 5;
const FS_PROJ_QUOTA: i8 = 1 << 1;
const FS_QSTATV_VERSION1: i8 = 1;
const QIF_BLIMITS: u32 = 1 << 0;
const QIF_ILIMITS: u32 = 1 << 2;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum QuotaError {
    UnsupportedFilesystem,
    InvalidConfiguration,
    PermissionDenied,
    QuotaUnavailable,
    EnforcementUnknown,
    ProjectMismatch,
    InheritanceMissing,
    HardLimitMismatch,
    OwnershipConflict,
    NoCapacity,
    ObservationFailed,
    CleanupFailed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Filesystem {
    Xfs,
    Ext4,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct ProjectAttributes {
    id: u32,
    inherits: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct QuotaSnapshot {
    pub hard_bytes: u64,
    pub used_bytes: u64,
    pub hard_inodes: u64,
    pub used_inodes: u64,
    pub(crate) enforcement: bool,
}

impl QuotaSnapshot {
    fn exhausted(self) -> bool {
        self.used_bytes >= self.hard_bytes || self.used_inodes >= self.hard_inodes
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct QuotaConfig {
    project_id: u32,
    hard_bytes: u64,
    hard_inodes: u64,
}

impl QuotaConfig {
    fn from_environment() -> Result<(PathBuf, Self), QuotaError> {
        let root = required_environment("OPENCONTINUITY_AIRDROP_QUOTA_ROOT")?;
        let project_id = parse_environment("OPENCONTINUITY_AIRDROP_QUOTA_PROJECT_ID")?;
        let hard_bytes = parse_environment("OPENCONTINUITY_AIRDROP_QUOTA_HARD_BYTES")?;
        let hard_inodes = parse_environment("OPENCONTINUITY_AIRDROP_QUOTA_HARD_INODES")?;
        let config = Self {
            project_id,
            hard_bytes,
            hard_inodes,
        };
        if config.project_id == 0 || config.hard_bytes == 0 || config.hard_inodes == 0 {
            return Err(QuotaError::InvalidConfiguration);
        }
        Ok((PathBuf::from(root), config))
    }
}

fn required_environment(name: &str) -> Result<OsString, QuotaError> {
    std::env::var_os(name).ok_or(QuotaError::InvalidConfiguration)
}

fn parse_environment<T: std::str::FromStr>(name: &str) -> Result<T, QuotaError> {
    required_environment(name)?
        .into_string()
        .map_err(|_| QuotaError::InvalidConfiguration)?
        .parse()
        .map_err(|_| QuotaError::InvalidConfiguration)
}

trait QuotaProbe {
    fn filesystem(&self, root: &File) -> Result<Filesystem, QuotaError>;
    fn attributes(&self, directory: &File) -> Result<ProjectAttributes, QuotaError>;
    fn quota(
        &self,
        root: &File,
        filesystem: Filesystem,
        project_id: u32,
    ) -> Result<QuotaSnapshot, QuotaError>;
}

struct LinuxQuotaProbe;

impl QuotaProbe for LinuxQuotaProbe {
    fn filesystem(&self, root: &File) -> Result<Filesystem, QuotaError> {
        let mut status: libc::statfs = unsafe { std::mem::zeroed() };
        if unsafe { libc::fstatfs(root.as_raw_fd(), &mut status) } != 0 {
            return Err(QuotaError::ObservationFailed);
        }
        match status.f_type as libc::c_long {
            XFS_SUPER_MAGIC => Ok(Filesystem::Xfs),
            EXT4_SUPER_MAGIC => Ok(Filesystem::Ext4),
            _ => Err(QuotaError::UnsupportedFilesystem),
        }
    }

    fn attributes(&self, directory: &File) -> Result<ProjectAttributes, QuotaError> {
        let mut attributes = Fsxattr::default();
        if unsafe {
            libc::ioctl(
                directory.as_raw_fd(),
                FS_IOC_FSGETXATTR,
                &mut attributes as *mut Fsxattr,
            )
        } != 0
        {
            return Err(map_observation_error(io::Error::last_os_error()));
        }
        Ok(ProjectAttributes {
            id: attributes.project_id,
            inherits: attributes.flags & FS_XFLAG_PROJINHERIT != 0,
        })
    }

    fn quota(
        &self,
        root: &File,
        filesystem: Filesystem,
        project_id: u32,
    ) -> Result<QuotaSnapshot, QuotaError> {
        match filesystem {
            Filesystem::Xfs => xfs_quota(root, project_id),
            Filesystem::Ext4 => ext4_quota(root, project_id),
        }
    }
}

pub trait QuotaLease {
    fn storage_root(&self) -> &Path;
    fn verify_directory(&self, directory: &Path) -> Result<(), QuotaError>;
    fn revalidate(&self) -> Result<QuotaSnapshot, QuotaError>;
    fn release(&mut self) -> Result<(), QuotaError>;
}

pub struct ProjectQuotaLease {
    root: File,
    root_path: PathBuf,
    owner: Option<File>,
    owner_name: CString,
    owner_identity: (u64, u64),
    filesystem: Filesystem,
    config: QuotaConfig,
    probe: Box<dyn QuotaProbe>,
}

impl ProjectQuotaLease {
    pub fn from_environment(destination: &Path) -> Result<Self, QuotaError> {
        let (configured_root, config) = QuotaConfig::from_environment()?;
        Self::open_with(
            destination,
            &configured_root,
            config,
            Box::new(LinuxQuotaProbe),
        )
    }

    fn open_with(
        destination: &Path,
        configured_root: &Path,
        config: QuotaConfig,
        probe: Box<dyn QuotaProbe>,
    ) -> Result<Self, QuotaError> {
        let destination =
            fs::canonicalize(destination).map_err(|_| QuotaError::InvalidConfiguration)?;
        let destination = open_absolute_directory(&destination)?;
        let root = open_absolute_directory(configured_root)?;
        if !same_file(&destination, &root).map_err(|_| QuotaError::ObservationFailed)? {
            return Err(QuotaError::InvalidConfiguration);
        }
        verify_private_root(&root)?;
        let filesystem = probe.filesystem(&root)?;
        validate_attributes(probe.attributes(&root)?, config)?;
        validate_snapshot(
            probe.quota(&root, filesystem, config.project_id)?,
            config,
            true,
        )?;

        let owner_name = CString::new(OWNER_DIRECTORY).expect("static quota owner name");
        if unsafe { libc::mkdirat(root.as_raw_fd(), owner_name.as_ptr(), 0o700) } != 0 {
            let error = io::Error::last_os_error();
            return Err(if error.kind() == io::ErrorKind::AlreadyExists {
                QuotaError::OwnershipConflict
            } else if matches!(
                error.raw_os_error(),
                Some(code) if matches!(code, libc::EDQUOT | libc::ENOSPC)
            ) {
                QuotaError::NoCapacity
            } else {
                QuotaError::PermissionDenied
            });
        }
        let owner = match openat_directory(root.as_raw_fd(), &owner_name) {
            Ok(owner) => owner,
            Err(_) => {
                let _ = unlink_directory(root.as_raw_fd(), &owner_name);
                return Err(QuotaError::ObservationFailed);
            }
        };
        if let Err(error) = validate_attributes(probe.attributes(&owner)?, config) {
            drop(owner);
            let _ = unlink_directory(root.as_raw_fd(), &owner_name);
            return Err(error);
        }
        let owner_identity = file_identity(&owner).map_err(|_| QuotaError::ObservationFailed)?;
        let root_path = PathBuf::from(format!("/proc/self/fd/{}", root.as_raw_fd()));
        Ok(Self {
            root,
            root_path,
            owner: Some(owner),
            owner_name,
            owner_identity,
            filesystem,
            config,
            probe,
        })
    }

    fn verify_owner(&self) -> Result<(), QuotaError> {
        let current = openat_directory(self.root.as_raw_fd(), &self.owner_name)
            .map_err(|_| QuotaError::OwnershipConflict)?;
        if file_identity(&current).map_err(|_| QuotaError::ObservationFailed)?
            != self.owner_identity
        {
            return Err(QuotaError::OwnershipConflict);
        }
        validate_attributes(self.probe.attributes(&current)?, self.config)
    }
}

impl QuotaLease for ProjectQuotaLease {
    fn storage_root(&self) -> &Path {
        &self.root_path
    }

    fn verify_directory(&self, directory: &Path) -> Result<(), QuotaError> {
        let directory = open_directory(directory)?;
        if !is_descendant(&directory, &self.root).map_err(|_| QuotaError::ObservationFailed)? {
            return Err(QuotaError::ProjectMismatch);
        }
        if self.probe.filesystem(&directory)? != self.filesystem {
            return Err(QuotaError::UnsupportedFilesystem);
        }
        validate_attributes(self.probe.attributes(&directory)?, self.config)
    }

    fn revalidate(&self) -> Result<QuotaSnapshot, QuotaError> {
        self.verify_owner()?;
        validate_attributes(self.probe.attributes(&self.root)?, self.config)?;
        let snapshot = self
            .probe
            .quota(&self.root, self.filesystem, self.config.project_id)?;
        validate_snapshot(snapshot, self.config, false)?;
        Ok(snapshot)
    }

    fn release(&mut self) -> Result<(), QuotaError> {
        self.revalidate()?;
        let owner = self.owner.as_ref().ok_or(QuotaError::OwnershipConflict)?;
        if fs::read_dir(format!("/proc/self/fd/{}", owner.as_raw_fd()))
            .map_err(|_| QuotaError::ObservationFailed)?
            .next()
            .is_some()
        {
            return Err(QuotaError::CleanupFailed);
        }
        drop(self.owner.take());
        unlink_directory(self.root.as_raw_fd(), &self.owner_name)
            .map_err(|_| QuotaError::CleanupFailed)
    }
}

impl Drop for ProjectQuotaLease {
    fn drop(&mut self) {
        // An unreleased owner directory is a fail-closed stale reservation.
    }
}

fn validate_attributes(
    attributes: ProjectAttributes,
    config: QuotaConfig,
) -> Result<(), QuotaError> {
    if attributes.id != config.project_id {
        return Err(QuotaError::ProjectMismatch);
    }
    if !attributes.inherits {
        return Err(QuotaError::InheritanceMissing);
    }
    Ok(())
}

fn validate_snapshot(
    snapshot: QuotaSnapshot,
    config: QuotaConfig,
    require_capacity: bool,
) -> Result<(), QuotaError> {
    if !snapshot.enforcement {
        return Err(QuotaError::EnforcementUnknown);
    }
    if snapshot.hard_bytes != config.hard_bytes
        || snapshot.hard_inodes != config.hard_inodes
        || snapshot.used_bytes > snapshot.hard_bytes
        || snapshot.used_inodes > snapshot.hard_inodes
    {
        return Err(QuotaError::HardLimitMismatch);
    }
    if require_capacity && snapshot.exhausted() {
        return Err(QuotaError::NoCapacity);
    }
    Ok(())
}

fn verify_private_root(root: &File) -> Result<(), QuotaError> {
    let metadata = root.metadata().map_err(|_| QuotaError::ObservationFailed)?;
    if metadata.uid() != unsafe { libc::geteuid() } || metadata.mode() & 0o077 != 0 {
        return Err(QuotaError::PermissionDenied);
    }
    Ok(())
}

fn open_absolute_directory(path: &Path) -> Result<File, QuotaError> {
    if !path.is_absolute()
        || path
            .components()
            .any(|part| matches!(part, Component::ParentDir))
    {
        return Err(QuotaError::InvalidConfiguration);
    }
    let mut directory = open_directory(Path::new("/"))?;
    for component in path.components() {
        match component {
            Component::RootDir | Component::CurDir => {}
            Component::Normal(name) => {
                let name =
                    CString::new(name.as_bytes()).map_err(|_| QuotaError::InvalidConfiguration)?;
                directory = openat_directory(directory.as_raw_fd(), &name)
                    .map_err(|_| QuotaError::InvalidConfiguration)?;
            }
            Component::ParentDir | Component::Prefix(_) => {
                return Err(QuotaError::InvalidConfiguration);
            }
        }
    }
    Ok(directory)
}

fn open_directory(path: &Path) -> Result<File, QuotaError> {
    OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_DIRECTORY | libc::O_CLOEXEC | libc::O_NOFOLLOW)
        .open(path)
        .map_err(map_open_error)
}

fn openat_directory(parent: libc::c_int, name: &CStr) -> io::Result<File> {
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

fn unlink_directory(parent: libc::c_int, name: &CStr) -> io::Result<()> {
    if unsafe { libc::unlinkat(parent, name.as_ptr(), libc::AT_REMOVEDIR) } == 0 {
        Ok(())
    } else {
        Err(io::Error::last_os_error())
    }
}

fn same_file(left: &File, right: &File) -> io::Result<bool> {
    Ok(file_identity(left)? == file_identity(right)?)
}

fn file_identity(file: &File) -> io::Result<(u64, u64)> {
    let metadata = file.metadata()?;
    Ok((metadata.dev(), metadata.ino()))
}

fn is_descendant(directory: &File, root: &File) -> io::Result<bool> {
    let parent_name = CString::new("..").expect("static parent name");
    let mut cursor = directory.try_clone()?;
    for _ in 0..1024 {
        if same_file(&cursor, root)? {
            return Ok(true);
        }
        let parent = openat_directory(cursor.as_raw_fd(), &parent_name)?;
        if same_file(&cursor, &parent)? {
            return Ok(false);
        }
        cursor = parent;
    }
    Err(io::Error::other("directory ancestry is too deep"))
}

fn map_open_error(error: io::Error) -> QuotaError {
    if error.kind() == io::ErrorKind::PermissionDenied {
        QuotaError::PermissionDenied
    } else {
        QuotaError::InvalidConfiguration
    }
}

fn map_observation_error(error: io::Error) -> QuotaError {
    if error.kind() == io::ErrorKind::PermissionDenied {
        QuotaError::PermissionDenied
    } else if matches!(
        error.raw_os_error(),
        Some(code) if matches!(code, libc::ENOSYS | libc::EOPNOTSUPP | libc::ESRCH)
    ) {
        QuotaError::QuotaUnavailable
    } else {
        QuotaError::ObservationFailed
    }
}

fn quota_command(command: libc::c_int, quota_type: libc::c_int) -> libc::c_int {
    (((command as u32) << SUBCMD_SHIFT) | quota_type as u32) as libc::c_int
}

fn quota_call<T>(
    root: &File,
    command: libc::c_int,
    id: u32,
    output: &mut T,
) -> Result<(), QuotaError> {
    let result = unsafe {
        libc::syscall(
            libc::SYS_quotactl_fd,
            root.as_raw_fd(),
            command,
            id as libc::c_int,
            output as *mut T,
        )
    };
    if result == 0 {
        Ok(())
    } else {
        Err(map_observation_error(io::Error::last_os_error()))
    }
}

fn xfs_quota(root: &File, project_id: u32) -> Result<QuotaSnapshot, QuotaError> {
    let mut status = QuotaStatusV {
        version: FS_QSTATV_VERSION1,
        ..Default::default()
    };
    quota_call(
        root,
        quota_command(Q_XGETQSTATV, XQM_PRJQUOTA),
        0,
        &mut status,
    )?;
    let enforcement = status.flags & (FS_QUOTA_PDQ_ACCT | FS_QUOTA_PDQ_ENFD)
        == FS_QUOTA_PDQ_ACCT | FS_QUOTA_PDQ_ENFD;
    let mut quota = XfsDiskQuota::default();
    quota_call(
        root,
        quota_command(Q_XGETQUOTA, XQM_PRJQUOTA),
        project_id,
        &mut quota,
    )?;
    if quota.id != project_id || quota.flags & FS_PROJ_QUOTA == 0 {
        return Err(QuotaError::ProjectMismatch);
    }
    Ok(QuotaSnapshot {
        hard_bytes: quota
            .block_hard_limit
            .checked_mul(512)
            .ok_or(QuotaError::HardLimitMismatch)?,
        used_bytes: quota
            .block_count
            .checked_mul(512)
            .ok_or(QuotaError::HardLimitMismatch)?,
        hard_inodes: quota.inode_hard_limit,
        used_inodes: quota.inode_count,
        enforcement,
    })
}

fn ext4_quota(root: &File, project_id: u32) -> Result<QuotaSnapshot, QuotaError> {
    let mut quota = GenericDiskQuota::default();
    quota_call(
        root,
        quota_command(Q_GETQUOTA, PRJQUOTA),
        project_id,
        &mut quota,
    )?;
    if quota.valid & (QIF_BLIMITS | QIF_ILIMITS) != QIF_BLIMITS | QIF_ILIMITS {
        return Err(QuotaError::EnforcementUnknown);
    }
    Ok(QuotaSnapshot {
        hard_bytes: quota
            .block_hard_limit
            .checked_mul(1024)
            .ok_or(QuotaError::HardLimitMismatch)?,
        used_bytes: quota.current_space,
        hard_inodes: quota.inode_hard_limit,
        used_inodes: quota.current_inodes,
        enforcement: true,
    })
}

#[repr(C)]
#[derive(Default)]
struct Fsxattr {
    flags: u32,
    extent_size: u32,
    next_extents: u32,
    project_id: u32,
    cow_extent_size: u32,
    padding: [u8; 8],
}

#[repr(C)]
#[derive(Default)]
struct QuotaFileStatusV {
    inode: u64,
    blocks: u64,
    extents: u32,
    padding: u32,
}

#[repr(C)]
#[derive(Default)]
struct QuotaStatusV {
    version: i8,
    padding1: u8,
    flags: u16,
    incore: u32,
    user: QuotaFileStatusV,
    group: QuotaFileStatusV,
    project: QuotaFileStatusV,
    block_time_limit: i32,
    inode_time_limit: i32,
    realtime_time_limit: i32,
    block_warn_limit: u16,
    inode_warn_limit: u16,
    realtime_warn_limit: u16,
    padding3: u16,
    padding4: u32,
    padding2: [u64; 7],
}

#[repr(C)]
#[derive(Default)]
struct XfsDiskQuota {
    version: i8,
    flags: i8,
    field_mask: u16,
    id: u32,
    block_hard_limit: u64,
    block_soft_limit: u64,
    inode_hard_limit: u64,
    inode_soft_limit: u64,
    block_count: u64,
    inode_count: u64,
    inode_timer: i32,
    block_timer: i32,
    inode_warnings: u16,
    block_warnings: u16,
    inode_timer_high: i8,
    block_timer_high: i8,
    realtime_timer_high: i8,
    padding2: i8,
    realtime_hard_limit: u64,
    realtime_soft_limit: u64,
    realtime_count: u64,
    realtime_timer: i32,
    realtime_warnings: u16,
    padding3: i16,
    padding4: [u8; 8],
}

#[repr(C)]
#[derive(Default)]
struct GenericDiskQuota {
    block_hard_limit: u64,
    block_soft_limit: u64,
    current_space: u64,
    inode_hard_limit: u64,
    inode_soft_limit: u64,
    current_inodes: u64,
    block_time: u64,
    inode_time: u64,
    valid: u32,
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use std::os::unix::fs::{DirBuilderExt, PermissionsExt};
    use std::sync::{Arc, Mutex};

    const CONFIG: QuotaConfig = QuotaConfig {
        project_id: 4242,
        hard_bytes: 8 * 1024 * 1024,
        hard_inodes: 128,
    };

    #[derive(Clone)]
    struct FakeProbe {
        state: Arc<Mutex<FakeState>>,
    }

    #[derive(Clone, Copy)]
    struct FakeState {
        filesystem: Result<Filesystem, QuotaError>,
        attributes: Result<ProjectAttributes, QuotaError>,
        quota: Result<QuotaSnapshot, QuotaError>,
    }

    impl Default for FakeState {
        fn default() -> Self {
            Self {
                filesystem: Ok(Filesystem::Xfs),
                attributes: Ok(ProjectAttributes {
                    id: CONFIG.project_id,
                    inherits: true,
                }),
                quota: Ok(QuotaSnapshot {
                    hard_bytes: CONFIG.hard_bytes,
                    used_bytes: 1024,
                    hard_inodes: CONFIG.hard_inodes,
                    used_inodes: 2,
                    enforcement: true,
                }),
            }
        }
    }

    impl QuotaProbe for FakeProbe {
        fn filesystem(&self, _: &File) -> Result<Filesystem, QuotaError> {
            self.state.lock().unwrap().filesystem
        }

        fn attributes(&self, _: &File) -> Result<ProjectAttributes, QuotaError> {
            self.state.lock().unwrap().attributes
        }

        fn quota(&self, _: &File, _: Filesystem, _: u32) -> Result<QuotaSnapshot, QuotaError> {
            self.state.lock().unwrap().quota
        }
    }

    fn private_root() -> tempfile::TempDir {
        let outer = tempfile::tempdir().unwrap();
        let root = outer.path().join("quota");
        let mut builder = fs::DirBuilder::new();
        builder.mode(0o700).create(&root).unwrap();
        // Keep the TempDir as the returned owner while making its path the
        // quota root expected by the helpers below.
        outer
    }

    fn root_path(outer: &tempfile::TempDir) -> PathBuf {
        outer.path().join("quota")
    }

    fn probe(state: FakeState) -> (Box<dyn QuotaProbe>, Arc<Mutex<FakeState>>) {
        let state = Arc::new(Mutex::new(state));
        (
            Box::new(FakeProbe {
                state: state.clone(),
            }),
            state,
        )
    }

    fn open_fake(root: &Path, state: FakeState) -> Result<ProjectQuotaLease, QuotaError> {
        let (probe, _) = probe(state);
        ProjectQuotaLease::open_with(root, root, CONFIG, probe)
    }

    #[test]
    fn rejects_unsupported_or_unobservable_quota_capability() {
        for filesystem in [
            Err(QuotaError::UnsupportedFilesystem),
            Err(QuotaError::QuotaUnavailable),
        ] {
            let outer = private_root();
            let root = root_path(&outer);
            let error = open_fake(
                &root,
                FakeState {
                    filesystem,
                    ..Default::default()
                },
            )
            .err()
            .unwrap();
            assert_eq!(error, filesystem.unwrap_err());
        }
    }

    #[test]
    fn rejects_project_or_inheritance_mismatch() {
        for (attributes, expected) in [
            (
                ProjectAttributes {
                    id: CONFIG.project_id + 1,
                    inherits: true,
                },
                QuotaError::ProjectMismatch,
            ),
            (
                ProjectAttributes {
                    id: CONFIG.project_id,
                    inherits: false,
                },
                QuotaError::InheritanceMissing,
            ),
        ] {
            let outer = private_root();
            let root = root_path(&outer);
            assert_eq!(
                open_fake(
                    &root,
                    FakeState {
                        attributes: Ok(attributes),
                        ..Default::default()
                    },
                )
                .err()
                .unwrap(),
                expected
            );
        }
    }

    #[test]
    fn rejects_unknown_enforcement_limits_and_exhausted_capacity() {
        let cases = [
            (
                QuotaSnapshot {
                    enforcement: false,
                    ..FakeState::default().quota.unwrap()
                },
                QuotaError::EnforcementUnknown,
            ),
            (
                QuotaSnapshot {
                    hard_bytes: CONFIG.hard_bytes + 1,
                    ..FakeState::default().quota.unwrap()
                },
                QuotaError::HardLimitMismatch,
            ),
            (
                QuotaSnapshot {
                    hard_inodes: CONFIG.hard_inodes + 1,
                    ..FakeState::default().quota.unwrap()
                },
                QuotaError::HardLimitMismatch,
            ),
            (
                QuotaSnapshot {
                    used_bytes: CONFIG.hard_bytes,
                    ..FakeState::default().quota.unwrap()
                },
                QuotaError::NoCapacity,
            ),
            (
                QuotaSnapshot {
                    used_inodes: CONFIG.hard_inodes,
                    ..FakeState::default().quota.unwrap()
                },
                QuotaError::NoCapacity,
            ),
        ];
        for (snapshot, expected) in cases {
            let outer = private_root();
            let root = root_path(&outer);
            assert_eq!(
                open_fake(
                    &root,
                    FakeState {
                        quota: Ok(snapshot),
                        ..Default::default()
                    },
                )
                .err()
                .unwrap(),
                expected
            );
        }
    }

    #[test]
    fn requires_private_exact_destination_and_exclusive_owner() {
        let outer = private_root();
        let root = root_path(&outer);
        let other = outer.path().join("other");
        fs::create_dir(&other).unwrap();
        let (probe, _) = probe(FakeState::default());
        assert_eq!(
            ProjectQuotaLease::open_with(&other, &root, CONFIG, probe)
                .err()
                .unwrap(),
            QuotaError::InvalidConfiguration
        );

        fs::set_permissions(&root, fs::Permissions::from_mode(0o750)).unwrap();
        assert_eq!(
            open_fake(&root, FakeState::default()).err().unwrap(),
            QuotaError::PermissionDenied
        );
        fs::set_permissions(&root, fs::Permissions::from_mode(0o700)).unwrap();
        fs::create_dir(root.join(OWNER_DIRECTORY)).unwrap();
        assert_eq!(
            open_fake(&root, FakeState::default()).err().unwrap(),
            QuotaError::OwnershipConflict
        );
    }

    #[test]
    fn verifies_descendants_and_releases_for_repeated_sessions() {
        let outer = private_root();
        let root = root_path(&outer);
        for _ in 0..2 {
            let mut lease = open_fake(&root, FakeState::default()).unwrap();
            let child = root.join("child");
            fs::create_dir(&child).unwrap();
            assert_eq!(lease.verify_directory(&child), Ok(()));
            assert_eq!(lease.revalidate().unwrap().used_bytes, 1024);
            lease.release().unwrap();
            fs::remove_dir(child).unwrap();
            assert!(!root.join(OWNER_DIRECTORY).exists());
        }

        let outside = tempfile::tempdir().unwrap();
        let mut lease = open_fake(&root, FakeState::default()).unwrap();
        assert_eq!(
            lease.verify_directory(outside.path()),
            Err(QuotaError::ProjectMismatch)
        );
        lease.release().unwrap();
    }

    #[test]
    fn changed_enforcement_or_owner_is_retained_fail_closed() {
        let outer = private_root();
        let root = root_path(&outer);
        let (probe, state) = probe(FakeState::default());
        let mut lease = ProjectQuotaLease::open_with(&root, &root, CONFIG, probe).unwrap();
        state.lock().unwrap().quota = Err(QuotaError::ObservationFailed);
        assert_eq!(lease.release(), Err(QuotaError::ObservationFailed));
        assert!(root.join(OWNER_DIRECTORY).exists());
        drop(lease);

        fs::remove_dir(root.join(OWNER_DIRECTORY)).unwrap();
        let mut lease = open_fake(&root, FakeState::default()).unwrap();
        fs::remove_dir(root.join(OWNER_DIRECTORY)).unwrap();
        fs::create_dir(root.join(OWNER_DIRECTORY)).unwrap();
        assert_eq!(lease.release(), Err(QuotaError::OwnershipConflict));
        assert!(root.join(OWNER_DIRECTORY).exists());
    }

    #[test]
    fn nonempty_owner_marker_prevents_release() {
        let outer = private_root();
        let root = root_path(&outer);
        let mut lease = open_fake(&root, FakeState::default()).unwrap();
        fs::write(root.join(OWNER_DIRECTORY).join("unexpected"), b"retained").unwrap();
        assert_eq!(lease.release(), Err(QuotaError::CleanupFailed));
        assert!(root.join(OWNER_DIRECTORY).exists());
    }

    /// Real enforcement is intentionally opt-in and ignored by default. It
    /// must run only on an operator-supplied disposable, exclusive project-
    /// quota root. The ordinary test suite never changes quota configuration.
    #[test]
    #[ignore = "requires OPENCONTINUITY_RUN_QUOTA_INTEGRATION=1 and a disposable delegated quota root"]
    fn kernel_project_quota_atomic_publication_and_accounting() {
        if std::env::var_os("OPENCONTINUITY_RUN_QUOTA_INTEGRATION").as_deref()
            != Some(std::ffi::OsStr::new("1"))
        {
            eprintln!("SKIP: explicit project-quota integration opt-in is absent");
            return;
        }
        let destination =
            PathBuf::from(required_environment("OPENCONTINUITY_AIRDROP_QUOTA_ROOT").unwrap());
        let mut lease = ProjectQuotaLease::from_environment(&destination).unwrap();
        let before = lease.revalidate().unwrap();
        assert!(before.hard_bytes <= 64 * 1024 * 1024);
        assert!(before.hard_inodes <= 4096);
        assert!(before.hard_bytes.saturating_sub(before.used_bytes) >= 2 * 1024 * 1024);
        assert!(before.hard_inodes.saturating_sub(before.used_inodes) >= 8);

        let existing = fs::read_dir(&destination)
            .unwrap()
            .map(|entry| entry.unwrap().file_name())
            .collect::<std::collections::HashSet<_>>();

        let staging = crate::airdrop::storage::ReceiveStaging::create(&destination).unwrap();
        let payload = staging.path().join("quota-fixture.bin");
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o600)
            .open(payload)
            .unwrap();
        file.write_all(&vec![0x5a; 1024 * 1024]).unwrap();
        file.sync_all().unwrap();
        drop(file);
        lease.verify_directory(staging.path()).unwrap();
        let allocated = lease.revalidate().unwrap();
        assert!(allocated.used_bytes > before.used_bytes);
        assert!(allocated.used_inodes > before.used_inodes);

        let published = staging.finalize().unwrap();
        assert_eq!((published.files, published.bytes), (1, 1024 * 1024));
        let after = lease.revalidate().unwrap();
        assert!(after.used_bytes >= allocated.used_bytes);
        assert!(after.used_inodes >= allocated.used_inodes);

        let bundle = fs::read_dir(&destination)
            .unwrap()
            .map(|entry| entry.unwrap())
            .find(|entry| {
                !existing.contains(&entry.file_name())
                    && entry
                        .file_name()
                        .to_str()
                        .is_some_and(|name| name.starts_with("AirDrop Receive "))
            })
            .unwrap()
            .path();
        fs::remove_dir_all(bundle).unwrap();

        // Two writers race against the same project hard limit. At least one
        // must receive EDQUOT; ENOSPC is not accepted as quota evidence.
        let fixture = destination.join(".opencontinuity-quota-kernel-fixture");
        let mut builder = fs::DirBuilder::new();
        builder.mode(0o700).create(&fixture).unwrap();
        let available = lease
            .revalidate()
            .unwrap()
            .hard_bytes
            .saturating_sub(lease.revalidate().unwrap().used_bytes);
        let errors = Arc::new(Mutex::new(Vec::new()));
        let mut writers = Vec::new();
        for writer in 0..2 {
            let path = fixture.join(format!(".owly-staging-concurrent-{writer}"));
            let errors = errors.clone();
            writers.push(std::thread::spawn(move || {
                let mut file = OpenOptions::new()
                    .write(true)
                    .create_new(true)
                    .mode(0o600)
                    .open(path)
                    .unwrap();
                let chunk = [0xa5; 64 * 1024];
                let mut written = 0_u64;
                while written <= available + 2 * 1024 * 1024 {
                    match file.write(&chunk) {
                        Ok(0) => panic!("quota fixture write made no progress"),
                        Ok(count) => written += count as u64,
                        Err(error) => {
                            errors.lock().unwrap().push(error.raw_os_error());
                            return;
                        }
                    }
                }
                panic!("writer exceeded the configured project hard limit");
            }));
        }
        for writer in writers {
            writer.join().unwrap();
        }
        assert!(
            errors
                .lock()
                .unwrap()
                .iter()
                .any(|error| *error == Some(libc::EDQUOT))
        );
        let exhausted = lease.revalidate().unwrap();
        assert!(exhausted.used_bytes <= exhausted.hard_bytes);
        for entry in fs::read_dir(&fixture).unwrap() {
            fs::remove_file(entry.unwrap().path()).unwrap();
        }

        // A separate create loop proves the project inode hard limit rather
        // than inferring it from the configured number.
        let mut inode_error = None;
        for index in 0..=before.hard_inodes + 4 {
            match OpenOptions::new()
                .write(true)
                .create_new(true)
                .mode(0o600)
                .open(fixture.join(format!("inode-{index}")))
            {
                Ok(_) => {}
                Err(error) => {
                    inode_error = error.raw_os_error();
                    break;
                }
            }
        }
        assert_eq!(inode_error, Some(libc::EDQUOT));
        for entry in fs::read_dir(&fixture).unwrap() {
            fs::remove_file(entry.unwrap().path()).unwrap();
        }
        fs::remove_dir(fixture).unwrap();
        lease.release().unwrap();
    }
}
