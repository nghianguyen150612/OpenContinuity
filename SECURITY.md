# Security Policy

OpenContinuity handles security-sensitive interoperability work. Report vulnerabilities privately to the repository maintainers through GitHub's private vulnerability reporting if enabled. If it is unavailable, contact the repository owner through a verified project channel; do not publish secrets or exploit details in a public issue.

## Security baseline

- Least privilege, no telemetry, and fail-closed behavior are defaults.
- Bluetooth proximity is not authorization. RSSI, IRK resolution, or an unlocked-state advertisement alone cannot authorize OS unlock.
- Apple Watch Auto Unlock requires authenticated trust, session freshness and suitable proximity assurance before any future unlock integration.
- Detection, trusted identity, cryptographic authentication, freshness, proximity assurance and operational readiness must be represented separately.
- Do not log or commit personal pairing keys, IRKs, Apple Account tokens, or unredacted captures. Redact test fixtures and diagnostics.
- No component may modify PAM, sudoers, system authentication policy, global Bluetooth identity, Wi-Fi drivers, kernel modules or lock-screen security as a side effect.

P001 performs no scanning, authentication changes, device writes, radio takeover, or service installation. The daemon is a foreground, nonprivileged idle process.

## Supported report scope

The doctor report describes this build's known implementation status; it does not perform security checks on the host, discover devices, or establish trust. Unknown trust fields must remain unknown until proven by explicit secure procedures.
