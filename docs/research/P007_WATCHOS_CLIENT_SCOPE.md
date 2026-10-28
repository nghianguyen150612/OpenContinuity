# P007 — Foreground watchOS BLE Central Client and Linux GATT Interoperability

**Status:** Source implementation added; Swift tests, Apple compilation and physical Watch tests NOT RUN. See `docs/P007_VALIDATION.md`.
**Branch:** `feat/p007-watchos-foreground-client`.
**Base:** `main` at `3321799745fca35d35bc1051a41e2a8416a66382` (P006/P006A squash merge).
**Reference:** Owned Apple Watch SE 2 + paired iPhone, and an owned Arch Linux/BlueZ host.
**Original roadmap caveat:** Earlier project planning placed Watch work in P007–P018; the full authoritative P001–P060 milestone definitions are unavailable. This P007 is a **provisional Watch-track implementation**, not an assertion about the original P007 spec. P001 production adapter/Qt6/QML Settings framework remain outstanding.

## Goal and decisive engineering deliverable

Create a **real foreground watchOS SwiftUI + CoreBluetooth central client source implementation** for the existing Linux P005/P006 GATT server, with independently testable pure Swift wire codec and explicit non-authorizing UI. It must discover the exact test service, connect, discover five characteristics, receive the 25-byte synthetic challenge, perform a bounded framed synthetic exchange, send Complete or Cancel, read the 44-byte/11-byte response, verify session/length/SHA-256, and reset safely on errors or disconnect. No third-party automatic Apple Watch-to-Linux unlock is available from this work.

Do not write only research documents or a nonfunctional sketch. Source deliverables and tests are expected even if the execution environment is Linux with no Xcode; never claim watchOS compilation or physical BLE success without running them.

## Existing Linux contract (do not casually break)

Service `8e6f0001-6f4e-4c4f-9b4f-4f70656e436f`, characteristics:
- VERSION `8e6f0002-6f4e-4c4f-9b4f-4f70656e436f`: read one byte `01`
- CHALLENGE `8e6f0003-6f4e-4c4f-9b4f-4f70656e436f`: read 25 bytes `version:u8 || session_id:[8] || synthetic_challenge:[16]`
- EXCHANGE `8e6f0004-6f4e-4c4f-9b4f-4f70656e436f`: write with response; 15-byte frame header, `OC || version || session_id[8] || seq:u8 || flags:u8 || total_length:u16-LE || nonempty bytes`
- RESPONSE `8e6f0005-6f4e-4c4f-9b4f-4f70656e436f`: read; success/cancel 44 bytes `version || status || session_id || length:u16-LE || SHA256(payload)`; error is 11 bytes `version || 0x80 || session_id || error_code`
- CONTROL `8e6f0006-6f4e-4c4f-9b4f-4f70656e436f`: write with response; `OC || version || session_id[8] || command:u8` where 1=Complete, 2=Cancel

All multibyte integers are little-endian. Max message 1024 bytes, sequence 0..255, partial write timeout 10 s, response TTL 10 s, server idle 30 s. At ATT MTU 23, max write attribute data 20 bytes, hence max frame body 5 bytes. Start with a conservative **20-byte total write** per frame, and use `CBPeripheral.maximumWriteValueLength(for: .withResponse)` only to reject an insufficient API limit, not to infer ATT MTU.

`CBPeripheral.readValue(for:)` has no explicit offset argument. CoreBluetooth's handling of long 25- and 44-byte reads must be **verified on Apple hardware**, and the app must fail closed on truncated values. Do not synthesize unavailable read fragments or claim a successful read from a simulator.

P006A guarantees reported disconnect signal handling *when received before requests on the same D-Bus connection*; it does NOT guarantee every RF disconnect is reported in time. Watch reconnect must discard all local session data and demand a genuinely new server session; on uncertainty abort/restart the experiment.

## Workstream 1 — Native watchOS client

Create a clearly isolated `experiments/watchos/` app source tree and protocol Swift package containing:
- Small SwiftUI foreground app with Start/Stop and explicit Connect/Disconnect; display research-only state, selected Linux host/peripheral, protocol version, synthetic exchange verdict and errors. No Approve Unlock or identity/security claims.
- `CBCentralManager` delegate with retained lifecycle; scan only for experiment UUID after user action; bounded scan, connect and service discovery; limit to one chosen peripheral; handle off, unavailable, unauthorized, timeouts and app suspension.
- `CBPeripheralDelegate` for characteristic discovery and read/write flow, verify UUIDs and read/write properties, serialize writes with `.withResponse`, wait for `didWriteValueFor`, and limit in-flight operations.
- Parse exact challenge and session, encode frames at safe 20-byte total length with 5-byte payload per chunk, use correct START/END and LE length, then Complete; verify 44-byte synthetic response session, length and SHA256 digest. Support user-requested Cancel.
- Handle errors, disconnect/reconnect, stale callbacks, app foreground/background transitions and timeout using a single clear state machine and session/operation generation guard.
- Zero reliance on private Apple APIs, background listener claims, Apple Auto Unlock data, IRKs, Apple Account secrets, production signing or Linux account authorization.

