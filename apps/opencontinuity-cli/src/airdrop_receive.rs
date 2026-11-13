//! Private staging and no-overwrite finalization for an accepted Owly receive.

use std::ffi::CString;
use std::fs::{self, File, Metadata, OpenOptions};
use std::io;
#[cfg(unix)]
use std::os::fd::AsRawFd;
#[cfg(unix)]
use std::os::unix::fs::OpenOptionsExt;
use std::path::{Path, PathBuf};

pub const MAX_RECEIVE_FILES: u64 = 100;
pub const MAX_RECEIVE_DIRECTORIES: u64 = 128;
pub const MAX_RECEIVE_DEPTH: usize = 8;
pub const MAX_RECEIVE_FILE_BYTES: u64 = 512 * 1024 * 1024;
pub const MAX_RECEIVE_TOTAL_BYTES: u64 = 1024 * 1024 * 1024;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct ReceiveUsage {
    pub files: u64,
    pub directories: u64,
    pub bytes: u64,
}

#[derive(Debug)]
pub struct FinalizedReceive {
    pub files: u64,
    pub bytes: u64,
}

pub struct ReceiveStaging {
    root: PathBuf,
    destination: File,
    keep: bool,
}

/// Process-level singleton lock stored only in a private runtime directory.
#[derive(Debug)]
pub struct ReceiveLock {
    _file: File,
}

/// Per-session state root used to keep Owly's generated TLS identity out of
/// persistent XDG state. It is removed on every normal exit path.
#[derive(Debug)]
pub struct ReceiveSessionState {
    root: PathBuf,
    keep: bool,
}

impl ReceiveSessionState {
    pub fn retain_until_quiescent(&mut self) {
        self.keep = true;
    }
    pub fn create_runtime() -> io::Result<Self> {
        let runtime = std::env::var_os("XDG_RUNTIME_DIR").ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::NotFound,
                "private runtime directory unavailable",
            )
        })?;
        Self::create_in(Path::new(&runtime))
    }

    fn create_in(directory: &Path) -> io::Result<Self> {
        let directory_meta = fs::symlink_metadata(directory)?;
        if !directory_meta.is_dir() || directory_meta.file_type().is_symlink() {
            return Err(io::Error::new(
                io::ErrorKind::PermissionDenied,
                "runtime path is not a directory",
            ));
        }
        #[cfg(unix)]
        {
            use std::os::unix::fs::{MetadataExt, PermissionsExt};
            if directory_meta.uid() != unsafe { libc::geteuid() }
                || directory_meta.permissions().mode() & 0o077 != 0
            {
                return Err(io::Error::new(
                    io::ErrorKind::PermissionDenied,
                    "runtime directory is not private to this user",
                ));
            }
        }
        for _ in 0..8 {
            let mut random = [0_u8; 16];
            getrandom::getrandom(&mut random)
                .map_err(|error| io::Error::other(error.to_string()))?;
            let root = directory.join(format!(
                "opencontinuity-airdrop-state-{}",
                hex_lower(&random)
            ));
            let mut builder = fs::DirBuilder::new();
            #[cfg(unix)]
            builder.mode(0o700);
            match builder.create(&root) {
                Ok(()) => return Ok(Self { root, keep: false }),
                Err(error) if error.kind() == io::ErrorKind::AlreadyExists => continue,
                Err(error) => return Err(error),
            }
        }
        Err(io::Error::new(
            io::ErrorKind::AlreadyExists,
            "could not allocate private receive state",
        ))
    }

    pub fn path(&self) -> &Path {
        &self.root
    }
}

impl Drop for ReceiveSessionState {
    fn drop(&mut self) {
        if !self.keep {
            let _ = fs::remove_dir_all(&self.root);
        }
    }
}

