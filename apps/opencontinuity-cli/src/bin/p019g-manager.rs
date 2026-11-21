#[path = "../trusted_manager.rs"]
mod trusted_manager;

use std::path::PathBuf;
use std::process::ExitCode;
use trusted_manager::{Identity, connect, serve};

fn main() -> ExitCode {
    let mut args = std::env::args().skip(1);
    match args.next().as_deref() {
        Some("serve") => match parse_serve(args.collect()) {
            Ok((socket, pid, uid, identity)) => match serve(&socket, pid, uid, identity) {
                Ok(()) => ExitCode::SUCCESS,
                Err(error) => {
                    eprintln!("guardian stopped fail-closed: {error:?}");
                    ExitCode::from(1)
                }
            },
            Err(error) => usage(error),
        },
        Some("ping") => match parse_ping(args.collect()) {
            Ok((socket, identity, sequence)) => match connect(&socket, identity, sequence) {
                Ok(_) => {
                    println!("P019G protocol acknowledgement received");
                    ExitCode::SUCCESS
                }
                Err(error) => {
                    eprintln!("P019G protocol rejected: {error}");
                    ExitCode::from(1)
                }
            },
            Err(error) => usage(error),
        },
        Some("test-supervisor") => match parse_supervisor(args.collect()) {
            Ok((socket, identity, sequence, hold)) => {
                let deadline = std::time::Instant::now() + std::time::Duration::from_secs(3);
                loop {
                    match connect(&socket, identity, sequence) {
                        Ok(_) => break,
                        Err(error)
                            if std::time::Instant::now() < deadline
                                && error.kind() == std::io::ErrorKind::NotFound =>
                        {
                            std::thread::sleep(std::time::Duration::from_millis(10));
                        }
                        Err(error) => {
                            eprintln!("test supervisor IPC failed: {error}");
                            return ExitCode::from(1);
                        }
                    }
                }
                println!("P019G supervisor acknowledged");
                if hold {
                    loop {
                        std::thread::sleep(std::time::Duration::from_secs(60));
                    }
                }
                ExitCode::SUCCESS
            }
            Err(error) => usage(error),
        },
        _ => usage("expected serve or ping"),
    }
}

fn parse_supervisor(mut args: Vec<String>) -> Result<(PathBuf, Identity, u64, bool), &'static str> {
    let hold = if args.last().is_some_and(|value| value == "--hold") {
        args.pop();
        true
    } else {
        false
    };
    let (socket, identity, sequence) = parse_ping(args)?;
    Ok((socket, identity, sequence, hold))
}

fn usage(error: &str) -> ExitCode {
    eprintln!(
        "{error}\nUsage:\n  p019g-manager serve --socket <absolute-path-in-0700-dir> --supervisor-pid <pid> --supervisor-uid <uid> --instance <32-lowercase-hex> --session <32-lowercase-hex> --epoch <32-lowercase-hex>\n  p019g-manager ping --socket <path> --instance <hex> --session <hex> --epoch <hex> --sequence <n>"
    );
    ExitCode::from(2)
}

fn parse_serve(
    args: Vec<String>,
) -> Result<(PathBuf, libc::pid_t, libc::uid_t, Identity), &'static str> {
    let mut socket = None;
    let mut pid = None;
    let mut uid = None;
    let mut instance = None;
    let mut session = None;
    let mut epoch = None;
    let mut args = args.into_iter();
    while let Some(option) = args.next() {
        let value = args.next().ok_or("missing option value")?;
        match option.as_str() {
            "--socket" if socket.is_none() => socket = Some(PathBuf::from(value)),
            "--supervisor-pid" if pid.is_none() => {
                pid = Some(value.parse().map_err(|_| "invalid pid")?)
            }
            "--supervisor-uid" if uid.is_none() => {
                uid = Some(value.parse().map_err(|_| "invalid uid")?)
            }
            "--instance" if instance.is_none() => instance = Some(parse_id(&value)?),
            "--session" if session.is_none() => session = Some(parse_id(&value)?),
            "--epoch" if epoch.is_none() => epoch = Some(parse_id(&value)?),
            _ => return Err("unknown or repeated option"),
        }
    }
    let socket = socket.ok_or("missing socket")?;
    if !socket.is_absolute() {
        return Err("socket path must be absolute");
    }
    Ok((
        socket,
        pid.filter(|value| *value > 0)
            .ok_or("missing or invalid pid")?,
        uid.ok_or("missing uid")?,
        Identity {
            instance: instance.ok_or("missing instance")?,
            session: session.ok_or("missing session")?,
            epoch: epoch.ok_or("missing epoch")?,
        },
    ))
}

fn parse_ping(args: Vec<String>) -> Result<(PathBuf, Identity, u64), &'static str> {
    let mut socket = None;
    let mut instance = None;
    let mut session = None;
    let mut epoch = None;
    let mut sequence = None;
    let mut args = args.into_iter();
    while let Some(option) = args.next() {
        let value = args.next().ok_or("missing option value")?;
        match option.as_str() {
            "--socket" if socket.is_none() => socket = Some(PathBuf::from(value)),
            "--instance" if instance.is_none() => instance = Some(parse_id(&value)?),
            "--session" if session.is_none() => session = Some(parse_id(&value)?),
            "--epoch" if epoch.is_none() => epoch = Some(parse_id(&value)?),
            "--sequence" if sequence.is_none() => {
                sequence = Some(value.parse().map_err(|_| "invalid sequence")?)
            }
            _ => return Err("unknown or repeated option"),
        }
    }
    Ok((
        socket.ok_or("missing socket")?,
        Identity {
            instance: instance.ok_or("missing instance")?,
            session: session.ok_or("missing session")?,
            epoch: epoch.ok_or("missing epoch")?,
        },
        sequence
            .filter(|value| *value > 0)
            .ok_or("missing or invalid sequence")?,
    ))
}

fn parse_id(value: &str) -> Result<[u8; 16], &'static str> {
    if value.len() != 32
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    {
        return Err("identity must be 32 lowercase hex characters");
    }
    let mut result = [0_u8; 16];
    for (index, pair) in value.as_bytes().chunks_exact(2).enumerate() {
        result[index] = (hex(pair[0])? << 4) | hex(pair[1])?;
    }
    Ok(result)
}

fn hex(value: u8) -> Result<u8, &'static str> {
    match value {
        b'0'..=b'9' => Ok(value - b'0'),
        b'a'..=b'f' => Ok(value - b'a' + 10),
        _ => Err("invalid identity hex"),
    }
}
