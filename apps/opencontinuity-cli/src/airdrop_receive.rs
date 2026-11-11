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
    destination: PathBuf,
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
}

impl ReceiveSessionState {
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
                Ok(()) => return Ok(Self { root }),
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
        let _ = fs::remove_dir_all(&self.root);
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
        for _ in 0..8 {
            let mut random = [0_u8; 16];
            getrandom::getrandom(&mut random)
                .map_err(|error| io::Error::other(error.to_string()))?;
            let name = format!(".opencontinuity-airdrop-{}", hex_lower(&random));
            let root = destination.join(name);
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

        let mut top_level = fs::read_dir(&self.root)?
            .map(|entry| entry.map(|entry| entry.path()))
            .collect::<io::Result<Vec<_>>>()?;
        top_level.sort();
        for path in top_level {
            let name = path
                .file_name()
                .and_then(|value| value.to_str())
                .ok_or_else(invalid_name)?;
            if !safe_component(name) {
                return Err(invalid_name());
            }
            move_without_overwrite(&path, &self.destination, name)?;
        }
        self.keep = true;
        let _ = fs::remove_dir(&self.root);
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

fn move_without_overwrite(source: &Path, destination: &Path, name: &str) -> io::Result<()> {
    for suffix in 0_u16..1000 {
        let candidate = collision_name(name, suffix);
        let from = path_cstring(source)?;
        let to = path_cstring(&destination.join(candidate))?;
        // SAFETY: both C strings are NUL-terminated, valid for this call, and
        // RENAME_NOREPLACE asks the kernel to atomically reject collisions.
        let result = unsafe {
            libc::renameat2(
                libc::AT_FDCWD,
                from.as_ptr(),
                libc::AT_FDCWD,
                to.as_ptr(),
                libc::RENAME_NOREPLACE,
            )
        };
        if result == 0 {
            return Ok(());
        }
        let error = io::Error::last_os_error();
        if error.kind() != io::ErrorKind::AlreadyExists {
            return Err(error);
        }
    }
    Err(io::Error::new(
        io::ErrorKind::AlreadyExists,
        "no unused destination name remains",
    ))
}

fn collision_name(name: &str, suffix: u16) -> String {
    if suffix == 0 {
        return name.to_string();
    }
    let path = Path::new(name);
    let suffix = format!(" ({suffix})");
    match (
        path.file_stem().and_then(|v| v.to_str()),
        path.extension().and_then(|v| v.to_str()),
    ) {
        (Some(stem), Some(extension)) => {
            let mut extension_budget = 255_usize.saturating_sub(suffix.len() + 1);
            let mut stem_budget = extension_budget.saturating_sub(extension.len());
            if extension.len() + suffix.len() + 1 >= 255 {
                stem_budget = 0;
                extension_budget = extension_budget.min(extension.len());
            }
            let stem = truncate_utf8(stem, stem_budget);
            let extension = truncate_utf8(extension, extension_budget);
            format!("{stem}{suffix}.{extension}")
        }
        (Some(stem), None) => {
            format!("{}{suffix}", truncate_utf8(stem, 255 - suffix.len()))
        }
        _ => format!("received{suffix}"),
    }
}

fn truncate_utf8(value: &str, maximum_bytes: usize) -> &str {
    let end = value
        .char_indices()
        .take_while(|(index, character)| index + character.len_utf8() <= maximum_bytes)
        .map(|(index, character)| index + character.len_utf8())
        .last()
        .unwrap_or(0);
    &value[..end]
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
        assert_eq!(staging.path().parent(), Some(destination.as_path()));
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
        assert_eq!(fs::read(destination.join("empty (1).txt")).unwrap(), b"");
        #[cfg(unix)]
        assert_eq!(
            fs::metadata(destination.join("empty (1).txt"))
                .unwrap()
                .permissions()
                .mode()
                & 0o777,
            0o600
        );
    }

    #[test]
    fn collision_suffix_preserves_utf8_and_component_limit() {
        let long = format!("{}.txt", "表".repeat(84));
        let collided = collision_name(&long, 1);
        assert!(collided.len() <= 255);
        assert!(collided.ends_with(" (1).txt"));
        assert!(collided.is_char_boundary(collided.len()));
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
            fs::read(destination.join("complete.txt")).unwrap(),
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
