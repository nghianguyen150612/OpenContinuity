# P016 — BLE Adversarial Reliability & Linux Qualification Evidence (Provisional)

**Status:** Implementation and adversarial coverage added; see [P016 validation](../P016_VALIDATION.md) for executed results, delivery and remaining physical gates.
**Branch:** `feat/p016-ble-adversarial-qualification`.
**Base main:** `1661bd39cb131da23d0b44c8c0ee301081c3f01a` (P015 PR #16 squash merge).
**Provenance:** Original `OpenContinuity-Roadmap-P001-P060.md` was not available. P016 scope is provisional and does not claim to reproduce an unseen original milestone.

## Mission

Complete the **explicit P015 risk debt** and build truthful, reproducible qualification evidence for P013–P015 Bluetooth LE discovery on Arch Linux. This is a reliability-and-evidence milestone, not an Apple Watch/AirPods pairing or native interoperability milestone. The P015 merge was an explicit operator exception: 6/6 final-head GitHub CI gates passed (Rust 174, Python 9, Qt Doctor 62 / inventory 27 / discovery 36 / QML 10, Swift/watchOS) but continuous slow-reader, cancellation/owner-loss under backpressure, terminal write failure and Qt stress had not been proven. Address these gaps with deterministic production-path tests **before** asking for a P016 merge.

P010 physical Apple Watch SE 2 qualification remains **NO-GO**; real RF, physical BlueZ cleanup and native Wayland/Hyprland were NOT RUN. All research results retain `authorization: never_evaluated`. Apple's native proprietary protocols remain unsupported.

## Baseline — inspect before coding

- `apps/opencontinuity-cli/src/device_discovery.rs`: P013 read-only? **No**: discovery actively invokes per-client BlueZ `SetDiscoveryFilter`, `StartDiscovery`, and owned `StopDiscovery` only when the user explicitly requests a bounded scan. P014 `--events-jsonl` uses session/device/terminal v1, shared P013 discovery lifecycle and an asynchronous stdout writer. P015 raised the bounded queue to 514 lines and reset 2 s no-progress deadline after confirmed writer acknowledgement; `SyncSender::try_send(Full)` persists, but should be unreachable within the fixed session+512-device-events+terminal budget under correct accounting. Verify this invariant rather than assuming it.
- `apps/opencontinuity-cli/tests/discovery_bluez_mock.rs`: real Rust/zbus production path with private fake D-Bus (NOT radio hardware), including 100-event delayed-reader burst, closed stdout, permanent block and owned session cleanup.
- `apps/opencontinuity-settings/src/DeviceDiscoveryProvider.cpp`, `DeviceDiscoveryReport.cpp`, `DeviceDiscoveryModel.cpp`: QProcess fixed CLI args, incremental strict JSONL parsing, bounded buffers, live rows, terminal/exit checking, Cancel SIGTERM and SIGKILL fallback, generation fence, stale/cleanup uncertainty.
- `scripts/p015_arch_preflight.py`: existing read-only sanitized adapter/system bus inspection. Reuse/refine rather than introducing new host privileges or scraping identifiers.
- Keep P012 `opencontinuity devices list --json`, P013 `opencontinuity devices discover --seconds N --json`, P014 `--events-jsonl` contracts **unchanged** unless a documented bug absolutely necessitates a versioned migration. Do not falsify backward compatibility.

## A. Adversarial producer/writer qualification

Implement deterministic, bounded, **real spawned CLI + fake BlueZ private D-Bus** tests wherever possible. Do not simply test an isolated clone of the writer.

1. Continuously slow but progressively draining stdout reader: many events, small read chunks, delayed read iterations, full order and exact terminal/device counts. Ensure deadline resets only after real write+flush acknowledgements; do not confuse slow *progress* with a permanent stall. Note the Qt process deadline is independent; document any slow-read rate that eventually exceeds it.
2. Saturate the permitted 514-record accounting with rapid new-and-updated records. Document the mathematical maximum (1 session + <=512 device events + 1 terminal), ensure no queue overcommit/sequence loss. A healthy fast reader must never receive false `output_blocked`; memory and total byte caps still apply.
3. **Cancellation while output is pending**: SIGINT/SIGTERM after one or more device events; verify exactly one owned Start/Stop attempt and filter clear, bounded exit, terminal `cancelled` with accurate `cleanup_status` when deliverable. With a blocked stdout, require bounded exit and no claim of terminal delivery.
4. **BlueZ unique-owner loss under queue pressure**: assert no redirected Stop to a replacement owner, fixed `bluez_unavailable` outcome, cleanup truthfulness and no indefinite writer thread/process. Test a bus close as a separate scenario.
5. **Terminal failure after BlueZ cleanup**: deterministic fail/close the stdout consumer precisely after prior events, including during final terminal write. Confirm owned Stop/filter clear occurred before terminal attempt; process returns nonzero and does not claim successful stream. If terminal bytes were partially visible, Qt must reject incomplete JSONL and mark the session stale/cleanup uncertain.
6. Very slow blocked writer, event flood at 4096, record cap 512, >2 KiB line, 512 KiB aggregate, duplicate signals, failed Start/uncertain reply, cleanup error. Test exact counting and absence of second Start/Stop.
7. Add bounded test timeouts/watchdogs and unique isolated fake bus instances to avoid CI hangs or cross-test interference. Never make a test depend on the shared host Bluetooth radio or external network.

**If a real production bug is found**, make the smallest safe change; do not rewrite the stream architecture merely to inflate changes. Review channel memory upper bound and the ability for the writer thread to survive only until process exit; never introduce an unbounded join, send or busy-spin. Preserve application-level two-second progress deadline and the 1–30s user scan bound.

## B. Qt adversarial consumer and user-visible reliability

Extend **actual** QtTest/QML test entrypoints on Qt 6.4.2-compatible Ubuntu:
- 100–200 rapid valid JSONL device/update lines, split across chunks, with live model updates **before** terminal.
- Split multibyte UTF-8, fragmented/coalesced lines, escaped duplicate JSON keys, overlong line, aggregate overflow, invalid sequence/session/counts, trailing partial line.
- Cancel after a queued burst, Cancel before process starts, navigation away while streaming, window close with a stalled child, force kill, crash, malformed/absent/partial terminal, sender with stale generation. Check explicit stale and cleanup-uncertain UI.
- Independently verify fixed `QProcess` program and safe args, no scan on app startup, no automatic reconnect/scan, and separation of P012 known devices from live **unverified** observations.
- Ensure no test silently treats `stale` or `cleanup=incomplete` as successful or confirmed radio cleanup.

Make test fixtures/harness deterministic and reusable. Do not weaken assertions or disable previous smoke cases merely to make CI green.

## C. Qualification evidence workflow (default read-only)

Extend `scripts/p015_arch_preflight.py` only if warranted and add a P016 **sanitized, deterministic evidence validator/bundler**, for example `scripts/p016_evidence.py`. No automatic active scan.

- Default `preflight` should perform **only read-only** D-Bus/BlueZ adapter inspection; no `SetDiscoveryFilter`, `StartDiscovery`, `StopDiscovery`, adapter power, pairing, D-Bus policy edits, sudo, remote endpoints or telemetry. No automatic discovery at runtime.
- Evidence intake accepts explicit **local** user-provided preflight JSON and optional sanitized *existing* P014 JSONL session file; validates schema, sequence, session IDs, bounded lines/total, device count, terminal/exit claims, no raw MAC/object paths/identifiers in output. It should reject unsafe/untrusted inputs rather than echo them.
- Emit a **versioned evidence summary** with independent statuses: environment preflight, real scan attempted, stream delivery, cleanup reported, independently corroborated RF, native Qt/Wayland interaction, Watch SE 2 qualification. Default `not_run` if no admissible evidence. A signed or parsed local JSONL file never independently proves actual RF, origin authenticity, Apple identity or Watch presence.
- Distinguish mocked private D-Bus test artifacts from actual Arch physical evidence; require explicit attestation and independent corroboration to make a physical claim, with limitations clearly documented. Do not auto-upgrade `physical_rf` from a successful software session report.
- File size, JSON depth/record/count and name bounds must be explicit; mask/sanitize local usernames, paths, MAC-like strings, IPs, device serials, names and any key/cookie fields before showing/reporting. Prefer omitting device labels entirely from the evidence summary. No file upload/network access.
- Provide a **separate**, explicit opt-in real-scan runbook using the already-existing `opencontinuity devices discover --seconds N --events-jsonl` on an owned Arch host with an independently controlled BLE peripheral. Do not invoke this scan in cloud CI or by default. Include cancellation and cleanup observation steps, BlueZ version, actual Qt Wayland/Hyprland session notes without exposing identifiers.
- Hardware qualification remains `NOT RUN/BLOCKED` when actual owned Arch/controller/Watch evidence is unavailable.

## D. Documentation and CI

Required documents:
- `docs/research/P016_ADVERSARIAL_STREAM_TEST_MATRIX.md`
- `docs/research/P016_QT_STREAM_LIFECYCLE_QUALIFICATION.md`
- `docs/research/P016_EVIDENCE_SCHEMA_AND_PRIVACY.md`
- `docs/research/P016_ARCH_PHYSICAL_RUNBOOK.md`
- `docs/P016_VALIDATION.md`

Update `README.md`, `docs/ROADMAP.md`, `docs/FEATURE_MATRIX.md`, `docs/THREAT_MODEL.md`, `docs/ARCHITECTURE.md`, Settings README, and P015 validation cross-reference as justified. Don't retroactively mark P015 unexecuted cases PASS; state P016 covered them if actual tests exist.

CI must preserve:
- `cargo fmt --all -- --check`
- `cargo clippy --workspace --all-targets -- -D warnings`
- `cargo test --workspace --all-targets`
- `cargo build --workspace`
- `python3 -m unittest discover -s scripts/tests -v`
- `git diff --check`
- Ubuntu Qt 6.4.2 CMake/Ninja/CTest, actual QML offscreen smoke, native window install check.
- Hosted Swift XCTest, Rust→Swift→Rust signed research interoperability, unsigned watchOS app compile and artifact upload.
- All **final-head** push and PR checks PASS; never infer success from an earlier SHA.

## E. Acceptance / delivery

- [ ] Adversarial tests run **against production streaming** and establish correct continuous slow-reader, owner loss/cancel under output pressure, terminal write failure after cleanup and bounded session behavior.
- [ ] No regression to P012 known-device snapshot, P013 bounded JSON scan, P014 JSONL schema or P015 progress-aware writer.
- [ ] Qt shows live data only as *unverified*, rejects invalid/stale/partial terminal, and handles close/Cancel safely under burst and blocked process.
- [ ] Read-only preflight + evidence validator are runnable, deterministic, privacy-safe and tested. No automatic or privileged scan.
- [ ] Clear separate statuses for mock software, actual RF, Qt on real Wayland and Apple Watch SE 2; no accidental physical qualification.
- [ ] Final-head Rust, Python, Qt, Swift/watchOS hosted CI complete SUCCESS.
- [ ] Open **Draft PR against main**. Do not merge or begin P017 without a user request.

**Hard exclusions:** Apple native Watch Auto Unlock/STS, Apple-specific pairing/enrollment, AirDrop/AWDL, AirPods mode/control, PAM/polkit/login manager, Linux unlocking, permanent scanning, background daemon, remote telemetry or device metadata upload. `authorization: never_evaluated` always. Even a perfect BLE scan is not authentic Apple identity, real proximity or a Watch secure-approval channel.

**Physical P010 gate remains NO-GO.**