Swift must use APIs confirmed to be available for the actual intended watchOS deployment. Where an SDK/compiler is unavailable, include the source and project/build instructions but mark it **NOT COMPILED** / **NOT RUN**, not PASS. If an Xcode project cannot be reliably generated/validated, avoid fabricated Xcode metadata and provide an accurate minimal target setup procedure with included compilable source candidates and testable portable Swift package if possible.

## Workstream 2 — Pure wire codec and cross-language fixtures

Keep BLE-facing code separate from the pure Swift protocol module. Implement type-safe parsing/encoding and SHA256 verification with strong bounds. Create stable fixture vectors derived from the Rust `gatt_wire` contract, including:
- v1 25-byte challenge, five-byte-chunk sequence, Complete and Cancel, 44-byte success and 11-byte error;
- byte order and exact UUID/name mapping;
- invalid version, missing bytes, mismatched ID, wrong digest, oversize, too many fragments, invalid flags, timeout and stale-generation handling.
- Document how to regenerate and verify golden vectors in both Rust and Swift. Add Rust fixture consistency tests.
- Where a Swift compiler is available, run `swift test` for pure logic; on Mac with Xcode run `xcodebuild` and simulator/signed device builds as appropriate. Without those tools label each unexecuted category separately.

## Workstream 3 — Linux-side interoperability and host qualification

- Preserve existing Rust CLI, BlueZ experimental `watch-gatt serve` command, bounded registration and P006A disconnect monitor. Do not change current protocol incompatibly.
- Add diagnostics or bounded tests only if necessary to support client interop. Use existing private D-Bus integration tests for a synthetic central, including a 25-byte challenge and 44-byte response across min MTU read scenarios and cancellation.
- Provide a reproducible real-Arch-host plan using `scripts/qualify-bluez-host.sh`, read-only capabilities first, followed by an **explicit operator-approved 1–300 s** GATT server trial, and an owned Watch client only if the app can be signed/installed.
- Report **separately** BlueZ API availability, controller peripheral capability, successful GATT registration, advertisement observable over RF, service discovery, frame exchange/digest and clean teardown. Do not equate mocked D-Bus with real BlueZ.

## Security and scope

This is **synthetic transport only**. The 8-byte session ID + 16-byte challenge is not the P003/P004 signed-approval protocol with a 32-byte nonce. The digest is not a MAC, Watch identity, proof of wearer presence, distance bound, or authorization. BLE discovery, pairing, RSSI, host display name and `Connected` are untrusted. Do not implement watch enrollment, P-256 signing, Secure Enclave, approval for Linux unlock, PAM/polkit/login manager changes, or Apple-native STS Auto Unlock in P007. Keep every research report `authorization: never_evaluated`. Production authentication and security review require future milestones.

## Deliverables

1. `experiments/watchos/` native experimental SwiftUI/CoreBluetooth client source, with setup/build guide and clear status.
2. Portable Swift wire codec and deterministic tests/fixtures, plus parity tests against Rust `gatt_wire`.
3. Accurate BLE state machine, timeout, callback, cancellation, disconnect/reconnect and app lifecycle handling.
4. `docs/research/P007_WATCHOS_CLIENT.md` (target, public APIs and exact implemented steps).
5. `docs/research/P007_WIRE_INTEROP.md` (fixtures/golden vectors, long-read and MTU constraints).
6. `docs/research/P007_HARDWARE_TEST_PLAN.md` (Arch host + Watch SE 2, explicit PASS/FAIL/BLOCKED/NOT RUN).
7. `docs/P007_VALIDATION.md` (head SHA, checks, SDK/hardware status, limitations); update README, architecture, feature matrix, roadmap and threat model as appropriate.

## Acceptance/merge gates

- [ ] Actual source-level foreground watchOS central client exists and follows the exact P006 wire contract.
- [ ] BLE behavior is isolated from pure codec and protocol tests; tests use deterministic vectors.
- [ ] Timeouts, malformed/missing data, wrong session ID, wrong digest, denial, cancel, reconnect and stale async callbacks fail closed.
- [ ] Explicit user start/stop and no automatic background scan, pairing or authorization.
- [ ] Existing P001–P006 regressions pass; Rust fmt, Clippy (warnings denied), tests, build and git diff check are successful.
- [ ] Swift tests and Apple compilation/build results are reported truthfully; absence of Apple SDK is not misrepresented as an Apple build PASS.
- [ ] Owned Linux controller and Watch physical validation statuses are reported separately. Not having hardware in Codex is an accepted blocker for physical validation, not evidence of success.
- [ ] Original Foundation/Qt6/QML and product Bluetooth adapter scope remain explicitly unfinished.
- [ ] PR opened against main on existing branch, hosted CI evaluated on final head, **no auto merge**.

Proceed with implementable work in the current environment. If code needs the watchOS SDK for exact API validation, mark that verification blocked and include a precise Mac build/test handoff; do not misrepresent compiler or hardware capabilities. Do not start P008.
