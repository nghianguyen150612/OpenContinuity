//! Cooperative same-UID exclusion for this project's GATT experiments and scans.
//! Abstract sockets need no persistent files and release on process death.
#[cfg(target_os = "linux")]
pub fn acquire(adapter: &str) -> std::io::Result<std::os::unix::net::UnixListener> {
    use std::os::linux::net::SocketAddrExt;
    use std::os::unix::net::{SocketAddr, UnixListener};
    // SAFETY: geteuid has no preconditions and returns the current effective UID.
    let uid = unsafe { libc::geteuid() };
    use std::hash::{Hash, Hasher};
    let mut scope = std::collections::hash_map::DefaultHasher::new();
    std::env::var("DBUS_SYSTEM_BUS_ADDRESS")
        .unwrap_or_else(|_| "system".into())
        .hash(&mut scope);
    adapter.hash(&mut scope);
    let scope = scope.finish();
    let address =
        SocketAddr::from_abstract_name(format!("opencontinuity-radio-{uid}-{scope:016x}"))?;
    UnixListener::bind_addr(&address)
}

#[cfg(all(test, target_os = "linux"))]
mod tests {
    #[test]
    fn guard_excludes_only_same_adapter_and_releases_on_drop() {
        let mut nonce = [0_u8; 8];
        getrandom::getrandom(&mut nonce).unwrap();
        let name = format!("test-{}-{}", std::process::id(), hex(&nonce));
        let first = super::acquire(&name).unwrap();
        assert!(super::acquire(&name).is_err());
        let other = super::acquire(&format!("{name}-other")).unwrap();
        drop(first);
        let reacquired = super::acquire(&name).unwrap();
        assert!(super::acquire(&name).is_err());
        drop(reacquired);
        assert!(super::acquire(&name).is_ok());
        drop(other);
    }

    fn hex(bytes: &[u8]) -> String {
        const DIGITS: &[u8; 16] = b"0123456789abcdef";
        let mut output = String::with_capacity(bytes.len() * 2);
        for byte in bytes {
            output.push(DIGITS[(byte >> 4) as usize] as char);
            output.push(DIGITS[(byte & 0x0f) as usize] as char);
        }
        output
    }
}
