# P017 — Arch BlueZ Physical Qualification & Foreground Watch Transport Trial (Provisional)

**Status:** INITIALIZED — branch/scope only. No P017 implementation, Draft PR, or CI claim.
**Branch:** `feat/p017-arch-ble-physical-qualification`
**Base main:** `74dc662762317e688d216c702c6e0c9145c90f9a` (P016 PR #17 squash merge).
**Roadmap caveat:** The canonical P001–P060 planning source remains unavailable. This P017 is explicitly **provisional**, not a reconstruction of the missing original milestone.

## Mission and split evidence gates

Bring P012–P016 Linux BlueZ inventory, bounded discovery, JSONL stream, QProcess Settings and P005/P009 experimental GATT server **to user-owned physical qualification readiness on Arch**. Build safe and executable tooling and testable artifacts in CI, while distinguishing *software readiness* from *real hardware evidence*. If the tool-running environment cannot access the actual Arch host and the approved radio equipment, mark **NOT RUN/BLOCKED**; do not invent RF trials or request privileged remote access.

Two independent hardware tracks:
- **A — Arch BlueZ discovery + Qt GUI**: a deliberately user-triggered 10 s LE scan of a controlled third-party BLE peripheral, independently observed RF activity, session ownership and teardown, Settings UI behavior on the real Wayland/Hyprland compositor. The P012 inventory must stay read-only; scan is never implicit.
- **B — Optional Apple Watch SE 2 foreground research**: existing watchOS CoreBluetooth **central** app actively scans/connects to an intentionally advertised Linux GATT **peripheral/server** for P006 synthetic or P009 signed test-only transport. This requires a physically installed, signed app and owned watch/host. It is separate from A; do not pretend Watch itself is a BLE advertising peripheral.
- **C — Genuine Apple Auto Unlock** remains **NO-GO** independently. No discovery/GATT success is proof of Apple-native STS, secure proximity or authorization.

Authoritative constraints:
- Apple states advertising services via `CBPeripheralManager` is unavailable in watchOS: https://developer.apple.com/documentation/corebluetooth/cbperipheralmanager
- watchOS app's documented foreground Bluetooth central capability and existing `experiments/watchos/WatchBluetoothManager.swift`: https://developer.apple.com/documentation/corebluetooth/cbcentralmanager
- Existing Apple guidance describes constrained background execution; P017 tests **foreground explicit action only**, never assumes reliable background scan or persistent connection: https://developer.apple.com/documentation/WatchKit/using-background-tasks
- BlueZ per-client discovery sessions and filter-merging semantics: https://github.com/bluez/bluez/blob/master/doc/org.bluez.Adapter.rst

## Existing source and mandatory review

- P016 `apps/opencontinuity-cli/src/device_discovery.rs` plus production-path `tests/discovery_bluez_mock.rs`; BLE scan starts only with explicit `devices discover`, pins BlueZ unique owner, validates event provenance and owns exactly one Start/at most one matching Stop.
- P016 zbus fix releases bounded signal subscriptions before Stop/filter-clear replies. Preserve this regression and the 514-record/2 s writer-progress invariants.
- P015 `scripts/p015_arch_preflight.py` is read-only.
- P016 `scripts/p016_evidence.py` validates local preflight/JSONL data, publishes only safe fixed-vocabulary summary, and **never attests physical RF merely from a JSONL transcript**.
- P005/P006/P009 research server and `experiments/watchos/` synthetic and authenticated research client; examine exact CLI commands and current app sources before writing a runbook. Do not invent flags, service UUIDs, APIs or build commands.
- P010/P016 runbooks, validation, threat model and privacy rules. Review existing GATT radio-guard exclusion: do **not** use discovery client and experimental GATT server simultaneously on an adapter where the project radio guard forbids that. Stage trials sequentially, or document clearly separate adapters if truly supported and individually evidenced.
- App software build uses unsigned watchOS artifact; physical installation needs a real signing team/provisioning and an Apple-supported paired Watch path.

## Deliverables 1 — Host-side qualification kit

Develop a **new** `scripts/p017_arch_qualify.py` or equivalent, with tests and an interface documented in help. It should:
- Default to **read-only** preflight only, reuse `p015_arch_preflight.py`, report kernel/Arch version, BlueZ binary version, system D-Bus availability, adapter count/index, powered state, LE filter capability, CLI version and Settings presence; suppress sensitive data and raw paths.
- Provide distinct explicit modes such as `--check`, `--active-discovery --i-consent-to-radio-scan`, and `--validate-existing --input <local file>` (exact flags may be finalized in implementation docs). Never scan as an incidental side effect of a read-only check; refuse absent consent, duration outside 1–30 s, unowned host conditions or missing CLI.
- Active trial may execute ONLY the existing bounded `opencontinuity devices discover --seconds N --events-jsonl` with trusted executable resolution and argv list (no shell). Inherited CLI runtime is responsible for owned Stop/filter/connection cleanup; tool must use a finite overall deadline, graceful SIGTERM and bounded SIGKILL fallback on failure.
- Local transcripts (if explicitly requested) must use user-only 0600 files, sanitized folder, safe creation avoiding symlink following, and no telemetry/upload. The pre-existing JSONL stream has display names that may reveal personal data; never publish/upload raw files or identifiers. Prefer piping straight to the P016 validator and publishing only its redacted summary. Avoid screenshots with nearby device names.
- Capture local CLI exit code, valid terminal presence and cleanup report independently, without modifying its original JSON schema. Track separately `execution_started`, `observed_events`, `reported_cleanup`, `exit_verified` and `independent_rf`.
- A failure, termination or missing terminal must not become a PASS; surfaced errors should not print raw MAC, D-Bus path, name, IP, username or secret material.
- No sudo/privilege escalation, adapter power/Pairable/Discoverable toggles, D-Bus policy edits, pairing or `Trust` changes in normal mode. If existing P009 GATT server registration needs permissions, document as environment blocker, not auto-patch host policies.

## Deliverables 2 — RF/cleanup corroboration and data provenance

Provide a human-operated hardware worksheet plus a machine-validated `P017` evidence envelope that never fabricates measurements:
- Stage S0: hardware/firmware/BlueZ/CLI readiness (read-only)
- S1: explicitly authorized bounded discovery launched
- S2: streamed events and terminal/exit status verified
- S3: user-owned controlled peripheral independently corroborated by a **separate receiver** and controlled advertising intervals; no proof from BlueZ events alone
- S4: native Qt/Wayland/Hyprland real launch + Scan/Cancel/navigation/window-close; distinguish offscreen CI
- S5: exact owned BlueZ cleanup evidence with bounded diagnostic observer and failure cases, without publishing other-client identifiers; distinguish *Stop attempted* vs *Stop confirmed* vs BlueZ shared Discovering state
- W0–Wn: signed watchOS install, foreground consent, experimental Linux advertisement discoverability, P006 synthetic exchange, P009 test-only signed exchange, LocalAuthentication result (each independent and optional).
- Keep statuses `pass`, `fail`, `blocked`, `not_run`; provenance `mock_ci`, `host_preflight`, `operator_hardware`, `independent_observer`. Prohibit promotion to physical PASS without attestation and raw private evidence reviewed by human operator. Raw MAC/object-path/RSSI/service payload, address or keys must never be in public exported data.
- Never treat self-asserted or unverified JSON as trusted hardware attestation. Summaries may record `operator_reported` separately; must not automatically certify.
- A successful controlled third-party BLE peripheral trial cannot qualify Apple Watch SE 2. Watch P010 remains NO-GO until specific separate gates actually pass.

## Deliverables 3 — Watch foreground research readiness (no forged device support)

Inspect existing P006/P009 Linux server and `experiments/watchos` before designing anything new. Produce a compatibility matrix of Linux-server vs Watch-central API availability, foreground scan, advertisement, GATT service discovery, read/write framing, manual enrollment, explicit consent, cancellation, signed build and device diagnostics. Reuse existing source instead of inventing a second protocol.
- Fix only observable source/configuration/test gaps needed for manually reproducible trials, with regression tests. Do not fabricate a signed Watch application or a hardware run from Xcode's unsigned build artifact.
- The Linux host deliberately advertises the P006/P009 experimental service as a temporary, opt-in peripheral; watchOS app performs foreground scanning as a central, under user control. Apple's Watch `CBPeripheralManager` cannot advertise services; no Watch-as-peripheral plan.
- P009 uses temporary test-only key pins and signed research messages, not production Apple device identity. No automatic enrollment, Apple Account, native Apple Auto Unlock, PAM/polkit/login manager, root integration or user-session unlock.
- If physical Apple Watch SE 2, Xcode signing or Linux GATT advertisement isn't accessible, record W stages `NOT RUN/BLOCKED` with exact prerequisites; do not automatically merge watch research gates into discovery gates.

## Deliverables 4 — Testing and hosted CI

Implement deterministic software tests:
- Python unit tests: preflight-only cannot activate radio, invalid duration/consent/CLI path, process timeout/cancellation/abnormal exit, transcript bounds and absence, privacy redaction, safe file creation, stage provenance and no automatic physical promotion.
- Rust private BlueZ bus: preserve P016 cleanup regression and existing 29 cases, plus any tests needed for host runner interaction and owner isolation; no CI dependency on an actual adapter.
- Qt 6.4.2 tests: real QML scan interaction via fixture and existing strict parser; do not weaken P016 adversarial tests. New UI changes only if justified by reproducible issues.
- macOS Swift XCTest, Rust↔Swift signature interop, watchOS SDK compile and unsigned artifact packaging remain unchanged; add compile-only compatibility checks where useful. Swift/Xcode success is not signed installation or Watch SE 2.
- CI: cargo fmt, Clippy -D warnings, cargo test/build, Python unittest, diff check, Ubuntu Qt CMake/Ninja/CTest, macOS Swift/CryptoKit/watchOS, final-head push+PR 6/6 PASS.
- Real Arch host scans, RF corroboration, real Wayland/Hyprland and Watch SE 2 must be **manual, explicit, non-CI** tasks. No scan in CI and no requirement to connect a remote personal computer without user action.

## Docs and delivery

Create:
- `docs/research/P017_ARCH_BLUEZ_PHYSICAL_MATRIX.md`
- `docs/research/P017_SAFE_HOST_QUALIFICATION_TOOL.md`
- `docs/research/P017_RF_AND_CLEANUP_EVIDENCE.md`
- `docs/research/P017_WATCH_FOREGROUND_CENTRAL_TRIAL.md`
- `docs/research/P017_PRIVACY_AND_TRUST_REVIEW.md`
- `docs/P017_VALIDATION.md`

Update README, roadmap, architecture, feature matrix, threat model, Settings and watchOS experimental READMEs as appropriate. Explain at every stage what was run and what remains NOT RUN.

Implement on the existing `feat/p017-arch-ble-physical-qualification` branch, commit and push scoped changes, open **Draft PR** against current `main` and verify final-head CI. Do **not** auto-merge and do not begin P018.

**Hard exclusions:** Native Apple Auto Unlock/STS, device identity/proximity attestation, paired Apple ecosystem impersonation, permanent background scanning, auto pair/connect/trust changes, privileged policy edits, account authentication, Linux PAM/polkit authorization or unlock, telemetry, production key provisioning. Always keep `authorization: never_evaluated`.
