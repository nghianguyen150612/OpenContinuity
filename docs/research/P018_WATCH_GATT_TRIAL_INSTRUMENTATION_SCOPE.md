# P018 — Foreground Watch GATT Trial Instrumentation & Cross-Side Evidence (Provisional)

**Status:** INITIALIZED — branch and scope only. P018 production changes, tests, PR and CI NOT STARTED.
**Branch:** `feat/p018-watch-gatt-trial-instrumentation`.
**Base main:** `e4b710d1ecf1d57bfddd352e564ef323ce7d7b4a` (P017 PR #18 squash merge).
**Source-roadmap caveat:** The original P001–P060 roadmap source has not been provided; P018 is an explicitly provisional continuation.

## Why P018

P017 delivered and qualified operator-controlled, safe host preflight and bounded BLE discovery trial tooling, private transcript validation and the W0–W6 Watch foreground runbook. It did **not** run actual Arch, Wayland or Watch trials. Repeating another preflight is not useful. P018 makes the existing P005/P006/P009 Linux GATT experiments and `experiments/watchos` foreground CoreBluetooth central app **observable in matching, bounded, privacy-safe lifecycle stages**, so a user can run the W0–W6 trial and assess where RF/GATT/protocol/consent/cleanup failed. P017's S0–S5 discovery evidence remains independent.

P010 Watch SE 2 hardware qualification, independent RF, Watch long-read/MTU behavior and native Apple Auto Unlock remain **NOT RUN / NO-GO**. No Apple-native enrollment/STS, hardware proximity, persistent identity or Linux authorization is authorized. Always `authorization: never_evaluated`.

## 0. Mandatory baseline review

Inspect the actual code and interfaces before changing flags or data formats:
- `apps/opencontinuity-cli/src/gatt.rs` and command parser for `research watch-gatt serve`; P006 synthetic GATT server/session, registration, advertisement, explicit durations, cleanup.
- P009 `research watch-auth-gatt` command implementation, temporary enrollment, signed test-only request/response, pinned owner and replay/session bounds; identify actual source filenames rather than guessing.
- `experiments/watchos/WatchBluetoothManager.swift`, `ContentView.swift`, `AuthenticatedResearchView.swift`, `ResearchConsentView.swift`, P007B bounded diagnostic model and pure Swift transport codecs.
- `docs/research/P017_WATCH_FOREGROUND_CENTRAL_TRIAL.md`, P009A temporary enrollment and P007B physical runbook; P016/BLE-owned cleanup regression and P017 qualification gates. Read docs/ROADMAP and current CI.
- Existing CLI JSON output, Swift code paths, P006/P009 wire formats and P008 signatures must remain compatible. No additional GATT service, ad UUID or second protocol unless an independently demonstrated defect mandates one.

## 1. Host GATT lifecycle diagnostics, opt-in only

Design a versioned, **local metadata-only** diagnostic stream/report for existing P006 and P009 experimental servers. If suitable diagnostics already exist, extend/reuse them rather than duplicate. Expose only after a deliberate operator flag/setting within existing `--experimental` guard. Document and test exact invocation based on the real parser. Do not change default CLI stdout/JSON contracts.

- Capture **only fixed status enums**, phase labels and monotonic **relative duration** at boundaries: adapter preflight, GATT application register requested/replied, LE advertisement register requested/replied, client connection/session accepted when actually observable, service/characteristic discovery when actually observable, request/response frame counts and lengths, ACK milestones, explicit consent decision **category** (not credentials or secrets), expiration, disconnect and cleanup attempts/replies. Do not invent a callback BlueZ does not expose.
- One randomized invocation-scoped local trial ID can correlate **operator-exported** local records, but must never appear on-air, serve as Apple identity or imply authenticated endpoint matching. Record provenance (`host_runtime_observed`, `watch_runtime_observed`, `operator_declared`), with no automatic source trust escalation.
- Bound diagnostics (e.g. <=64 events per stage run, <=2 KiB per JSON line, <=128 KiB aggregate; make exact limits explicit after source review); fixed enums, independent event sequence, one terminal after owned cleanup, and bounded cancellation. Implement a safe backpressure policy: diagnostics must **never** block transport replies, violate GATT deadlines, or prevent advertisement/application cleanup. If buffered logging fails, mark evidence incomplete, not transport PASS.
- Do not serialize D-Bus paths, MAC addresses, RSSI, raw ads, device names, raw GATT payloads, P008/P009 message bytes, keys, signatures, enrollment identifiers, fingerprints, Apple Account metadata, local usernames/private IPs or free-form exception strings. Keep full raw logs off by default; no remote telemetry, uploads or persistent data collection.
- Distinguish *Start attempted*, *reply confirmed*, *cleanup attempted*, *reply confirmed*, *unknown due to process loss* and *independent RF corroboration*. Session exit alone is not owned RF cleanup proof. Preserve radio guard, selected adapter/BlueZ unique owner, and one-requested-session guarantees.

## 2. Foreground Watch app diagnostics, consent and state

P007B already contains optional 64-event metadata-only diagnostics. Reuse these; do not introduce a second telemetry feature.

- Audit `WatchBluetoothManager` state transitions and CoreBluetooth delegate generation: explicit foreground Start Scan → discovery → connect → discoverServices/characteristics → P006 synthetic exchange OR isolated P009 signed research flow → decision → response/ACK → cancel/disconnect/expiry. Record only events actually delivered by public APIs. Each phase must distinguish observed vs expected and include bounded relative timing and fixed outcome enums.
- Diagnostics disabled by default; require explicit UI opt-in, show current capture state and ability to clear it; erase on opt-out, app background/suspension and terminal cleanup as appropriate. Avoid displaying personal device names or insecure full callback error descriptions. Keep watchUI responsive and nonblocking.
- If metadata-only diagnostics cannot be exported from watchOS without leaking identifiers, provide a human-readable sanitized counters/status screen and a manual operator checklist instead of inventing unsupported Watch sharing APIs. Never claim an unsigned watchOS CI build is physically installed or signed.
- Do not change the P006/P009 frame format, static UUIDs, 20-byte ATT-MTU-23 fallback assumptions or signed research enrollment model. Detect and report MTU/long-read ambiguity as unknown or failure, not as a successful trial.
- The Watch acts as a **CoreBluetooth central** and Linux advertises the experimental GATT **peripheral**. Never require Watch BLE advertisement or infer background watchOS reliability.

## 3. Correlation, offline validation and truthful evidence

Implement a separate pure offline checker (e.g. `scripts/p018_watch_trial_evidence.py`) and deterministic tests. It ingests explicitly supplied sanitized **host diagnostics** and **Watch operator-exported metadata** only; if no Watch export exists, accept a strict allowlisted operator observation record, but mark it unverified.

- Versioned report/envelopes with fixed stages: W0 signed physical installation; W1 Linux GATT app+advertisement and independent RF; W2 Watch foreground scan/connect/discovery; W3 P006 synthetic transport; W4 P009 signed test-only rejection; W5 explicit consent/LA and signed approval (on real Watch only); W6 cancellation/timeouts/disconnect/owned cleanup.
- For each stage use `pass`, `fail`, `blocked`, `not_run`, with evidence source `mock_ci`, `host_runtime_observed`, `watch_runtime_observed`, `operator_declared`, `independent_observer`. A source that is only a supplied file or operator declaration is *not* device attestation. No automatic physical PASS just because a record claims pass; all physical achievements require separate owner-reviewed, independently corroborated evidence.
- Strict allowlist JSON parser: deny duplicate JSON keys, nested/oversized payloads, unknown schemas/enums, inconsistent sequence and phase, missing terminal, mismatched run IDs, impossible timestamps/counts, truncated lines, and unsafe labels. Never echo raw untrusted input or file paths on error.
- Preserve protocol and privacy boundaries; do not import or decode signed messages/keys to produce evidence, and do not implicitly make this into a Watch authentication verifier. A simulated signed denial in CI is software-only.
- Add a concise troubleshooting matrix mapping **verified** stage failures to actionable safe causes (BlueZ GattManager1/LEAdvertisingManager1 unavailable, advertisement slot unavailable, permission denial, Watch app not foreground, incomplete characteristic/long-read delivery, missing ACK, consent cancelled, LA unsupported, Stop/Unregister timeout). Avoid speculative claim about proximity or pairing status.

## 4. Explicit, staged physical runbook

Provide operator instructions that use **verified existing CLI command syntax** and current Watch UI. Trial order:
1. P017 read-only host check; physical S0–S5 discovery trial is **separate** and may run before GATT but must not overlap same adapter guard.
2. W0: signed development Watch app installation and explicit launch on owned Watch SE 2; signed local build via Xcode/device team, never ship signing secrets.
3. W1: opt-in, bounded P006 Linux GATT advertisement, independent radio observer; reject unavailable controller or permission without auto-changing policies.
4. W2–W3: Watch foreground scan/connect and P006 synthetic exchange; verify exact result/ACK and bounded cleanup; record sanitized host/watch phase deltas. Repeat cancellation, timeout and device suspension.
5. W4: stop P006, explicitly configure existing manual test-only P009 public-key pins and dual fingerprint confirmation, run signed test rejection and verify on Linux.
6. W5: separate fresh session and explicit consent/LocalAuthentication real Watch approval only when public policy supported; document fail-closed unsupported/denied behavior.
7. W6: cancellation, disconnect, timeout, radio guard, advertisement/application unregister and controlled receiver RF cessation; report cleanup *attempted* separately from *confirmed*.
At each step show privacy warnings and `authorization: never_evaluated`. No pairing/trust writes, auto scan, login integration, Apple native STS/Auto Unlock, OS changes or key export. No physical hardware available in cloud means **NOT RUN**, not PASS.

## 5. Deterministic software coverage / CI

- Rust private fake BlueZ bus: real production paths for P006/P009 servers and diagnostics (existing tests where available); rejection, TTL expiry, method cleanup, output saturation and bounded unexpected service/owner loss. Do not assume Watch hardware in CI.
- Swift pure codec/transport and state-machine tests: diagnostics disabled by default, bounded cap, explicit enable/clear, stale generation, foreground/background transition, cancel, consent, deadline expiry, no secret/identifier export. No Apple-specific API shims faking hardware success.
- Python checker tests: good/bad evidence, strict validation, no physical auto-promotion, P017 stage independence, safe file handling/limits, privacy and no network. No active scan or advertisement in CI.
- Qt/C++20 Settings and QML tests stay intact. Only touch Qt if necessary to support actual troubleshooting without adding automatic Bluetooth actions.
- Preserve Rust fmt/Clippy -D warnings/test/build, Python unittest, Ubuntu Qt 6.4.2 CMake/Ninja/CTest/QML, hosted Swift XCTest/Rust→Swift→Rust signed fixtures, unsigned watchOS compilation and artifact packaging. Both **final-head push and PR** CI must pass. Do not skip unreliable tests or represent CI as signed Watch hardware.

## Deliverables / GitHub

Create:
- `docs/research/P018_WATCH_GATT_DIAGNOSTIC_SCHEMA.md`
- `docs/research/P018_HOST_AND_WATCH_PHASE_MAPPING.md`
- `docs/research/P018_OFFLINE_EVIDENCE_AND_PRIVACY.md`
- `docs/research/P018_WATCH_SE2_PHYSICAL_TRIAL_RUNBOOK.md`
- `docs/research/P018_GATT_LIFECYCLE_SECURITY_REVIEW.md`
- `docs/P018_VALIDATION.md`
Update source/CLI/Watch READMEs, docs/ROADMAP, feature matrix, architecture, threat model and CI only as warranted. Each validation entry must distinguish software, fake BlueZ, physical Arch/RF, real Wayland and physically installed Watch.

Continue on `feat/p018-watch-gatt-trial-instrumentation`; implement real instrumentation/validator/tests/docs, commit, push, create a **Draft PR against main** and check all final-head workflows. **Do not merge or begin P019**.

**Hard exclusions:** Apple-native Watch Auto Unlock, STS, genuine Apple identity, on-air Apple protocol spoofing, secure proximity claims, PAM/polkit, login manager, unlock permission, production enrollment/keys, pair/trust changes, persistent background scanning, telemetry, forced policies, and persistent collection of device identifiers. `authorization: never_evaluated` throughout.