impl ReceiveLock {
    pub fn acquire_runtime() -> io::Result<Self> {
        let runtime = std::env::var_os("XDG_RUNTIME_DIR").ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::NotFound,
                "private runtime directory unavailable",
            )
        })?;
        Self::acquire_in(Path::new(&runtime))
    }

    fn acquire_in(directory: &Path) -> io::Result<Self> {
        let directory_meta = fs::symlink_metadata(directory)?;
        if !directory_meta.is_dir() || directory_meta.file_type().is_symlink() {
            return Err(io::Error::new(
                io::ErrorKind::PermissionDenied,
                "runtime path is not a directory",
            ));
        }
        #[cfg(unix)]
        {
            use std::os::unix::fs::{MetadataExt, PermissionsExt};
            if directory_meta.uid() != unsafe { libc::geteuid() }
                || directory_meta.permissions().mode() & 0o077 != 0
            {
                return Err(io::Error::new(
                    io::ErrorKind::PermissionDenied,
                    "runtime directory is not private to this user",
                ));
            }
        }
        let path = directory.join("opencontinuity-airdrop-receive.lock");
        let file = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .mode(0o600)
            .custom_flags(libc::O_CLOEXEC | libc::O_NOFOLLOW)
            .open(path)?;
        let metadata = file.metadata()?;
        if !metadata.is_file() {
            return Err(io::Error::new(
                io::ErrorKind::PermissionDenied,
                "receive lock is not a regular file",
            ));
        }
        #[cfg(unix)]
        {
            use std::os::unix::fs::{MetadataExt, PermissionsExt};
            if metadata.uid() != unsafe { libc::geteuid() }
                || metadata.permissions().mode() & 0o077 != 0
            {
                return Err(io::Error::new(
                    io::ErrorKind::PermissionDenied,
                    "receive lock is not private to this user",
                ));
            }
        }
        // SAFETY: `file` owns a valid descriptor for the duration of the lock.
        if unsafe { libc::flock(file.as_raw_fd(), libc::LOCK_EX | libc::LOCK_NB) } != 0 {
            return Err(io::Error::last_os_error());
        }
        Ok(Self { _file: file })
    }
}

