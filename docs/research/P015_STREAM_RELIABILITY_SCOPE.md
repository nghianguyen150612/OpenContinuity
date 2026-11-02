# P015 — BLE Streaming Reliability, Lifecycle Hardening & Arch Qualification Toolkit (Provisional)

**Status:** STARTED — branch and scope only. Source implementation, tests, hosted CI, and P015 PR NOT STARTED.
**Branch:** `feat/p015-ble-stream-reliability-qualification`
**Base `main`:** `04910e018c8cdd1b58b86c552722931d0ac1f964` (P014 PR #15 squash merge).
**Provenance:** The originally requested `OpenContinuity-Roadmap-P001-P060.md` is not available. P015 is an explicitly **provisional** next milestone, not a claim about the unseen original P015.

## Goal

Harden the **actual P014 JSONL discovery event stream** under realistic bursts, slow/blocked stdout, Qt cancellation and race conditions; add **non-destructive, operator-invoked Arch Linux/BlueZ qualification tools** that gather sanitized and provenance-marked evidence. Demonstrate reliability in deterministic software CI. Do not claim radio/hardware success without the user-owned Arch Bluetooth adapter and authorized physical tests.

P014 merged with 6/6 final-head push/PR checks passing, Rust 172, Python 7, Qt Doctor 62 / inventory 27 / discovery 36 / QML 10 and hosted Swift/watchOS. P013/P014 RF trials and P010 Watch SE 2 physical qualification are still NOT RUN/NO-GO. Rust `EventStream::emit()` currently performs `SyncSender::try_send()` to a one-record queue and immediately maps `Full` to `output_blocked`. A healthy writer briefly not scheduled can cause false scan failure on clustered BlueZ events. **This is the primary P015 defect.**

## 1. Diagnose and fix nonblocking backpressure correctly

Inspect `apps/opencontinuity-cli/src/device_discovery.rs` and existing private-bus tests. Preserve the single owned BlueZ scan and the P014 `--events-jsonl` schema v1.

- Replace the **immediate Full → output_blocked** behavior with bounded, cooperative enqueue retry/backpressure that distinguishes transient queue fullness from *no writer progress beyond a real deadline*. Track oldest unacknowledged write/queue age and actual writer acknowledgments. Never silently lose/reorder/update device records or skip sequence numbers.
- The **async discovery/event loop must remain responsive** to requested scan duration, SIGINT/SIGTERM, BlueZ owner loss and bounded cleanup even during output pressure. Do not use unbounded `send()`, blocking stdout on the async BlueZ loop, indefinite join, unbounded channel or retry busy-spin. Choose a safe async-compatible queue or producer future with explicit bounded wait and cancellation; verify the pinned Rust and zbus API versions.
- Preserve ordering: exactly one session event, zero or more device events, exactly one terminal after cleanup *when stdout is deliverable*. Terminal-success must never precede Stop/filter cleanup or be reported merely because the writer accepted bytes. Retain explicit `output_blocked`/`output_closed` failure and Qt 'cleanup uncertain' behavior when a terminal is absent.
- Bound overall scan length 1–30 seconds, setup/cleanup/method limits, events 4096, unique devices 128, device/update lines 512, line <=2 KiB, total <=512 KiB, writer queue and memory. Distinguish *writer progress deadline* from *scan duration*, so a burst cannot restart/extend an expired scan. A permanently unwritable stdout must stop the scan and attempt owned cleanup.
- Explicitly assess the P014 `EventStream` writer-thread shutdown and QProcess race handling: on closed pipe, SIGTERM during a pending event, SIGKILL fallback, queue still full after cleanup, terminal waiting for drain, writer failure between terminal enqueue and flush, and reader that exits early.
- Do not change the P013 `opencontinuity devices discover --seconds 10 --json` schema or P012 `devices list --json`. Do not weaken BlueZ session ownership or the cooperative radio guard.

## 2. Lifecycle and signal hardening

- Review the sequence of `SetDiscoveryFilter`, `StartDiscovery`, signal observation, single owned `StopDiscovery`, filter clearing, unique BlueZ owner pinning and connection close. Preserve exactly one owned Start and at most one owned Stop attempt, including on cancellation and writer errors.
- Test cancellation during setup/filter/start, signal flood, stdout blockage, post-Stop terminal emission and unexpected BlueZ owner changes. Lost Start reply remains uncertain; never send unowned Stop to pretend cleanup succeeded.
- Review Qt6.4 `DeviceDiscoveryProvider` and `DeviceDiscoveryReport` incremental validation, 8 KiB reads, 2 KiB per-line and 512 KiB total caps, QProcess generation fencing, stale model, hidden/navigated page and window-close cancellation. Fix demonstrable races, not speculative rewrites.
- A stream accepted as successful must match its terminal status, cleanup, event and device counts, session ID, sequence and process exit 0. An absent terminal, malformed UTF-8, duplicate JSON fields or QProcess abnormal exit fails closed; never project stale data as a verified nearby device.

## 3. Arch Linux qualification toolkit — real tooling, not fake evidence

Add *operator-invoked*, sanitized, reproducible host checks and optional manual test scripts under `scripts/` and documentation. Keep software tests independent of hardware.

- **Read-only preflight**: report kernel/OS identification, BlueZ availability/version, system D-Bus, adapters enumerated (redacted/snapshot-indexed), powered state, LE filter feature availability, CLI/Qt versions and permission/error classification. Prefer explicit allowlisted `busctl`/D-Bus read methods and Rust/BlueZ SDK methods; no sudo, adapter power toggles, policy writes or automatic radio scan.
- **Optional active real scan**: separate, obvious operator approval step that invokes the **existing** `opencontinuity devices discover --seconds N --events-jsonl`, with a short bounded duration, privacy-safe logs, and an independent optional scanner/known peripheral to corroborate observation. This is *not* part of default preflight or CI.
- Provide a safe `qualified/blocked/not_run/failed` evidence model separating (1) environment readiness, (2) actual LE discovery session/filter/cleanup, (3) streamed partial observations, (4) independently corroborated RF, (5) Gtk/Qt/Wayland/Hyprland window interaction, and (6) Watch SE 2/LocalAuthentication (separate P010, remains NO-GO unless individually evidenced). Do not infer that a BlueZ event proves real radio proximity or device identity.
- Keep hardware output sanitized: never publish device MAC/address, system D-Bus object paths, device serials, IRKs, pairing keys, raw ads, local usernames/home paths, private IPs, secure transcripts, or unapproved identifiers. Names can be identifying; make pseudonymization/redaction part of manual capture. Distinguish locally generated software fixtures from true hardware observations.
- If this execution environment lacks the user's Arch host/controller, **do not request privileged remote access or invent successful hardware results**; record NOT RUN/BLOCKED and provide precise on-host commands/checklist.

## 4. Test plan and CI

Run production-path deterministic stress through a private fake BlueZ D-Bus bus and real spawned CLI where possible:
- A burst of 20–200 legitimate events delivered faster than writer scheduling with a healthy stdout consumer must **not** fail instantly with output_blocked. Preserve ordered, lossless, bounded observations and exact counts.
- Slow but progressing stdout and a reduced-size pipe; a *permanently stalled* stdout must terminate within the documented write deadline, attempt owned Stop/filter cleanup and exit nonzero. Closed stdout must produce output_closed when observable and cleanup must still be attempted.
- Cancellation before Start, during burst/backpressure, after Start, during cleanup, lost Start reply, BlueZ owner replacement, duplicate signals, event/device/output caps, final record drain, writer failure and no extra Stop or Start.
- Test `--json` backward compatibility (schema, exit codes and true P013 semantics), mutual exclusion with `--events-jsonl`, no automatic scan, privacy field exclusion and `authorization: never_evaluated`.
- Qt backend/QML fixture tests exercise live progressive rows before terminal, many rapid events, Cancel, close/navigate-away while output is pending, timeout/SIGKILL, stale/generation guards, invalid terminal, split UTF-8, unknown/duplicate fields, and clear provenance labeling.
- Preserve every prior regression gate: Rust fmt, Clippy `-D warnings`, workspace tests/build, Python suite, Qt 6.4 CMake/Ninja/CTest with real QML offscreen smoke, hosted Swift tests/Rust→Swift→Rust interop, unsigned watchOS SDK compilation + artifact. Require **both final-head push and PR GitHub checks PASS** before recommending merge. Do not change CI to suppress flaky failures without root-cause evidence.
- Optionally add a separate nonhardware CI stress job if it yields clearer deterministic results, but do not add mandatory Bluetooth hardware access to hosted CI.

## 5. Deliverables

Create:
- `docs/research/P015_STREAM_BACKPRESSURE_DESIGN.md`
- `docs/research/P015_DISCOVERY_LIFECYCLE_REVIEW.md`
- `docs/research/P015_ARCH_BLUEZ_QUALIFICATION.md`
- `docs/research/P015_PRIVACY_EVIDENCE_POLICY.md`
- `docs/P015_VALIDATION.md`

Include at least one executable self-contained Arch preflight tool/script with tests and a separately opt-in physical qualification runbook. Update README, architecture, roadmap, feature matrix, Settings docs, threat model, and test descriptions as warranted. Record precise versions, commands, actual counts, final SHA and CI links.

## 6. Acceptance and merge boundaries

- [ ] P014 false-positive `output_blocked` when channel is *temporarily* full is fixed by a bounded progress-aware mechanism, with a deterministic burst test.
- [ ] Permanently blocked/closed output still terminates and cleans owned BlueZ resources in bounded time; writer threads do not survive CLI as persistent work.
- [ ] Streaming v1 and legacy P013/P012 CLI formats stay backward compatible.
- [ ] Qt live scan displays validated updates immediately, accurately reports progress/errors/staleness/cleanup, and no automatic scans.
- [ ] Arch preflight is read-only, privacy-safe, runnable and tested; real RF test remains a separate opt-in step.
- [ ] All final-head Rust, Python, Qt, Swift/watchOS and signed fixture CI checks PASS.
- [ ] Physical RF and Apple Watch SE 2 claims remain NOT RUN/NO-GO without actual owner-approved evidence.
- [ ] Create Draft PR on `main`, **do not auto-merge** and **do not start P016**.

**Security exclusions:** Never pair/connect/trust/forget devices, power on the adapter, modify D-Bus permissions, add active background scanning, upload identifiers/telemetry, authenticate Apple hardware from Bluetooth signals, implement AirDrop/AWDL or AirPods controls, alter PAM/polkit/login manager, or unlock Linux. All security research remains `authorization: never_evaluated`.

This milestone is **reliability and qualification tooling**, not proof of RF proximity, certified Watch identity, or Apple-native interoperability.
