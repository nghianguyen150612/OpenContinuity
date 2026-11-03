# P017 validation

P017 is provisional and delivers operator-controlled software qualification and
manual hardware procedures on `feat/p017-arch-ble-physical-qualification`.
Software results are independent from physical evidence. The canonical original
roadmap remains unavailable. No P018 work or automatic merge is included.

## Executed local software validation

- Rust 1.85.0: `cargo fmt --all -- --check`, workspace/all-target Clippy with
  `-D warnings`, `cargo test --workspace --all-targets`, and workspace build PASS.
  All 179 tests pass, including 29 production CLI/private BlueZ discovery tests.
  P016's delayed-Stop/subscription cleanup regression, owner isolation, shared
  clients, 514-record budget and bounded writer-progress cases are preserved.
- Python: `python3 -m unittest discover -s scripts/tests -v` PASS (37 tests).
  P017 adds 17 test methods covering read-only default, missing host prerequisites,
  consent/ownership/duration rejection, trusted executable resolution, real fixture
  process execution, deadline/cancel/KILL fallback, descendant-held pipes, output
  cap, missing terminal, abnormal exit, incomplete cleanup, duration mismatch,
  private file creation, redaction, evidence schema and no physical promotion.
  P015 gains a regression preventing activation of an unowned BlueZ service; all
  subsequent probes target its observed unique owner. Existing tests remain.
- Debian Bookworm container: Qt 6.4.2, GCC 12.2, C++20, CMake/Ninja build PASS.
  CLI rebuilt inside the container for matching glibc. All four CTest suites PASS:
  Doctor 62, inventory 27, discovery 42, actual QML window 12. Real offscreen QML
  covers live observations, burst updates, partial terminal, cleanup warning,
  cancel/navigation/window-close and stale callback behavior. Installation,
  executable version and real installed-window offscreen launch smoke PASS.
- Read-only P017 check executed on this Debian 13 cloud host with the built CLI:
  kernel numeric version 6.18.57, CLI 0.1.0; system bus/BlueZ unavailable, zero
  reported adapters and Settings absent from trusted PATH. S0 BLOCKED. No real
  discovery/GATT command or RF operation was run. This is host preflight only.
- `git diff --check` PASS. No production Rust/Qt/Watch protocol changes.

The shared CI workflow includes Python qualification tests and diff checking;
no radio scan is added to CI. Apple SDKs/CryptoKit are unavailable locally.
Final-head hosted macOS results must supply Swift XCTest, CryptoKit validation,
Rust→Swift→Rust signed transport/signature interoperability, unsigned watchOS
compilation and unsigned artifact packaging. None proves signed installation.

## Delivery verification

The Draft PR targets main. Final revision, Draft status and the push/pull_request
workflow URLs are recorded in the PR and task final report after execution.
Both events must each pass Rust, Qt Settings and Swift/watchOS (6/6). Checks from
the starting revision or P016 do not qualify the final head. The unsigned Watch
artifact must be present with local signing required. Never merge automatically.

## Explicit physical gates

| Gate | Outcome | Reason |
| --- | --- | --- |
| S0 actual Arch readiness | BLOCKED | Only non-Arch cloud host available; no usable system bus/adapter |
| S1 physical scan | NOT RUN | No owned approved Arch radio |
| S2 physical event/terminal/exit trial | NOT RUN | Software fixtures only |
| S3 independent RF | NOT RUN | No controlled peripheral/separate receiver |
| S4 native Wayland/Hyprland | NOT RUN | Offscreen CI/container only |
| S5 independently observed physical cleanup | NOT RUN | Mock BlueZ ownership tests only |
| W0 signed Watch SE 2 installation | BLOCKED / NOT RUN | No Watch, paired path or signing environment |
| W1–W6 Watch advertisement/connection/P006/P009/LA/cleanup | BLOCKED / NOT RUN | No signed installed app or real Linux adapter |
| P010 Watch physical acceptance | NO-GO | Separate hardware gates remain open |
| Genuine Apple Auto Unlock / Linux authorization | NO-GO / never_evaluated | Explicitly outside scope |

The wrapper exports only fixed vocabulary and bounded counts, with optional
private 0600 raw files. Operator declarations are schema-checked but unverified;
reported passes never promote observed hardware stages or certify physical RF.
A successful P006/P009 exchange proves neither genuine Apple identity, secure
proximity nor permission to unlock Linux.

Software merge readiness requires green final-head CI and human review. Hardware
qualification remains BLOCKED independently. Residual risks are real controller
filtering/firmware, shared BlueZ semantics, owned cleanup under physical failures,
Wayland plugin/compositor behavior, advertisement packing, Watch long reads/MTU,
callback timing and signing/provisioning. No hardware claim is fabricated.

Deliverables: [host tool](research/P017_SAFE_HOST_QUALIFICATION_TOOL.md),
[Arch/Wayland matrix](research/P017_ARCH_BLUEZ_PHYSICAL_MATRIX.md),
[evidence and cleanup](research/P017_RF_AND_CLEANUP_EVIDENCE.md),
[Watch foreground trial](research/P017_WATCH_FOREGROUND_CENTRAL_TRIAL.md),
[privacy review](research/P017_PRIVACY_AND_TRUST_REVIEW.md).
