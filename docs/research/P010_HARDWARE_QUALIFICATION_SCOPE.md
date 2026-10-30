# P010 — Physical Watch ↔ Arch Linux BlueZ Qualification & Signed Research Evidence

**Status:** Software qualification tooling implemented; validation tracked in [P010 validation](../P010_VALIDATION.md). Physical BLE hardware acceptance BLOCKED; trials NOT RUN.
**Branch:** `research/p010-watch-bluez-hardware-qualification`
**Base main:** `c6028d88b346878c5e2127951ed8959bad64d967`
**Merged prerequisite code:** P007B infrastructure PR #8 (squash `c6028d88...`), P009/P009A transport and temporary research enrollment PR #10 (squash `66ad10f4...`), P008 signed research protocol PR #9. All are **source/CI milestones**, not proof of real radio or Apple device behavior.
**Provenance:** The authoritative P001–P060 roadmap file was not recovered. This P010 is an explicitly provisional continuation of the Watch research track, not a claim about original P010 scope.

## Goal, evidence and go/no-go policy

Qualify the **actual physical** Linux BlueZ LE-peripheral ↔ foreground Apple Watch SE 2 CoreBluetooth-central path, first P007 synthetic service, then P009 signed research transport. The scope includes safe runbooks, reproducible CI/dev build artifacts, sanitized evidence collection, focused fixes for *observed* interoperability failures and rigorous go/no-go reporting.

A PASS on software, Xcode compilation, D-Bus mocks, GATT registration, or independent RF scanning does **not** imply success of another stage. Physical stages are PASS **only** with actual evidence from named equipment; otherwise NOT RUN/BLOCKED/FAIL. Do not declare P010 fully accepted if the requisite physical trial did not occur.

Even successful authenticated P009 signed approval proves only a configured **test key** signed the request/response. It is neither Apple's proprietary Auto Unlock nor genuine Apple Watch identity, wearer presence, secure proximity or Linux authorization. Every report must include `authorization: never_evaluated`.

## Current baseline and protocols

- P007/P007A: synthetic GATT v1 service `8e6f0001-6f4e-4c4f-9b4f-4f70656e436f`; version, 25-byte challenge, frame writes, 44-byte success/cancel value or 11-byte error. Long-read behavior on physical Watch UNKNOWN.
- P007B: read-only sanitized host preflight `scripts/qualify-p007b-host.py`, opt-in bounded 64-event `GattDiagnostics`, manual signed installation and BLE runbooks; source now integrated with P009A; no real hardware evidence.
- P008: `OC-WATCH-AUTH-RESEARCH/1`; strictly bounded canonical P-256 ECDSA/SHA-256 signed requests and responses, explicit research action, 60-second absolute expiry, independent test-only pinned host and Watch public keys; no persistent production replay store.
- P009/P009A: separate `9a7f0001-6f4e-4c4f-9b4f-4f70656e436f` authenticated research service; GET/read/ACK for host request, write-with-response for signed Watch reply; max 1088-byte signed envelope, 15-byte framing header, conservative five-byte data chunk / 218-frame ceiling. Watch test enrollment setup, P-256 ephemeral in-memory key, manual out-of-band comparison of both SHA-256 public-key fingerprints, explicitly initiated foreground consent via `LocalAuthentication`. Physical callback timing, radio interoperability and LA policy have NOT RUN.
- P009A deadline model: BlueZ setup up to 30s; sign request after registration; host wait 45s; host request transfer 20s/8s idle; consent 25s; response transfer 15s/8s idle; absolute signed expiry <=60s; 1–60s bounded server runtime. No extension of signed expiry.

## Phase A — Qualification prerequisites (read-only, minimal mutation)

1. Review `docs/research/P007B_REAL_BLE_RUNBOOK.md`, `docs/research/P007B_APPLE_SIGNED_INSTALL.md`, `docs/research/P009A_TEMPORARY_ENROLLMENT.md`, `docs/research/P009A_PHYSICAL_TEST_PLAN.md`, `docs/P007B_VALIDATION.md` and P009A validation.
2. On the **owned Arch test host**, collect **sanitized** kernel, BlueZ/firmware/controller capabilities, powered-adapter state, D-Bus service owner, `GattManager1`, `LEAdvertisingManager1`, supported/active advertisement instances, current permissions and the name of the selected adapter. Existing preflight scripts are read-only and must remain so. Running a test server requires separate explicit operator action.
3. Record whether 128-bit advertised service plus optional local name fits controller/BlueZ advertisement capacity. If `InvalidLength` appears, distinguish the actual D-Bus error and on-air packet content. Any change to advertised payload MUST remain private experimental service, preserve its UUID and include unit/mock tests; never power/configure the adapter or weaken D-Bus policy automatically.
4. No assumption that this hosted agent can access the user's Arch host, Xcode signing identity, Watch SE 2 or paired iPhone. Mark unavailable probes as BLOCKED/NOT RUN, with the exact missing prerequisite.

## Phase B — Physical installation and first GATT v1

1. Confirm an Apple Watch SE 2 is paired to an owned iPhone and a compatible **signed development build** can be installed via Xcode onto that Watch. Keep signing assets external to repository and hosted CI.
2. Explicitly run the existing P007 synthetic GATT server for a bounded interval on the selected powered BlueZ adapter. Verify actual GATT/advertisement registrations, separately verify RF beacon with a distinct owned scanner, and verify Watch service discovery. Report these independently.
3. On physical Watch verify complete 25-byte challenge and 44-byte result (or 11-byte error), sequential 20-byte writes, Complete/Cancel control, SHA-256 digest verdict, response at MTU 23 and other actually negotiated MTUs. Record exactly which CoreBluetooth callbacks occurred and sanitized byte lengths; no raw payload, MAC, Apple Account or private key logs.
4. Test disconnect/reconnect/foreground exit, BlueZ daemon/service owner loss, SIGINT, timeout, cleanup and negative conditions. Verify owned GATT/advertisement objects actually removed; do not claim unobserved disconnect callbacks were received.