impl ReceiveStaging {
    pub fn retain_until_quiescent(&mut self) {
        self.keep = true;
    }
    pub fn preserve(mut self) {
        self.keep = true;
    }
    /// Canonicalize the operator-selected existing destination, then create a
    /// private staging directory on the same filesystem for atomic finalizing.
    pub fn create(destination: &Path) -> io::Result<Self> {
        let destination = fs::canonicalize(destination)?;
        if !fs::metadata(&destination)?.is_dir() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "destination is not a directory",
            ));
        }
        let destination = OpenOptions::new()
            .read(true)
            .custom_flags(libc::O_DIRECTORY | libc::O_NOFOLLOW | libc::O_CLOEXEC)
            .open(destination)?;
        let anchored = PathBuf::from(format!("/proc/self/fd/{}", destination.as_raw_fd()));
        for _ in 0..8 {
            let mut random = [0_u8; 16];
            getrandom::getrandom(&mut random)
                .map_err(|error| io::Error::other(error.to_string()))?;
            let name = format!(".opencontinuity-airdrop-{}", hex_lower(&random));
            let root = anchored.join(name);
            let mut builder = fs::DirBuilder::new();
            #[cfg(unix)]
            builder.mode(0o700);
            match builder.create(&root) {
                Ok(()) => {
                    return Ok(Self {
                        root,
                        destination,
                        keep: false,
                    });
                }
                Err(error) if error.kind() == io::ErrorKind::AlreadyExists => continue,
                Err(error) => return Err(error),
            }
        }
        Err(io::Error::new(
            io::ErrorKind::AlreadyExists,
            "could not allocate a private receive directory",
        ))
    }

    pub fn path(&self) -> &Path {
        &self.root
    }

    /// Discard all staged content after an unsafe or over-budget transfer.
    pub fn discard(mut self) -> io::Result<()> {
        fs::remove_dir_all(&self.root)?;
        self.keep = true;
        Ok(())
    }

    /// Inspect a bounded tree without following symlinks. Called while the
    /// backend is active to stop oversized or malformed output promptly.
    pub fn usage(&self) -> io::Result<ReceiveUsage> {
        let mut usage = ReceiveUsage::default();
        scan_directory(&self.root, 0, true, &mut usage)?;
        if usage.files > MAX_RECEIVE_FILES
            || usage.directories > MAX_RECEIVE_DIRECTORIES
            || usage.bytes > MAX_RECEIVE_TOTAL_BYTES
        {
            return Err(limit_error());
        }
        Ok(usage)
    }

    /// Move validated complete outputs into the user-approved destination.
    /// Leftover Owly `.owly-staging-*` directories contain interrupted upload
    /// data and are discarded. Every final move uses RENAME_NOREPLACE.
    pub fn finalize(mut self) -> io::Result<FinalizedReceive> {
        self.usage()?;
        for entry in fs::read_dir(&self.root)? {
            let entry = entry?;
            let file_name = entry.file_name();
            if file_name
                .to_str()
                .is_some_and(|name| name.starts_with(".owly-staging-"))
            {
                let path = entry.path();
                let metadata = fs::symlink_metadata(&path)?;
                if metadata.file_type().is_symlink() || metadata.is_file() {
                    fs::remove_file(path)?;
                } else if metadata.is_dir() {
                    fs::remove_dir_all(path)?;
                } else {
                    return Err(io::Error::new(
                        io::ErrorKind::InvalidData,
                        "unsupported interrupted receive entry",
                    ));
                }
            }
        }

        let mut usage = ReceiveUsage::default();
        scan_directory(&self.root, 0, false, &mut usage)?;
        if usage.files > MAX_RECEIVE_FILES
            || usage.directories > MAX_RECEIVE_DIRECTORIES
            || usage.bytes > MAX_RECEIVE_TOTAL_BYTES
        {
            return Err(limit_error());
        }

        // Owly creates completed files with mode 0644 and directories subject
        // to the backend process umask. Keep accepted content private after it
        // leaves our 0700 staging root as well.
        restrict_tree_permissions(&self.root)?;

        // One rename publishes the entire validated tree, never a prefix.
        let source = path_cstring(
            self.root
                .file_name()
                .map(Path::new)
                .ok_or_else(invalid_name)?,
        )?;
        let mut published = false;
        for _ in 0..8 {
            let mut random = [0_u8; 16];
            getrandom::getrandom(&mut random).map_err(|e| io::Error::other(e.to_string()))?;
            let name = CString::new(format!("AirDrop Receive {}", hex_lower(&random)))
                .map_err(|_| invalid_name())?;
            match rename_bundle(&self.destination, &source, &name) {
                Ok(()) => {
                    published = true;
                    self.keep = true;
                    break;
                }
                Err(e) if e.kind() == io::ErrorKind::AlreadyExists => continue,
                Err(e) => return Err(e),
            }
        }
        if !published {
            return Err(io::Error::new(
                io::ErrorKind::AlreadyExists,
                "bundle collisions exhausted",
            ));
        }
        Ok(FinalizedReceive {
            files: usage.files,
            bytes: usage.bytes,
        })
    }
}

impl Drop for ReceiveStaging {
    fn drop(&mut self) {
        if !self.keep {
            let _ = fs::remove_dir_all(&self.root);
        }
    }
}

fn scan_directory(
    directory: &Path,
    depth: usize,
    allow_upstream_partial: bool,
    usage: &mut ReceiveUsage,
) -> io::Result<()> {
    if depth > MAX_RECEIVE_DEPTH {
        return Err(limit_error());
    }
    let mut entries = fs::read_dir(directory)?.collect::<io::Result<Vec<_>>>()?;
    entries.sort_by_key(|entry| entry.file_name());
    for entry in entries {
        let path = entry.path();
        let name = entry
            .file_name()
            .into_string()
            .map_err(|_| invalid_name())?;
        let metadata = fs::symlink_metadata(&path)?;
        let file_type = metadata.file_type();
        let upstream_partial = allow_upstream_partial
            && depth == 0
            && name.starts_with(".owly-staging-")
            && file_type.is_dir();
        if !upstream_partial && !safe_component(&name) {
            return Err(invalid_name());
        }
        if file_type.is_symlink() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "symlinks are not accepted in receive staging",
            ));
        }
        if file_type.is_dir() {
            usage.directories = usage.directories.saturating_add(1);
            if usage.directories > MAX_RECEIVE_DIRECTORIES {
                return Err(limit_error());
            }
            scan_directory(&path, depth + 1, allow_upstream_partial, usage)?;
        } else if file_type.is_file() {
            add_file_usage(&metadata, usage)?;
        } else {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "non-regular receive output is not accepted",
            ));
        }
        if usage.files > MAX_RECEIVE_FILES || usage.bytes > MAX_RECEIVE_TOTAL_BYTES {
            return Err(limit_error());
        }
    }
    Ok(())
}

