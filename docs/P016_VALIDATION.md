# P016 validation

P016 implements adversarial streaming qualification on the existing P016 branch.
Software evidence is independent from RF, native Wayland and Apple hardware.

Local executed results:

- Rust 1.85.0: formatting, Clippy with warnings denied, workspace build and all
  179 workspace/all-target tests PASS, including 29 spawned CLI/private fake
  BlueZ tests (five new adversarial test functions with multiple scenarios).
- Python: 19 tests PASS, including ten P016 evidence tests. Read-only P015
  preflight executed in this Debian cloud host: system bus/BlueZ unavailable,
  no adapters reported. Sanitized evidence summary reports blocked readiness;
  no active discovery was performed on a real bus or controller.
- Debian Bookworm container, Qt 6.4.2, GCC 12, C++20, CMake/Ninja: all four CTest
  suites PASS: Doctor 62, inventory 27, discovery 42, actual QML window 12.
  The real Rust CLI was rebuilt inside this container to match its glibc.
  Offscreen QML verifies live burst updates, partial-terminal cleanup warnings,
  Cancel/navigation and bounded close of a stalled child. Native install and
  launch checks are also run with the offscreen/software Qt environment.
- `git diff --check` PASS. No Apple workflows or security capabilities changed.

The new delayed-Stop regression fails against starting HEAD with incomplete
cleanup (4.25 seconds) and passes after releasing bounded signal subscriptions
before cleanup (0.43 seconds). Retaining unread subscriptions could block zbus's
connection reader and starve method replies. The fix retains pinned-owner calls,
one owned Stop attempt, filter clear, radio guard and bounded connection close.
Queue sizing, writer progress acknowledgement and terminal ordering remain
unchanged. P012 inventory and P013 JSON regression tests pass.

See the [Rust matrix](research/P016_ADVERSARIAL_STREAM_TEST_MATRIX.md),
[Qt lifecycle qualification](research/P016_QT_STREAM_LIFECYCLE_QUALIFICATION.md),
[evidence schema/privacy](research/P016_EVIDENCE_SCHEMA_AND_PRIVACY.md) and
[physical runbook](research/P016_ARCH_PHYSICAL_RUNBOOK.md).

Delivery gate: Draft PR against main; never merge automatically. Final-head
push and pull_request workflows must both pass Rust, Qt and Swift/watchOS.
Use the PR checks and Actions links to verify the exact final revision; no
success is inferred from P015 or an earlier P016 revision. Hosted Apple checks
cover Swift XCTest, CryptoKit, Rust→Swift→Rust signatures/transport, unsigned
watchOS SDK compilation and artifact packaging. Apple SDKs are unavailable
locally, so hosted results are the applicable software evidence.

Physical Arch/controller discovery, independently corroborated RF, actual
BlueZ radio cleanup and native Wayland/Hyprland are **NOT RUN**. P010 Watch SE 2
remains **NO-GO**. A parsed JSONL transcript cannot qualify RF, exit status,
authenticity, Apple identity, proximity or Linux authorization. All observations
remain unverified; `authorization: never_evaluated`. P017 is not started.
