# P004 — Foreground Watch ↔ Linux BLE GATT feasibility spike

**Status:** STARTED / DESIGN & EVIDENCE PLAN ONLY; no hardware validation, Apple application, radio integration, or unlock implementation.
**Base:** `main` at `1d5d3f5a19f81727a5d3deef61d24de5f82d4ed4` (P003 squash merge).
**Working branch:** `research/p004-watch-ble-gatt-spike`.
**Reference hardware:** owned Apple Watch SE (2nd generation), paired iPhone, Arch Linux/BlueZ development host.

## Roadmap scope caveat

The authoritative original `OpenContinuity-Roadmap-P001-P060.md` is not checked into this repository and its **exact P004 title/acceptance criteria have not been recovered**. Earlier planning categorized P001–P006 as Foundation/Settings and P007–P018 as Watch Auto Unlock, whereas the actual P002 and P003 implementations have followed Watch research. This document defines a **provisional engineering continuation**, not a claim to implement the original P004 specification. Reconcile this divergence before declaring P004 aligned with the authoritative roadmap.

Carry-over gaps: the P001-identified Bluetooth/BlueZ adapter scaffolding and Qt6/QML Settings framework are not implemented by P001–P003. A scoped, isolated BlueZ research transport in P004 must not be misreported as full production adapter scaffolding. The Settings framework remains open.

## Rationale and goal

P003 established a **conditional-go research path** for a *foreground* Watch application acting as Bluetooth LE central and Linux BlueZ acting as GATT peripheral. It did not implement a real Watch application, BlueZ service, hardware testing, or enrollment ceremony. This milestone evaluates that topology with opt-in, non-authorizing experiments and clarifies exact public API and device requirements.

A successful link-level experiment is **not Apple Auto Unlock interoperability** and must not imply Linux session authorization. No Watch-proximity, RSSI, watch-pairing, iPhone-presence, or mock response may unlock a session.

## Research and implementation tracks

### 1. Linux BlueZ experimental GATT endpoint

- Inspect the current local BlueZ version and the upstream `org.bluez.GattManager1`, `org.bluez.LEAdvertisingManager1`, `org.bluez.GattService1`, and characteristic D-Bus APIs. Ground API names and supported operations in actual BlueZ documentation.
- Design a Linux-side GATT peripheral service only when the selected controller and BlueZ configuration advertise the required capabilities. **Do not assume every controller supports peripheral mode or advertising.**
- Keep this adapter strictly opt-in, read-only or tightly bounded synthetic challenge transport, with no privileged permanent service or automatic startup.
- Define random/private-service identifiers appropriate to experimentation and explicitly disclose discovery and radio-privacy implications.
- Ensure start/stop, SIGINT, timeout, adapter detach, failed registration, and cleanup are idempotent and avoid modifying unrelated BlueZ clients.
- Do not introduce system login, PAM, polkit, password interception, privilege escalation, keychain/IRK extraction, or automatic unlock.

### 2. watchOS companion feasibility

- Verify an app's `CBCentralManager` foreground behavior and GATT access on **Watch SE 2** using a real watchOS SDK or actual device; do not extrapolate from iOS.
- If Xcode, signing and hardware are available, add an **isolated experimental** watchOS companion target with clear consent and permissions. Define the smallest opt-in connection → challenge receive → explicit on-screen approve/deny → response flow.
- If Apple tooling is unavailable, document the Swift interface sketch and exact manual build/test procedure; mark Apple builds and physical behavior **NOT RUN**.
- Explicitly measure app foreground/background, screen-off, off-wrist and locked states separately. Do not infer continuous background reachability from one foreground success.

### 3. Authentication model and enrollment boundary