fn add_file_usage(metadata: &Metadata, usage: &mut ReceiveUsage) -> io::Result<()> {
    let size = metadata.len();
    if size > MAX_RECEIVE_FILE_BYTES {
        return Err(limit_error());
    }
    #[cfg(unix)]
    if metadata.nlink() != 1 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "multiply linked receive output is not accepted",
        ));
    }
    usage.files = usage.files.saturating_add(1);
    usage.bytes = usage.bytes.checked_add(size).ok_or_else(limit_error)?;
    Ok(())
}

fn restrict_tree_permissions(path: &Path) -> io::Result<()> {
    let metadata = fs::symlink_metadata(path)?;
    let file_type = metadata.file_type();
    if file_type.is_symlink() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "symlinks are not accepted in receive staging",
        ));
    }
    if file_type.is_dir() {
        fs::set_permissions(path, fs::Permissions::from_mode(0o700))?;
        for entry in fs::read_dir(path)? {
            restrict_tree_permissions(&entry?.path())?;
        }
        Ok(())
    } else if file_type.is_file() {
        fs::set_permissions(path, fs::Permissions::from_mode(0o600))
    } else {
        Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "non-regular receive output is not accepted",
        ))
    }
}

fn rename_bundle(destination: &File, source: &CString, target: &CString) -> io::Result<()> {
    // Both names are relative to the pinned destination descriptor. No parent
    // path lookup occurs at publication and the target is never overwritten.
    let result = unsafe {
        libc::renameat2(
            destination.as_raw_fd(),
            source.as_ptr(),
            destination.as_raw_fd(),
            target.as_ptr(),
            libc::RENAME_NOREPLACE,
        )
    };
    if result == 0 {
        Ok(())
    } else {
        Err(io::Error::last_os_error())
    }
}

fn path_cstring(path: &Path) -> io::Result<CString> {
    #[cfg(unix)]
    {
        use std::os::unix::ffi::OsStrExt;
        CString::new(path.as_os_str().as_bytes()).map_err(|_| invalid_name())
    }
    #[cfg(not(unix))]
    {
        CString::new(path.to_string_lossy().as_bytes()).map_err(|_| invalid_name())
    }
}

fn safe_component(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= 255
        && name != "."
        && name != ".."
        && !name.starts_with('.')
        && !name.contains('/')
        && !name.contains('\\')
        && !name.chars().any(|character| {
            character.is_control()
                || matches!(
                    character,
                    '\u{202a}'..='\u{202e}' | '\u{2066}'..='\u{2069}' | '\u{200e}' | '\u{200f}'
                )
        })
}

fn limit_error() -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, "receive output limit exceeded")
}

fn invalid_name() -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, "unsafe receive output name")
}

fn hex_lower(value: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut output = String::with_capacity(value.len() * 2);
    for byte in value {
        output.push(HEX[(byte >> 4) as usize] as char);
        output.push(HEX[(byte & 0x0f) as usize] as char);
    }
    output
}

