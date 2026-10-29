# P007B — Real BlueZ ↔ Apple Watch Foreground BLE Interoperability Qualification

Status: **STARTED — scope and validation protocol only. Hardware tests NOT RUN.**
Branch: `research/p007b-watch-bluez-physical-interop`
Base: `main` `1cf956bd7356df2e7e2800da5657c01d12f7b256` (P007/P007A squash merge).
P007 upstream head: `e7d1d22a1858692fe1d6a1bc9bc770df6f6b2680`, PR #7.

## Provenance and boundary

The authoritative original P001–P060 roadmap is unavailable. This P007B is the next provisional Watch research milestone, **not a verified original milestone**. P001 production Bluetooth platform adapter and Qt6/QML Settings remain incomplete. P007/P007A provided SwiftUI/CoreBluetooth foreground source, nine Swift tests and a successful unsigned watchOS SDK compile on macOS CI; these do **not** establish real Bluetooth or Watch operation. No auto unlock or operating-system authorization exists.

## Objective

Obtain independently reproducible evidence for a foreground Apple Watch SE 2 central communicating with the Linux Rust/BlueZ GATT peripheral over a real Bluetooth LE controller, or produce a precise, evidence-backed BLOCKED report if the requisite controlled equipment is unavailable.

### Evidence stages — keep them separate

1. Host inventory: Arch Linux version, kernel, BlueZ, D-Bus and local Bluetooth controller firmware/capabilities (read-only).
2. API preflight: powered adapter, `GattManager1`, `LEAdvertisingManager1`, permission/policy, advertisement capacity.
3. Explicit opt-in, bounded GATT application registration and LEAdvertisement registration, and cleanup confirmation.
4. Over-the-air advertisement observed by a separate owned BLE scanner (independent of D-Bus registration).
5. Signed watchOS app installed and launched on physical Apple Watch SE 2 paired with an owned iPhone; record Watch/watchOS, iPhone/iOS and Mac/Xcode versions without identifiers.
6. Foreground Watch scan, service and five-characteristic discovery.
7. Read exactly 1-byte version, 25-byte challenge, test long-read behavior at actual negotiated conditions and fail closed on truncation.
8. Sequential `.withResponse` 20-byte frames (15-byte header + up to 5-byte payload), Complete, read exact 44-byte response and verify session ID, length and SHA-256 digest. Record actual callback/errors.
9. Cancel flow with 44-byte status 2 and SHA-256 of literal UTF-8 `cancelled`, error path with 11-byte response.
10. Disconnect/reconnect, interrupted writes and pending results, timeout, app background/suspension, server SIGINT/SIGTERM/expiration, and BlueZ-side cleanup. Distinguish actual signal receipt from a physical reconnection inference.

Every stage recorded as PASS/FAIL/BLOCKED/NOT RUN with timestamps, host/device/software versions, reproduction commands and sanitized raw evidence. No success inferred from a previous stage.

## Existing exact non-authorizing contract

Service: `8e6f0001-6f4e-4c4f-9b4f-4f70656e436f`
Characteristics: `8e6f0002` version (read), `0003` challenge (read), `0004` exchange (write with response), `0005` response (read), `0006` control (write with response) — use full UUIDs in the source.
Challenge: 25 bytes = v1 + 8-byte session ID + 16-byte random challenge.
Frame: 15-byte `OC`/version/ID/sequence/flags/total_length little-endian header plus nonempty chunk; 20-byte conservative write, 5-byte max data chunk.
Complete control = 1. Cancel = 2. Success status = 0, Cancel status = 2; 44-byte response carries ID, payload length and SHA-256; protocol errors are 11 bytes.
Server synthetic result TTL 10 s; session idle 30 s; runtime 1–300 s.
P006A BlueZ disconnect monitor orders only signals actually received before GATT requests on that connection; missing/late RF signals cannot be retroactively detected.

## Engineering deliverables

A. Improve scripts/host qualification and structured, privacy-preserving experimental diagnostics only if real observed failures demand it. Keep any new host probes read-only until separate explicit trial. Never power adapter, pair, change D-Bus policy, enable elevated service, or alter configuration automatically.
B. Create a repeatable Apple Watch signed-install and test handoff: XcodeGen `experiments/watchos/project.yml`, Xcode/watchOS versions, project generation, target/scheme, development team setup, non-secret signing configuration and install procedure. Signing is a manual operator action; never upload private keys, provisioning profiles or Apple credentials.
C. Produce an operational validation runbook and an evidence template with stage IDs, expected outputs, redaction guidance and troubleshooting by layer (bus/API ↔ controller ↔ RF ↔ discovery ↔ ATT/read/write ↔ Swift protocol ↔ cleanup).
D. Where possible, add **opt-in, bounded, non-sensitive, redacted diagnostic events** on Linux and Watch that do not leak BLE MACs, persistent identifiers, session keys/challenges, raw packets or Apple account information. Diagnostic logs must still say `authorization: never_evaluated`.
E. Confirm the existing SwiftUI watchdog, foreground state transitions, short writes and 25/44-byte CoreBluetooth reads on physical equipment. For long-read failures, document actual values and analyze correct public-API/BlueZ alternatives, backed by tests. Do not invent CoreBluetooth manual offset API.
F. Add deterministic regression tests for bugs that P007B actually fixes. Preserve Rust 1.85 and Swift tests/watchOS compilation gates.

## Concrete operator trial

Read-only-first on owned Arch host:
```sh
cargo build --workspace
scripts/qualify-bluez-host.sh
target/debug/opencontinuity research watch-ble --capabilities --json
```
After explicit choice and a powered adapter:
```sh
target/debug/opencontinuity research watch-gatt serve --adapter hci0 --duration-seconds 120 --experimental --json
```
Check actual adapter name and policy first. Use a separately owned BLE scanner to confirm the UUID is on air. Use paired physical Watch app for the foreground synthetic trial. Do not conflate Swift CI with real radio.

## Acceptance conditions

- Source/CI foundation preserved; P001–P007 regressions pass.
- Host and Watch *physical* stages PASS **only** with actual evidence, else explicitly BLOCKED/NOT RUN.
- Challenge 25 bytes and result 44 bytes delivered completely or a documented reproducible FAIL with a separately scoped, tested remedy.
- Success and Cancel synthetic transfers, timeout, disconnect/reconnect and cleanup have stage-specific evidence; no fake verification.
- Rust fmt, Clippy, test, build, diff check; Swift package tests, watchOS unsigned SDK build where available; hosted checks recorded on final commit.
- Documentation: `docs/research/P007B_REAL_BLE_RUNBOOK.md`, `docs/research/P007B_APPLE_SIGNED_INSTALL.md`, `docs/research/P007B_LONG_READ_AND_RECONNECT.md`, `docs/P007B_VALIDATION.md`, update architecture/feature matrix/roadmap if needed.
- PR against main, no auto merge, no P008. If the environment lacks physical Apple/Linux devices or a signed Watch app, leave PR draft and report exact limitations. Distinguish an evidence/runbook submilestone from a successful physical interoperability milestone.

## Safety and scope exclusions

No production Watch identity verification, host enrollment, P-256/Secure Enclave approval, PAM, polkit, login manager, background automatic unlock, Apple-private STS/AWDL/Keychain/IRK, RSSI-derived proximity authorization or user account secrets. Every experimental diagnostic retains `authorization: never_evaluated`; even successful transport is **not** authentication or unlock.
