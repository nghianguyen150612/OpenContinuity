# P010 validation record

`authorization: never_evaluated`

P010 implements the safely verifiable software portion of physical qualification.
Hardware acceptance is **BLOCKED / NO-GO**, and the pull request remains Draft.
No physical server was started and no radio/controller policy was changed.

## Implemented and retained

Reviewed existing CLI GATT/main, Rust wire/signed protocol/transport, Watch manager,
diagnostics, transport/enrollment/consent views, P007B host tools and P007B/P009A
validation/runbooks before editing. Existing infrastructure is retained. New code:

- Opt-in Rust server setup milestones/lifetime/cleanup timing JSON.
- Watch structured UTC/monotonic-duration samples, request frame count,
  GET/read/ACK and write-ACK latency, request/response delivery, discovery,
  connection/services, observed write limit, consent and actual LA outcome.
  Sixty-four recent samples and fixed-stage count/total/max aggregates; opt-out clears.
- Strict offline manifest template/validator with stage/domain/length/proof checks,
  provenance and a separate physical acceptance gate. Deterministic Python/Swift/
  Rust tests cover bounds, opt-in, metadata and false simulation acceptance.
- Read-only selected-adapter probe separation and sanitized preflight evidence.
- Apple CI unsigned development artifact; existing real serialized
  Rust→Swift→Rust signature integration and actual watchOS SDK compilation retained.

No wire-format, expiry, enrollment, replay or approval-policy changes. Evidence
manifests themselves are not cryptographically signed; P009 transcript signatures
remain independently verified by existing code. Human physical proof review remains
necessary; a manifest assertion is not attestation.

## Validation execution

Local Python tests: PASS (7 tests). Local Rust formatting, Clippy, workspace tests
and build: execution in progress; see final PR/CI checks for final-head results.
Swift package tests and actual watchOS SDK build require hosted Apple CI; no Apple
SDK/Swift runtime is installed in this Debian executor. Hosted results are software
results only. `git diff --check` is required before commit.

## Physical evidence

[Preflight JSON](research/fixtures/P010_CLOUD_PREFLIGHT_2026-10-30.json) records
Debian 13, kernel 6.18.57, no selected controller, unavailable BlueZ utilities and
failed service/D-Bus command probes. No Bluetooth sysfs controller is exposed.
[Evidence manifest](research/fixtures/P010_CLOUD_EVIDENCE_2026-10-30.json) records
all required physical stages BLOCKED; it validates structurally and fails physical
acceptance deliberately. Actual trials are NOT RUN.

BlueZ registration, RF advertisement, physical reads/writes/digest, signed P009
exchange, LocalAuthentication, long-read/ATT MTU, latency feasibility, reconnect
and physical cleanup all remain BLOCKED. Registration permissions cannot be
established by read-only introspection. No successful trace or physical number is
invented. Lack of an observed interoperability failure does not justify a protocol
fix or deadline extension.

Missing prerequisites: owned Arch host/controller/firmware and powered adapter,
working system D-Bus/BlueZ managers and registration permission, distinct owned
scanner, physical Watch SE 2 and paired iPhone, compatible Mac/Xcode with authorized
local signing, operator approval and independently verified temporary enrollment.

## Operator documents

- [Qualification runbook](research/P010_HARDWARE_QUALIFICATION_RUNBOOK.md)
- [Signed installation](research/P010_SIGNED_WATCH_INSTALL.md)
- [RF/MTU findings](research/P010_RF_AND_MTU_FINDINGS.md)
- [Security boundaries](research/P010_SECURITY_BOUNDARIES.md)

Go: continue operator-controlled qualification using existing bounded research
servers. No-go: claim hardware compatibility, promote PR to Ready, authorize Linux,
merge automatically or start P011.
