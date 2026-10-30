# P009 — Signed Watch Research Message Transport Integration (No Unlock)

**Status:** STARTED — branch and specification only. Runtime signed BLE exchange NOT IMPLEMENTED, NOT TESTED on physical devices.
**Branch:** `feat/p009-authenticated-watch-transport-research`
**Base:** `main` `54eaf52612a10001a2f528d0e0fffd9731cf6df6` (P008 squash merge of PR #9).
**Independent unresolved P007B:** PR #8 `research/p007b-watch-bluez-physical-interop` at `8cf11d093d26d5e0c3d20d63b7bc33f47ca03f2f` is DRAFT, unmerged; controller/RF/Watch long-read, signed app installation and real exchanges are NOT RUN.
**Provenance:** Complete original P001–P060 roadmap not recovered; this milestone is a *provisional continuation* of the Watch research track. Production Bluetooth adapter and Qt6/QML Settings remain missing.

## Goal

Implement the first **end-to-end research integration** between P008's separately verified P-256 signed request/response and a non-authorizing Watch-facing transport, with deterministic Linux/Swift cross-language integration and actual Apple SDK compile. Specifically close the gap that P007 GATT v1 cannot carry a Linux-originated authenticated request to Watch and a signed Watch response back to Linux. This is **NOT an implementation of Apple's Auto Unlock**. Successful signature validation only means the configured *research key* signed a message. No Linux account/login/privilege action is allowed.

## Required architecture and invariants

1. Preserve P005–P007 GATT **v1** as the existing synthetic exchange: service/characteristics, 25-byte challenge, 20-byte writes, 44-byte SHA-256 result and v1 command semantics unchanged. Do not tunnel an auth decision into v1 synthetic ACK or let v1 session IDs become authenticated identity.
2. Add an **explicit, separate, versioned opt-in research transport** (`OC-WATCH-AUTH-TRANSPORT/1`, perhaps a new GATT service/characteristics with independently allocated experimental UUIDs). No silent fallback/downgrade. Coordinate/propose the exact wire topology before implementing and record it in documentation. The host must be able to publish a **signed host request** as bounded framed data and accept an actual **signed Watch response** as bounded framed data. Both require strict completion semantics, ordered/acknowledged chunks, end-to-end payload length checks, per-client ownership, and generation/expiry boundaries.
3. P008 signed payload format is already canonical and signed: P-256 SHA-256, 65-byte public keys, raw 64-byte signatures, 32-byte nonce, 16-byte session, 60-second maximum lifetime, exact action `research_confirmation`. The signed transport must carry canonical bytes and 64-byte detached signature intact. Max canonical payload 1024 bytes, max signed envelope 1088 bytes; **do not reuse P007's 1024-byte / 256-frame assembler** if that cannot carry 1088 bytes. Introduce separately bounded framing and packet/frame limits with overflow/DoS tests. If using 20-byte ATT writes, 15-byte headers leave 5-byte chunks and 1088 bytes takes up to 218 frames; verify the mathematics and actual protocol field capacity. Long ATT reads on Watch are UNKNOWN: do not assume CoreBluetooth will return 1088 bytes in one `readValue`.
4. Choose a **CoreBluetooth-public-API compatible** large-message transfer pattern. Prefer a small bounded fixed-size read chunk plus explicit offset/index or acknowledgement **in protocol data** (not imaginary CoreBluetooth read-offset API); progress only after ordered acknowledgements. Handle duplicated reads, retries, BLE disconnects, owner changes, partial frames, peer interleaving, cancellation and timeouts deterministically; prevent an untrusted peer from injecting a completion that advances a different client's transfer. Require explicit user initiation on foreground Watch; background never auto-approves.
5. Use P008 **independently pinned** host public key on Watch before showing any consent UI and independently pinned Watch public key on Linux before accepting a response. No endpoint-provided key becomes trust. The P008 test-only enrollment is **in-memory** with no crash-safe replay store or production trust ceremony. P009 may use controlled in-memory test provisioning/injected research keys for tests only; **never embed fixed test private signing keys or publishable production secrets in a shipped Watch app**, and never accept BLE-advertised keys as authenticated enrollment. If live trust/key provisioning is unavailable, report a strictly blocked live approval flow rather than bypassing verification.
6. Integrate `ResearchConsentView` into a gated, user-initiated foreground research flow **only after** the signed host request is fully reassembled, canonically parsed, signature-verified against pinned host enrollment, bound to the selected session/account and replay checked. Approval must require explicit tap and available LocalAuthentication; manual rejection may be signed if supported. Missing/failed/cancelled LA must not generate an approval. Strictly isolate research-consent signing from the synthetic GATT UI.
7. Linux verifies the response signature, canonical transcript digest, UID/host/Watch/session/action, expiry, nonce and independently enrolled verification key. Return only a typed `research_confirmation` verdict and `authorization: never_evaluated`. A signed denial is a denial, not approval; never change PAM/polkit/login-manager/session state.
8. No trusted device identity, secure proximity, certified Watch identity, verified wearer presence, or physical BLE success can be inferred. Relay attacks remain possible. P007B remains separate and unmerged; this milestone must not depend on its diagnostics unless deliberately reconciled by a separate reviewed PR.

## Implementation tracks

### A. Portable protocol and orchestration
- Separate `watch_auth_transport` Rust module and pure Swift counterpart with versioning, bounded typed envelopes, index/offset/ack state machine and deterministic fixtures.
- Use P008 codec/verifiers as-is where possible. Explicit failure before any consent on malformed/unknown host key, wrong action, expired request, wrong UID, stale session, duplicate frames or downgrade.
- Tests for whole-message direction Linux signed request → Swift verify → simulated explicit consent → Swift sign response → Rust verify using independent enrolled test keys. Tests must exercise **actual serialized transport envelopes and framing**, not call the crypto verifier directly as a substitute.
- Test adversarial transport: mixed clients, replay, stale callback/generation, truncation, partial reads, duplicate/out-of-order index, fragment cap, oversized envelope, invalid signature, signed denial, cancellation, owner change, timeouts and cleanup.
- Explicit test-only in-memory trust, no durable production data or privilege.

### B. Linux experimental transport
- Add opt-in bounded research host transport path (new experimental command/flag/service), isolated from v1 server; do not expose any background always-on daemon or privileged hook.
- Read-only host preflight before registration. No automatic controller power, pairing, D-Bus policy change, sudo or adapter reconfiguration. Register only owned D-Bus resources, cleanup on expiration/signal/error. Reuse P006A ordered disconnect signal handling and avoid regression.
- Denial/failures must be bounded and classified, avoid public errors containing session IDs/keys, no full raw protocol logs. If hardware is missing, exercise private D-Bus mock where feasible and report real RF as NOT RUN.

### C. Watch foreground client
- Separate opt-in `Authenticated Research` flow from P007 synthetic transport, with SwiftUI state explaining *Research confirmation — does not unlock Linux*.
- BLE connection and transfer modeled with explicit operation generation ownership, no app suspension ghost work, no background scan/approval, no trust-on-first-use.
- Make UI contingent on actual independently pinned host record and research signing key injected through a safe controlled research setup. If such setup cannot be provided safely, expose unsupported/blocked research state and still implement compile/testable transport plumbing without fake success.
- No UI can claim `verified genuine Apple Watch` merely from signatures.

### D. Tests, CI, review docs
- Keep P001–P008 Rust and Swift suites passing and watchOS unsigned compile in hosted Apple CI; extend Rust↔Swift vector parity to framed requests/responses. Ensure tests check **both** signature direction and transport state invariants.
- Required docs: `docs/research/P009_AUTH_TRANSPORT_DESIGN.md`, `docs/research/P009_TRUST_AND_CONSENT_INTEGRATION.md`, `docs/research/P009_TRANSPORT_THREAT_REVIEW.md`, `docs/research/P009_PHYSICAL_VALIDATION_PLAN.md`, `docs/P009_VALIDATION.md`, and targeted README/ROADMAP/FEATURE_MATRIX/ARCHITECTURE/THREAT_MODEL updates.
- CI stage status must distinguish Rust/Swift crypto/transport, unsigned watchOS compile, signed Watch app installation, physical Linux GATT and on-air RF, actual Watch long-read, LA on device, Secure Enclave and end-to-end BLE signature exchange. Mark unavailable stages NOT RUN or BLOCKED, not PASS.

## Acceptance gates

- [ ] New versioned auth transport exists and does NOT break existing synthetic GATT v1.
- [ ] Linux can produce **host→Watch** signed request envelopes; Watch can return **Watch→Linux** signed responses, with transport framing and bounds independent of the v1 limits.
- [ ] Real bidirectional end-to-end simulation or private D-Bus integration verifies signed messages with independently pinned test records; exact serialized wire bytes tested across Rust and Swift.
- [ ] Watch consent can appear only after verified host key; a manual confirmation with supported LocalAuthentication is required for signing approval. No preloaded fixed production key or silent auto approval.
- [ ] Denial, mismatch, replay, expiry, bad chunk ordering, disconnected/reconnected sessions and mixed clients fail closed; no authorization transitions.
- [ ] Existing P008 P-256/Swift cryptographic conformance PASS, Rust/Python gates PASS and watchOS unsigned SDK build PASS.
- [ ] Hardware missing or secure provisioning absent is clearly BLOCKED; no physical compatibility claim. Keep PR Draft until applicable acceptance gates are passed, never auto-merge and do not start P010.
- [ ] `authorization: never_evaluated` throughout, never `linux_unlocked`.

## Security exclusions

No PAM, polkit, login manager, lock-screen/session unlock, Apple native STS, Apple Account credential access, automatically trusted BLE pairing, RSSI/latency distance proof, hidden system Bluetooth settings changes, global BlueZ reconfiguration, private Apple APIs, unreviewed persistent keys, unattended Watch approval or claim of genuine Watch/secure proximity. All signing/enrollment/replay data is explicitly experimental and non-production.
