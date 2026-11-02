# P014 — Live Bluetooth LE Discovery Stream & Native Settings (Provisional)

**Status:** STARTED — branch and scope only; implementation NOT STARTED; PR NOT CREATED.
**Branch:** `feat/p014-live-ble-discovery-stream`
**Base main:** `c7a7bc58958e9b112580df39477096ef29152e54` (P013 PR #14 squash merge).
**Roadmap provenance:** The original `OpenContinuity-Roadmap-P001-P060.md` remains unavailable. This is an explicitly provisional P014 continuation, not a claim about its original definition.

## Goal and non-negotiable safety

Add **real-time, opt-in** BlueZ scan progress and observed-device updates to the P013 native Qt6/QML Settings Devices page. P013 currently uses a bounded one-shot CLI that reports one final JSON object, so Qt shows coarse elapsed progress and a final result. P014 adds a **separate versioned, line-delimited JSON event protocol**, capable of streaming sanitized records while the existing P013 discovery session runs. The final status is authoritative only after bounded owned-session cleanup. This milestone does not pair/connect/trust devices or implement any Apple protocol, Apple Watch identity, distance proof, or Linux authorization.

P010 Apple Watch SE 2/Arch physical interoperability is still NO-GO; a successful mocked D-Bus scan or offscreen Qt test is **not** RF or watchOS hardware qualification. All outputs carry `authorization: never_evaluated`.

## Baseline to preserve

- Rust CLI `opencontinuity devices list --json`: P012 read-only one-shot BlueZ known-device snapshot with schema v1; no scan.
- Rust CLI `opencontinuity devices discover --seconds 10 --json`: P013 explicit 1–30s LE scan, separate schema `bluez_le_discovery_session` v1, GetManagedObjects + owner-validated signals, per-client discovery filter, exactly one owned Start and matching Stop attempt, bounded setup/cleanup/cancellation and privacy-sanitized ephemeral IDs; **keep byte-level JSON contract and exit codes unchanged**.
- `apps/opencontinuity-cli/src/device_discovery.rs` houses the production discovery state machine and D-Bus async code. `apps/opencontinuity-cli/src/radio_guard.rs` provides cooperative same-UID exclusion against P007/P009 GATT experiments; not a global lock.
- `apps/opencontinuity-settings`: independent CMake/C++20, Qt6/QML Settings pages, P012 read-only DeviceInventoryProvider and P013 DeviceDiscoveryProvider; P011 DoctorProvider v1. Existing tests on Qt 6.4.2 and Swift/watchOS passed final P013 head.
- Real Arch adapter/Watch trials not performed; do not replace an unavailable device with fake positive evidence.

## 1. Streaming CLI protocol (new, explicit flag)

Add `opencontinuity devices discover --seconds 10 --events-jsonl` (and optional validated `--adapter-index`). `--events-jsonl` and existing `--json` are mutually exclusive. `--json` stays the default only when explicitly requested, exactly as P013; invalid/missing format returns exit 2 before any D-Bus access.

Define a separate, strictly versioned `bluez_le_discovery_events` schema v1. One UTF-8 JSON object per newline, flushed promptly; no log chatter on stdout/stderr during an accepted invocation. Suggested envelope:
- common `schema_version:1`, `source:"bluez_le_discovery_events"`, `type`, `authorization:"never_evaluated"`, and `scan_session_id` (a cryptographically random **per-invocation non-persistent** token, not a device ID).
- `type:"session"`: after setup or successful Start, explicit requested duration, adapter index, and `scan_started`; do not claim started before Start success.
- `type:"device"`: new or changed sanitized observation, ephemeral scan-scoped ID, sanitized name, provenance exactly `bluez_event_during_session`; never emit cached Device1 objects as sightings.
- `type:"terminal"`: **exactly once** after cleanup attempt where process remains alive, with full definitive P013-style status, scan_started/completed, actual duration, cleanup_status, observation count, cancellation/reason enum. Emit a terminal failure if setup fails before scan. No false successful terminal on killed/crashed or disconnected process.
- Events must be ordered, monotonic within the one session, validated with a sequence number, with at most one session/start and one terminal. Define precisely whether pre-start failure needs a preceding session record; document deterministically.

Privacy: no MAC addresses, object paths, raw advertisement/RSSI/service payloads, persistent identifiers, cryptographic material, raw D-Bus errors or Apple Account information; reuse P013 name sanitizer and random scan IDs. Sanitize control/bidi characters. Streamed device rows must share the same true in-memory identity and observation set as the final report; do not create a second mock pipeline or independently classify radios.

Bounds: max 128 unique devices, 4,096 input events, 1–30s scan, shared per-method deadlines and setup/cleanup limits, at most 512 emitted **device/update lines** and 512 KiB total streaming output, each line at most 2 KiB; cap per-row text and count. If output bounds are hit, fail closed by terminating collection, attempting owned cleanup, and emitting a non-success terminal if possible. No unbounded channels, queues, strings, logs or stdout memory retention. A slow or closed stdout consumer cannot extend a radio scan or prevent bounded cleanup: use a bounded nonblocking writer or other architecture with demonstrable backpressure/timeout behavior; avoid synchronous blocking writes on the async discovery event loop.

A stream **never** proves RF provenance or Apple identity. Avoid outputting radio observations during failed/ambiguous StartDiscovery acquisition.

## 2. Rust architecture and process lifetime

Refactor minimally to reuse the **single P013** D-Bus lifecycle and observation logic for both `--json` and `--events-jsonl`. Internal events can be emitted via a bounded callback/channel with explicit generation/session scope; no duplicated StartDiscovery, no separate scan triggered by a stream reader.

- Subscribe before Start; discard cached/pre-reply signals using receive sequence as P013.
- Keep the BlueZ unique owner pinned for all calls; owner changes fail closed.
- Retain one Start and at most one Stop on confirmed ownership, and filter cleanup even on cancellation. Lost Start reply => uncertain cleanup; never invent a successful Stop or completion.
- SIGINT/SIGTERM sends cancellation through the existing AtomicBool and allows bounded cleanup; never convert it to silent QProcess kill as the default path.
- Ensure streaming output completion does not keep BlueZ connection alive or bypass cleanup. Attempt cleanup before terminal record.
- CLI result codes remain exit 0 only for completed + clean cleanup and exit 1 for other structured outcomes; exit 2 for syntax error.
- No background scanning, service process, permanent cache, disk logs or privileged helper.

## 3. Qt live stream integration

Extend P013 `DeviceDiscoveryProvider` (or add a narrowly scoped streaming protocol class) to consume `--events-jsonl` instead of waiting for a final object. **Do not change** DoctorProvider, P012 DeviceInventoryProvider, the known-devices button, or unrelated Watch research code.

- Use QProcess with program+args separate, trusted absolute CLI resolution, no shell, explicit Scan click only.
- Parse stdout **incrementally**: split by newline; handle fragmented/coalesced UTF-8, multibyte UTF-8 boundary splits, a missing final newline, escaped/duplicate JSON keys, invalid status/type/sequence/session ID, oversized frames and total output, invalid device IDs and fields; reject malformed streams, do not show untrusted HTML.
- Model displays observed rows immediately during scanning in a distinctly titled **Unverified BlueZ observations** section, separate from the known devices snapshot. Distinguish device updates from new unique IDs. Display elapsed time and bounded progress; avoid claiming real radio proximity.
- Cancellation: send SIGTERM and wait for authoritative `terminal` record within bounded grace; only then show confirmed cleanup. If timeout/SIGKILL/crash/closed pipe occurs without valid terminal, show **cleanup unknown / incomplete**, mark any live rows **stale/unverified**. A nonzero exit is permitted with terminal error/cancel status according to schema; validate exit/terminal agreement.
- Navigation away/window close follows documented cleanup behavior, no reentrant scan, stale callbacks or resurrected old records. Unrelated known-device refresh remains accessible and independent.
- Limit Qt stdout/stderr buffers; use typed model, QML plain-text labels, no unsafe rendering, no persistent logs/telemetry. Device counts must be consistent between rows and terminal record or fail closed.
- Keep Qt 6.4.2 Ubuntu CI support, Qt 6.8+ Arch support and compositor-independent Wayland/X11 packaging. No polling in the UI beyond a local elapsed-time display; never schedule radio scans automatically.

## 4. Deterministic validation

- Rust tests: same D-Bus session lifecycle in both CLI formats, stream ordering and schema, session IDs, bounded updates and device count, split/cancel/timeout/owner change, lost Start reply, negative Start, Stop/filter failure, terminal only after cleanup, stdout closed/backpressure, no second Start, no addresses/paths/secrets, exact CLI exit status and old `--json` byte-compatible contract. Use production-path private-bus mock BlueZ fixtures (not only mock struct tests).
- C++ QtTest: fragmented line buffers, broken UTF-8, duplicate keys (incl escaped), wrong schema or session, missing terminal, multiple terminals, huge lines, midstream exit, cancelled before and after first observation, stale callbacks and generation, bounded SIGTERM→kill fallback, partial progress, no live device treated as authenticated, no background scan.
- QML: real offscreen window with click Scan, real-time device cards before terminal, update row, explicit Cancel, no-result and unavailable conditions, navigation/window shutdown, strict separation from P012 known inventory, preserve all eight pages.
- CI: Rust fmt/Clippy `-D warnings`/workspace tests/build, Python 7+ tests, Qt 6.4.2 CMake/Ninja/CTest, macOS Swift XCTest, Rust→Swift→Rust signature roundtrip and unsigned watchOS app compile/package. Preserve all existing gates and recheck final-head push and PR CI.
- Physical Arch adapter, independently verified RF and watchOS hardware behavior are separate manual tests. No optional hardware test may be silently reported PASS because a mock succeeded.

## 5. Documents and GitHub delivery

Create:
- `docs/research/P014_DISCOVERY_STREAM_PROTOCOL.md`
- `docs/research/P014_STREAM_LIFECYCLE_AND_BACKPRESSURE.md`
- `docs/research/P014_SETTINGS_LIVE_SCAN_UI.md`
- `docs/research/P014_PRIVACY_SECURITY_REVIEW.md`
- `docs/research/P014_ARCH_PHYSICAL_PLAN.md`
- `docs/P014_VALIDATION.md`

Update READMEs, architecture, roadmap, feature matrix, threat model and CI as necessary. Distinguish software/CI PASS from hardware NO-GO.

Continue from the existing branch `feat/p014-live-ble-discovery-stream`, implement actual source/tests/docs, commit, push and open a Draft PR against `main`. Promote only when all software gates pass and scope evidence is accurately recorded; **do not merge automatically or begin P015**.

### Hard exclusions

No automatic background scanning, pairing/connect/trust/forget, persistent Apple device identification, AirDrop/AWDL, AirPods mode controls, Apple Watch Auto Unlock/STS, Watch key enrollment, secure proximity, login manager/PAM/polkit, system privilege changes or Linux unlock. All result streams retain `authorization: never_evaluated`.