#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;
#[cfg(unix)]
use std::os::unix::fs::{DirBuilderExt, MetadataExt};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn destination_must_exist_and_be_a_directory() {
        let root = tempfile::tempdir().unwrap();
        let missing = root.path().join("missing");
        assert!(ReceiveStaging::create(&missing).is_err());
        let file = root.path().join("file");
        fs::write(&file, b"x").unwrap();
        assert!(ReceiveStaging::create(&file).is_err());
    }

    #[test]
    fn output_is_private_and_symlink_destination_is_resolved_once() {
        let root = tempfile::tempdir().unwrap();
        let destination = root.path().join("destination");
        fs::create_dir(&destination).unwrap();
        #[cfg(unix)]
        std::os::unix::fs::symlink(&destination, root.path().join("alias")).unwrap();
        #[cfg(unix)]
        let chosen = root.path().join("alias");
        #[cfg(not(unix))]
        let chosen = destination.clone();
        let staging = ReceiveStaging::create(&chosen).unwrap();
        assert_eq!(
            fs::canonicalize(staging.path().parent().unwrap()).unwrap(),
            destination
        );
        #[cfg(unix)]
        assert_eq!(
            fs::metadata(staging.path()).unwrap().permissions().mode() & 0o777,
            0o700
        );
    }

    #[test]
    fn path_traversal_and_unsafe_names_are_rejected() {
        for name in [
            "",
            ".",
            "..",
            "../escape",
            "a/b",
            "a\\b",
            ".hidden",
            "x\u{202e}txt",
        ] {
            assert!(!safe_component(name), "{name:?}");
        }
        let root = tempfile::tempdir().unwrap();
        let stage = ReceiveStaging::create(root.path()).unwrap();
        fs::write(stage.path().join(".hidden"), b"hidden").unwrap();
        assert_eq!(
            stage.usage().unwrap_err().kind(),
            io::ErrorKind::InvalidData
        );
    }

    #[cfg(unix)]
    #[test]
    fn symlink_escape_and_special_outputs_are_rejected_and_not_moved() {
        use std::os::unix::fs::symlink;
        let root = tempfile::tempdir().unwrap();
        let destination = root.path().join("destination");
        let outside = root.path().join("outside");
        fs::create_dir(&destination).unwrap();
        fs::create_dir(&outside).unwrap();
        fs::write(outside.join("keep"), b"outside").unwrap();
        let stage = ReceiveStaging::create(&destination).unwrap();
        symlink(&outside, stage.path().join("escape")).unwrap();
        assert_eq!(
            stage.usage().unwrap_err().kind(),
            io::ErrorKind::InvalidData
        );
        drop(stage);
        assert_eq!(fs::read(outside.join("keep")).unwrap(), b"outside");
        assert!(!destination.join("escape").exists());
    }

    #[test]
    fn zero_byte_files_finalize_and_duplicates_never_overwrite() {
        let root = tempfile::tempdir().unwrap();
        let destination = root.path().join("destination");
        fs::create_dir(&destination).unwrap();
        fs::write(destination.join("empty.txt"), b"existing").unwrap();
        let stage = ReceiveStaging::create(&destination).unwrap();
        fs::write(stage.path().join("empty.txt"), b"").unwrap();
        let finalized = stage.finalize().unwrap();
        assert_eq!(finalized.files, 1);
        assert_eq!(finalized.bytes, 0);
        assert_eq!(
            fs::read(destination.join("empty.txt")).unwrap(),
            b"existing"
        );
        assert_eq!(
            fs::read(bundle_path(&destination).join("empty.txt")).unwrap(),
            b""
        );
        #[cfg(unix)]
        assert_eq!(
            fs::metadata(bundle_path(&destination).join("empty.txt"))
                .unwrap()
                .permissions()
                .mode()
                & 0o777,
            0o600
        );
    }

    fn bundle_path(destination: &Path) -> PathBuf {
        fs::read_dir(destination)
            .unwrap()
            .map(|e| e.unwrap().path())
            .find(|p| {
                p.file_name()
                    .unwrap()
                    .to_string_lossy()
                    .starts_with("AirDrop Receive ")
            })
            .unwrap()
    }

    #[test]
    fn atomic_bundle_nested_counts_and_permissions() {
        let root = tempfile::tempdir().unwrap();
        let stage = ReceiveStaging::create(root.path()).unwrap();
        fs::create_dir(stage.path().join("folder")).unwrap();
        fs::write(stage.path().join("folder/表.txt"), b"abc").unwrap();
        fs::write(stage.path().join("empty"), b"").unwrap();
        let result = stage.finalize().unwrap();
        assert_eq!((result.files, result.bytes), (2, 3));
        assert_eq!(fs::read_dir(root.path()).unwrap().count(), 1);
        let bundle = bundle_path(root.path());
        assert_eq!(fs::metadata(&bundle).unwrap().mode() & 0o777, 0o700);
        assert_eq!(
            fs::metadata(bundle.join("folder")).unwrap().mode() & 0o777,
            0o700
        );
        assert_eq!(
            fs::metadata(bundle.join("folder/表.txt")).unwrap().mode() & 0o777,
            0o600
        );
        assert!(safe_component(&"表".repeat(85)));
        assert!(!safe_component(&"表".repeat(86)));
    }

    #[test]
    fn collision_and_rename_failure_leave_source_unpublished() {
        let root = tempfile::tempdir().unwrap();
        let stage = ReceiveStaging::create(root.path()).unwrap();
        fs::write(stage.path().join("file"), b"content").unwrap();
        let target = CString::new("AirDrop Receive collision").unwrap();
        fs::create_dir(root.path().join(target.to_str().unwrap())).unwrap();
        let source = path_cstring(Path::new(stage.root.file_name().unwrap())).unwrap();
        assert_eq!(
            rename_bundle(&stage.destination, &source, &target)
                .unwrap_err()
                .kind(),
            io::ErrorKind::AlreadyExists
        );
        assert!(stage.path().join("file").exists());
        let invalid = CString::new("missing/target").unwrap();
        assert!(rename_bundle(&stage.destination, &source, &invalid).is_err());
        assert!(stage.path().join("file").exists());
    }

    #[test]
    fn hardlinks_fifo_and_total_limit_never_publish() {
        for kind in ["hardlink", "fifo", "total"] {
            let root = tempfile::tempdir().unwrap();
            let stage = ReceiveStaging::create(root.path()).unwrap();
            fs::write(stage.path().join("first"), b"x").unwrap();
            match kind {
                "hardlink" => {
                    fs::hard_link(stage.path().join("first"), stage.path().join("second")).unwrap()
                }
                "fifo" => {
                    let path = path_cstring(&stage.path().join("fifo")).unwrap();
                    assert_eq!(unsafe { libc::mkfifo(path.as_ptr(), 0o600) }, 0);
                }
                _ => {
                    for i in 0..3 {
                        File::create(stage.path().join(format!("large{i}")))
                            .unwrap()
                            .set_len(MAX_RECEIVE_FILE_BYTES)
                            .unwrap();
                    }
                }
            }
            assert!(stage.finalize().is_err(), "{kind}");
            assert_eq!(fs::read_dir(root.path()).unwrap().count(), 0);
        }
    }

    #[test]
    fn concurrent_collision_is_kernel_enforced_and_parent_rename_is_pinned() {
        let root = tempfile::tempdir().unwrap();
        let destination = root.path().join("destination");
        fs::create_dir(&destination).unwrap();
        let stage = ReceiveStaging::create(&destination).unwrap();
        fs::write(stage.path().join("file"), b"abc").unwrap();
        let moved = root.path().join("moved");
        fs::rename(&destination, &moved).unwrap();
        fs::create_dir(&destination).unwrap();
        stage.finalize().unwrap();
        assert_eq!(fs::read_dir(&destination).unwrap().count(), 0);
        assert_eq!(fs::read(bundle_path(&moved).join("file")).unwrap(), b"abc");

        let stage = ReceiveStaging::create(&destination).unwrap();
        let source = path_cstring(Path::new(stage.root.file_name().unwrap())).unwrap();
        let fd = stage.destination.try_clone().unwrap();
        let barrier = std::sync::Arc::new(std::sync::Barrier::new(2));
        let other = barrier.clone();
        let maker = std::thread::spawn(move || {
            other.wait();
            let target = CString::new("AirDrop Receive concurrent").unwrap();
            unsafe { libc::mkdirat(fd.as_raw_fd(), target.as_ptr(), 0o700) }
        });
        barrier.wait();
        let target = CString::new("AirDrop Receive concurrent").unwrap();
        let result = rename_bundle(&stage.destination, &source, &target);
        let created = maker.join().unwrap();
        assert!(result.is_ok() || created == 0);
        if let Err(error) = result {
            assert_eq!(error.kind(), io::ErrorKind::AlreadyExists);
        }
    }

    #[test]
    fn validation_failure_never_publishes_a_prefix() {
        let root = tempfile::tempdir().unwrap();
        let stage = ReceiveStaging::create(root.path()).unwrap();
        fs::write(stage.path().join("good"), b"good").unwrap();
        fs::write(stage.path().join(".bad"), b"bad").unwrap();
        assert!(stage.finalize().is_err());
        assert_eq!(fs::read_dir(root.path()).unwrap().count(), 0);
    }

    #[test]
    fn private_session_state_is_removed_on_drop() {
        let runtime = tempfile::tempdir().unwrap();
        #[cfg(unix)]
        fs::set_permissions(runtime.path(), fs::Permissions::from_mode(0o700)).unwrap();
        let state = ReceiveSessionState::create_in(runtime.path()).unwrap();
        let state_path = state.path().to_path_buf();
        #[cfg(unix)]
        assert_eq!(
            fs::metadata(&state_path).unwrap().permissions().mode() & 0o777,
            0o700
        );
        fs::write(state_path.join("private-key-material"), b"test only").unwrap();
        drop(state);
        assert!(!state_path.exists());
    }

    #[test]
    fn discard_removes_untrusted_staged_output() {
        let root = tempfile::tempdir().unwrap();
        let destination = root.path().join("destination");
        fs::create_dir(&destination).unwrap();
        let stage = ReceiveStaging::create(&destination).unwrap();
        let stage_path = stage.path().to_path_buf();
        fs::write(stage_path.join("partial"), b"not finalized").unwrap();
        stage.discard().unwrap();
        assert!(!stage_path.exists());
        assert!(fs::read_dir(destination).unwrap().next().is_none());
    }

    #[test]
    fn interrupted_owly_staging_is_removed_without_publishing_partial_files() {
        let root = tempfile::tempdir().unwrap();
        let destination = root.path().join("destination");
        fs::create_dir(&destination).unwrap();
        let stage = ReceiveStaging::create(&destination).unwrap();
        let partial = stage.path().join(".owly-staging-interrupted");
        fs::create_dir(&partial).unwrap();
        fs::write(partial.join("part"), b"partial").unwrap();
        fs::write(stage.path().join("complete.txt"), b"complete").unwrap();
        let result = stage.finalize().unwrap();
        assert_eq!(result.files, 1);
        assert_eq!(
            fs::read(bundle_path(&destination).join("complete.txt")).unwrap(),
            b"complete"
        );
        assert!(!partial.exists());
    }

    #[test]
    fn file_and_total_size_limits_are_enforced() {
        let root = tempfile::tempdir().unwrap();
        let stage = ReceiveStaging::create(root.path()).unwrap();
        let sparse = File::create(stage.path().join("large.bin")).unwrap();
        sparse
            .set_len(MAX_RECEIVE_FILE_BYTES + 1)
            .expect("sparse test file");
        assert_eq!(
            stage.usage().unwrap_err().kind(),
            io::ErrorKind::InvalidData
        );
    }

    #[test]
    fn directory_depth_is_bounded() {
        let root = tempfile::tempdir().unwrap();
        let stage = ReceiveStaging::create(root.path()).unwrap();
        let mut nested = stage.path().to_path_buf();
        for i in 0..=MAX_RECEIVE_DEPTH {
            nested.push(format!("d{i}"));
            fs::create_dir(&nested).unwrap();
        }
        assert_eq!(
            stage.usage().unwrap_err().kind(),
            io::ErrorKind::InvalidData
        );
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn concurrent_receives_are_rejected_by_the_lock() {
        let root = tempfile::tempdir().unwrap();
        fs::set_permissions(root.path(), fs::Permissions::from_mode(0o700)).unwrap();
        let first = ReceiveLock::acquire_in(root.path()).unwrap();
        assert_eq!(
            ReceiveLock::acquire_in(root.path()).unwrap_err().kind(),
            io::ErrorKind::WouldBlock
        );
        drop(first);
        assert!(ReceiveLock::acquire_in(root.path()).is_ok());
    }
}