- Correct the P003 simulation gap: a verifier must obtain the expected enrolled Watch public key from a **separately constructed enrollment record**, never from the untrusted endpoint currently under test.
- Evaluate realistic watchOS key storage and signing capabilities, including algorithm compatibility. The P003 Ed25519 mock is **not proof** of Secure Enclave Ed25519 support on watchOS; determine whether a supported P-256/ECDSA scheme and stable public-key format is required.
- Bind every experimental challenge/response to a domain-separated protocol version, host identity, local user, session, action, nonce, expiry and enrolled Watch identity.
- Define **host-to-Watch authentication** and the trust establishment ceremony; a one-way signed Watch response does not authenticate the origin of a request.
- Document exactly what a foreground user approval proves, and what it does **not** prove: hardware provenance, distance, wearer identity, or immunity to relay.
- No enrollment secret must be copied from Apple's Auto Unlock, Apple Account, BLE IRK, macOS Keychain or system pairing records.

### 4. Threat model and test discipline

- Model spoofed GATT servers, unauthorized hosts prompting the watch, malicious relay, cloned test endpoints, replay, protocol downgrade, message fragmentation, oversized input, lost links, device suspension, stale response and compromised host.
- Use deterministic unit/mock tests and optional loopback/GATT-host tests. Separate all host-only validation from physical Watch tests.
- Version and bound all messages; reject unknown fields and unexpected states. Only an experimental non-authorizing status may be returned; `authorization` remains `never_evaluated`.
- Treat successful BLE transport connection as a transport event, not authorization.

## Deliverables for Codex

1. `docs/research/P004_WATCH_BLE_GATT_SPIKE.md` (this scope, evidence and conclusions).
2. `docs/research/P004_BLUEZ_CAPABILITY_MATRIX.md` with Linux adapter/controller limitations and API citations.
3. `docs/research/P004_WATCH_APP_FEASIBILITY.md` with watchOS APIs, build prerequisites, and Watch SE 2 test outcomes or explicit NOT RUN.
4. `docs/research/P004_ENROLLMENT_AND_CRYPTO.md` separating mock keys from secure enrolled identities and documenting host authentication.
5. `docs/research/P004_HARDWARE_VALIDATION.md` with exact controller, firmware, BlueZ, watchOS/iOS versions and reproducible test evidence or NOT RUN.
6. `docs/P004_VALIDATION.md` with actual branch/head SHA, change list, local test outputs, hosted CI status and unresolved blockers.
7. A scoped Rust experimental BlueZ boundary **only if supported by available toolchain and validated API evidence**. Otherwise deliver design/contracts and mock-only tests rather than fabricated radio results.
8. Optional isolated Swift companion experiment only when Xcode/watchOS tooling exists; no claims of Apple build or device success without execution.

## Acceptance criteria

- [ ] Existing P001–P003 tests and behavior preserved, including `opencontinuity`, `opencontinuityd`, `doctor`, and mock research commands.
- [ ] BlueZ GATT peripheral/LE advertisement capability and controller requirements documented, with exact references.
- [ ] Watch SE 2 foreground central capability and Apple API availability evidenced; unavailable device validation clearly marked.
- [ ] A separate enrolled-verification-key design corrects the P003 derived-key-from-untrusted-mock limitation; no production authorization.
- [ ] Host authenticity, user consent, signer algorithm compatibility, replay, response expiry and malformed-message behavior assessed.
- [ ] If new code is added, formatting, Clippy, test, build and targeted negative tests pass locally and on hosted CI; CI pending is not PASS.
- [ ] No permanent privileged services, PAM/login-manager modifications or auto-unlock path.
- [ ] Original P004 roadmap scope and P001 BlueZ/Qt6 gaps explicitly tracked without silently claiming completion.
- [ ] P004 pull request against `main` is created for review. **Do not merge automatically.**

## Boundaries and next decision

Only after trustworthy physical evidence of Watch foreground connectivity and key operations may a subsequent milestone consider a secure, explicitly approved user-flow prototype. Hands-free/background unlock remains NO-GO absent a defensible authenticated authorization and proximity/user-presence model. Apple-native STS Auto Unlock remains blocked on documented public interfaces. AirDrop and AirPods Pro 3 are not in P004 scope.
