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

## P004 exposure

P004 models an explicitly separate in-memory test enrollment record and verifies mock responses against its enrolled public key. Unknown, revoked, inactive, wrong-algorithm, wrong-binding, invalid-signature and substituted-key cases reject. The record and Ed25519 keys are deterministic test artifacts, not secure enrollment or production storage. The P004 `watch-ble --capabilities` path only read BlueZ object information; at that point there was no GATT registration, pairing, or adapter state change. P004 host observations are recorded in its validation file.

P004 does not authenticate Linux to the Watch. A malicious GATT peripheral could attempt to prompt for approval; a production Watch app must verify an enrolled host signature before showing host/action consent. P004 documents this mutual-authentication boundary but has not implemented it. Approval is neither wearer proof nor distance proof. Do not use this prototype for login, PAM, lock-screen or privileged decisions.

## P005 experimental transport exposure

P005’s opt-in CLI exports a temporary custom GATT service and LE advertisement. A nearby party can observe the service UUID/name and any active client can send malformed or oversized writes; per-client framing, session checks, 1024-byte total bounds and timeout/disconnect cleanup limit the experiment. Random challenge/session values come from the OS random source. Advertisement and application registration are owned by this process and are unregistered in reverse order. No adapter settings or pairing state are changed. D-Bus denial fails visibly; no policy is bypassed.

The data and digest response are synthetic transport test values. They do not identify a Watch, validate enrollment, prove proximity, or authorize the operating system. BLE discovery and connection are not identity or authentication. A future Watch app must verify an independently enrolled Linux host identity before presenting any prompt; a future signature would still not prevent a relay or prove distance. P007 adds an experimental foreground watchOS SwiftUI/CoreBluetooth client for this synthetic exchange. It does not add a production Bluetooth adapter, Qt6/QML Settings app, PAM, polkit, login manager, or unlock code. The source is not Apple-compiled and real BlueZ and Apple hardware behavior remain unverified here.

## Controls for future work

Keep protocols in portable code and hardware/system access behind narrow adapters. Validate all untrusted frames and lengths. Redact secrets and captures. Use explicit user consent for pairing and device writes. Keep tests deterministic and sanitized. Review least privilege and license compatibility before adding dependencies. No feature may modify PAM, sudoers, authentication configuration, Bluetooth global identity, Wi-Fi drivers, kernel modules or lock-screen security without a separately designed and reviewed milestone; P001 must not do so.

## P006 research transport

See [P006 GATT session hardening](research/P006_GATT_SESSION_HARDENING.md) for implemented state/resource controls and unresolved nearby-client exhaustion, unreported disconnects, relay and compromised-host risks. Synthetic acknowledgments, pairing and proximity signals never authorize; `authorization: never_evaluated`.

## P008 signed approval research

P008 adds a separate P-256/SHA-256 signed profile with role-separated request and response transcripts, strict bounded canonical encoding, independent test-only host/Watch enrollment records, expiry checks and bounded in-memory replay guards. The Watch consent component accepts only a request verified by the pinned host key before UI presentation. Approval requires an explicit foreground action and a supported LocalAuthentication result; cancellation, unsupported policy, denial and expiry return no signature. The test-only fixture CLI has no privileged, Bluetooth, key-store or account-management side effects. All reports retain `authorization: never_evaluated`.

Enrollment pins are not authenticated out-of-band, keys are not stored persistently, replay state does not survive restart, and app key signatures or LocalAuthentication results do not attest genuine Watch hardware, wearer presence, secure distance or uncompromised software. Relays, clock drift, stolen/compromised endpoints and denial of service remain unresolved. P008 does not modify PAM, polkit, login managers or desktop unlock; it is not Apple's proprietary Watch-to-Mac Auto Unlock protocol, and P007's synthetic SHA-256 acknowledgment remains unauthenticated. See the [P008 protocol](research/P008_AUTH_PROTOCOL.md), [enrollment boundaries](research/P008_ENROLLMENT_TRUST.md), [consent research](research/P008_WATCH_CONSENT.md) and [security review](research/P008_SECURITY_REVIEW.md).

## P009 authenticated research transport

P009 adds separate versioned framing and portable transfer state. Size, sequence, transfer ID, direction, exact length, explicit completion, owner substitution and timeout checks are exercised in Rust tests; the Swift counterpart tests the same frame contract and GET/read/ACK cursor. An isolated BlueZ service binds one client to one signed request and invalidates it on disconnect, owner change or timeout. The Watch CoreBluetooth manager reads one selected request frame per acknowledged GET/read/ACK cycle, verifies the pinned host signature before showing consent, and sends sequential response writes with response. CI carries actual serialized Rust request frames through Swift reassembly and CryptoKit verification, then returns Swift-signed response frames to Rust/P008 verification. These controls cover source and software fixtures only. They do not establish Apple SDK build success, BlueZ runtime behavior or RF interoperability.

Transfer IDs and ACKs are not authenticated identity. P008 signatures and independent enrollment records authenticate only test keys; they do not prove proximity or genuine Watch hardware. Replay state remains in memory. The shipped app has no reviewed enrollment/key injection path, so the authenticated scan is unavailable from its UI and live signed approval remains blocked. See [P009 transport design](research/P009_AUTH_TRANSPORT_DESIGN.md), [trust/consent integration](research/P009_TRUST_AND_CONSENT_INTEGRATION.md), and [threat review](research/P009_TRANSPORT_THREAT_REVIEW.md). Reports must retain `authorization: never_evaluated`.
