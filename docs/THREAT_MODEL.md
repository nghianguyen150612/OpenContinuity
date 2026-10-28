# Threat Model

## Assets

Future device pairing secrets (including IRKs), Apple Account credentials, device identity, session integrity, OS authentication, user files and network/radio availability.

## Adversaries and failure modes

Nearby spoofed or replaying BLE devices; passive observers; malicious or compromised peers; weak RSSI-based proximity guesses; stale authentication; parser vulnerabilities; accidental secret logging; overprivileged daemons; license violations that prevent lawful distribution.

## Trust and unlock boundary

Presence is not identity, identity is not cryptographic authentication, and authentication is not sufficient without freshness, suitable proximity assurance and operational readiness. A future unlock flow must bind a fresh cryptographic session to a trusted device, resist replay and fail closed on missing or ambiguous evidence. BLE RSSI, IRK resolution and unlocked-state advertisements alone are insufficient. P001 has no unlock path.

## P001 exposure

The CLI only serializes static build capability facts. It does not enumerate adapters or devices. The daemon runs as the invoking user, idles, and stops on a termination signal. No persistent data, telemetry, network listener, privileged helper, system service, key store, or pairing is created.

## P002 branch exposure

P002 adds a fixture-only research command. It parses bounded JSON into sanitized observation metadata; names, company IDs, and RSSI remain untrusted. It does not scan, connect, pair, authenticate, or authorize. The report type can only say `never_evaluated`. See [P002 Security Analysis](research/P002_SECURITY_ANALYSIS.md) for the research threat review.

## P003 endpoint simulation exposure

P003 adds only portable, bounded mock contracts and a deterministic Ed25519 test exchange. A successful mock signature is explicitly simulated and never authorizes. The mock's signing seed and nonce sequence are public test values; no production key or RNG is present. The offline fixture cannot use BlueZ, Apple APIs, networking, keychain, PAM, a root helper, or unlock a Linux session. See [P003 Security Design](research/P003_SECURITY_DESIGN.md).

For a future separately enrolled endpoint, trust must bind a Watch key to one Linux host, account, session and action. The iPhone is an untrusted relay and cannot forge a Watch signature. WatchConnectivity reachability is transport state only; queued transfers can arrive too late. A signed user approval plus Local Authentication does not prove distance: BLE/IP wormholes remain possible, and RSSI or latency is not secure proximity evidence. Stolen Watch, compromised host, key custody, secure enrollment, crash-safe replay storage and key revocation are unresolved before any unlock design.

## Controls for future work

Keep protocols in portable code and hardware/system access behind narrow adapters. Validate all untrusted frames and lengths. Redact secrets and captures. Use explicit user consent for pairing and device writes. Keep tests deterministic and sanitized. Review least privilege and license compatibility before adding dependencies. No feature may modify PAM, sudoers, authentication configuration, Bluetooth global identity, Wi-Fi drivers, kernel modules or lock-screen security without a separately designed and reviewed milestone; P001 must not do so.
