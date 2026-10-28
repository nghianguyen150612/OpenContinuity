# P006 — BlueZ host qualification, GATT session hardening and Watch interoperability readiness

**Status:** P006A disconnect invalidation implemented and mock tested; real BlueZ and Watch hardware NOT RUN. See [validation](../P006_VALIDATION.md).
**Branch:** `feat/p006-bluez-hardware-qualification`
**Base:** `main` at `a4e3de698451912980e7359dabb4bd040290b644` (P005 squash merge).

## Roadmap provenance — important

The original P001–P060 roadmap is not available in the repository or recovered verbatim from planning history. The recovered phase-level structure groups P001–P006 under **Foundation + Settings Framework**, with Apple Watch work planned for P007–P018. P002–P005 proceeded under an explicitly provisional Watch/BLE track. This P006 scope is a continuation of that *actual implementation track*, **not the verified original P006 acceptance contract**. Do not mark original foundation/Qt6/QML deliverables complete. Record scope divergence and keep the missing production Bluetooth adapter and Qt6/QML Settings framework visible.

## Goal

Qualify P005's genuine but non-authorizing Linux BlueZ GATT server against a real, controlled BlueZ host when hardware is available; correct identified session/response lifecycle risks; produce reproducible Watch SE 2 foreground client interoperability prerequisites without claiming unperformed tests.

The current code has synthetic GATT service registration, advertisement, version/challenge/exchange/response/control characteristics, 1024-byte framed messages, zbus D-Bus object server and private mock D-Bus tests. P005 had no real controller, BlueZ daemon or Watch hardware in its build environment. This milestone must **not** claim Bluetooth interoperability just because mock tests or GitHub CI pass.

## Workstream A — transport lifecycle and hardening (required)

1. Inspect `apps/opencontinuity-cli/src/gatt.rs` and `crates/opencontinuity-core/src/gatt_wire.rs` thoroughly.
2. Fix the P005 `ClientSession::rotate()` concern: response from an old session must never be misread as an authenticated/current-session result. Define a precise, tested state machine for result retrieval, session rotation and abandonment. In particular, `Complete`/Cancel ACK retrieval must be possible without a stale response being associated with a newly issued challenge; define response retention/consumption and TTL.
3. Check and test all shutdown exits, particularly D-Bus loss and state-polling errors. Release only current-process advertisement/application and record cleanup outcomes accurately even on errors.
4. Bound client-state count, per-client buffers, idle lifetime, bad-frame handling and resource exhaustion. Avoid unbounded memory growth from arbitrary connected-device paths.
5. Test multi-client isolation, disconnect/reconnect, ambiguous device identity, missing GATT write options, MTU values, offsets, read fragmentation, duplicate/reordered frames, 1024-byte cap and sequence-number overflow.
6. Keep `--experimental`, explicit adapter selection and bounded duration mandatory. No permanent service, pairing, auto-start, Bluetooth system configuration mutation, PAM or session unlocking.

## Workstream B — real Linux BlueZ qualification (when available)

1. Add a repeatable, **read-only-first** host capability qualification command or script and a separately opt-in, bounded GATT trial. Determine distribution/kernel/BlueZ, controller firmware and supported roles with verifiable commands.
2. Check `GattManager1`, `LEAdvertisingManager1`, manager capacity and actual controller capability; distinguish API availability from successful registration and from on-air advertisement.
3. Test app registration, advertisement, service visibility to a second **owned** BLE device/scanner, cleanup on Ctrl-C/SIGTERM/timeout/power loss. Do not change global adapters, security policy or other clients.
4. If the Codex runtime has no system bus/BlueZ or radio, mark physical tests NOT RUN, do not invent evidence, and produce **exact host-run qualification commands** for an Arch Linux machine plus expected report fields and artifact sanitization.
5. Avoid treating passive radio observations, advertisements, RSSI or BlueZ pairing as Watch authentication.

## Workstream C — Watch SE 2 interoperability readiness

1. Define the exact watchOS SwiftUI/CoreBluetooth foreground-central client contract for P005's six UUIDs (one service + five characteristics), ATT MTU handling, read offsets, frame fragmentation, Complete/Cancel and session lifecycle.
2. Optionally add isolated watchOS source **only if SDK/API availability has been checked**. Label source-only, compiled, simulator and physical Watch results separately. No claims of Xcode/watchOS build without macOS/Xcode.
3. Reconcile P005's synthetic 16-byte challenge + 8-byte session ID with P003/P004's 32-byte nonce signed-approval mock: they are distinct, non-authorizing contracts. Do not pass synthetic ACKs into the signing verifier.
4. Analyze secure Watch-held keys (P-256 availability), user-initiated approval, Linux host authentication, signed enrollment, timeout and relay limitations as future gates. Do not implement a production unlock path.

## Deliverables

- Corrected lifecycle/state machine and regression tests, including test for stale response across session rotation.
- Better resource limits and error/cleanup reporting, backed by private D-Bus mocks.
- Deterministic host capability/qualification harness; optional real BlueZ results only with actual hardware evidence.
- `docs/research/P006_BLUEZ_HOST_QUALIFICATION.md`
- `docs/research/P006_GATT_SESSION_HARDENING.md`
- `docs/research/P006_WATCH_CLIENT_CONTRACT.md`
- `docs/P006_VALIDATION.md`
- Changes to README/architecture/roadmap/feature matrix/security documents where warranted.

## Acceptance and Git policy

- [x] Source inspection and P005 regression complete; independently evidenced fixes for stale-response/session lifecycle and cleanup/error paths.
- [x] Memory and frame bounds, multiple clients, ordered disconnect/reconnect, timeout and protocol faults are mock tested.
- [x] Read-only-first real host qualification procedure, with hardware PASS only if performed.
- [x] Watch foreground GATT client interoperability contract and known platform limits documented.
- [x] `authorization: never_evaluated` retained in all research paths; no PAM/login-manager integration.
- [x] P001–P005 regressions preserved; `cargo fmt --all -- --check`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo test --workspace --all-targets`, `cargo build --workspace`, `git diff --check` pass.
- [ ] Hosted CI assessed on final HEAD, PR opened for review **without auto-merge**.
- [x] Original roadmap's Foundation + Settings scope and missing Qt6/QML/product adapter tracked as not yet satisfied.

**No automatic P007 work.** P006 closes and qualifies the existing non-authorizing BLE transport prototype; any future real Watch identity/authentication requires separately reviewed hardware and security evidence.