## Phase C — Real P009 signed research trial (only after prerequisite pass)

1. Create a fresh Linux test-only P-256 host key with `generate-host-key`; `export-host-pin` and import it on Watch. Generate ephemeral Watch CryptoKit key and share **public-only** Watch enrollment. Compare both key fingerprints over a separately verified, out-of-band channel; run `check-enrollment` with effective UID and require both fingerprint confirmations in `serve`. No automatic trust from BLE or pairing; no fixed app private keys.
2. Run P009 opt-in bounded server **only after** verified setup. Observe signed host request issue time after registration. Watch scans/connects explicitly, reads fixed-size status, GET/read/ACK all frames, assembles the canonical request+signature, verifies pinned host signature and exact UID, nonce, session, action, P008 expiry and replay BEFORE displaying consent.
3. Test a distinct signed **rejection** first. Then, with deliberate human consent and a supported on-device `LAContext.deviceOwnerAuthentication` policy, test signed approval. Record true physical consent/callback evidence, not simulated confirmations. Linux must verify Watch public-key signature and report a **research-only** verdict.
4. Explicitly measure physical frame count, one-in-flight write responses, negotiated ATT/MTU, mean and worst-case request GET/read/ACK latency, response write latency and total readiness-to-verified-response time. The 60-second signed bound is hard; fail closed and diagnose if physical timing makes the scheme impractical, then propose a *separately reviewed* transport optimization (e.g. lower round trips or ATT notification semantics with real CoreBluetooth support) without downgrading cryptography or silently extending a signed request.
5. Test wrong pinned key, wrong UID, duplicate and stale callback, changed owner/session/transfer ID, malformed/truncated frames, failed LocalAuthentication, expiry during consent, response started near consent deadline, premature disconnect, app suspension, BLE setup errors and cleanup. No fabricated device claims.

## Phase D — Implement reproducible qualification instruments

Deliver actual code, not documents alone:
- Extend the existing sanitized P007B diagnostics with **explicitly opted-in** P009 stage/latency counters (timestamps/elapsed durations, fixed public stage IDs, expected vs actual byte lengths, frame count, ACK count, timeout/error categories, cleanup), bounded and disabled by default. No raw payloads, GATT addresses, device identifiers, keys, cryptographic transcripts or Apple accounts in exported reports.
- Build a deterministic, **offline self-test** that exercises the same Rust/Swift transport counters and schema without a controller; mark `source: simulated`, never `source: physical` and test that synthetic results cannot be mislabeled.
- Produce privacy-preserving host evidence JSON schema and validation command, where `PASS` fields require operator-supplied **actual proof** (observation timestamp, device/tool versions and per-stage evidence reference). An empty or simulated document must not pass as hardware validation.
- Optional host-only hardware smoke runner must be separately enabled, bounded, selected-adapter-only and confirm explicit user approval; scripts must default to read-only. No automatic Bluetooth power, pairing, global config, privileged installation or network listener.
- Consider a **reproducible unsigned Watch development build artifact** in hosted Apple CI where feasible, with clear manual signed installation steps; do not upload signing keys, profiles, app secrets, or claim that an unsigned bundle is installable.
- Use one connected host with proper command inputs to diagnose; if hardware unavailable, do all software instrumentation/tests and report physical trials BLOCKED. Never lower go/no-go criteria to compensate for absent equipment.

## Tests, docs and acceptance

- Preserve P007 synthetic v1 and P008/P009 cryptographic formats and all P009A lifecycle/enrollment boundaries. No auto unlock, PAM/polkit/login integration, identity attestation or secure proximity.
- Rust: `cargo fmt --all -- --check`, Clippy with `-D warnings`, workspace tests/build, `git diff --check`.
- Python: preflight/evidence validator unit tests; Swift: package XCTests, Rust→Swift→Rust framed interop; Apple: unsigned watchOS build on hosted macOS.
- Adversarial tests: incomplete/misclassified stage evidence, redaction, event cap, wrong owner/session/expiry, timeout, disconnect, inconsistent lengths and false physical success.
- Require explicit stage-by-stage PASS/FAIL/BLOCKED/NOT RUN and exact host/tool/software/hardware evidence. Never infer success from generic CI pass.
- Create `docs/research/P010_HARDWARE_QUALIFICATION_RUNBOOK.md`, `docs/research/P010_SIGNED_WATCH_INSTALL.md`, `docs/research/P010_RF_AND_MTU_FINDINGS.md`, `docs/research/P010_SECURITY_BOUNDARIES.md`, `docs/P010_VALIDATION.md`; update architecture/feature matrix/roadmap/README with accurate status.
- Submit PR from existing P010 branch against `main`. Leave Draft when hardware go/no-go is incomplete, even if source/CI passes. Never auto-merge or begin P011.

## Non-goals / hard prohibitions

Apple's proprietary Watch Auto Unlock/STS, production trusted Watch identity, Secure Enclave attestation, wearer/proximity proof, RSSI-based authorization, PAM, polkit, login-manager modification, system session unlock, automatic background approval, persistent production credentials, unreviewed privileged Bluetooth policies and automatic host/Watch trust. Every result remains `authorization: never_evaluated`.

**P010 is a controlled physical-evidence milestone. If the developer environment lacks actual Linux Bluetooth and Watch hardware, complete the code/tests/runbooks now and report real-device acceptance as BLOCKED rather than marking P010 hardware qualified.**
