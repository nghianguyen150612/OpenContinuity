# P018 validation report

**Status: P018 software implementation complete; Draft PR #19 remains open.**
This report separates software checks from physical qualification. The hosted
results below validate implementation commit `c0c14d5a35cf1163cb6df0dc13d18c2816f15d6e`;
the later validation-document update does not change implementation code.

## Local Codex Cloud checks

Environment: Debian 13, x86_64, Rust 1.85.0 and Python 3.12.14. Local Swift/Xcode,
watchOS SDK, CMake, Ninja and Qt 6.4.2 were unavailable. Hosted workflows were
used for the platform gates rather than treating missing local tools as passes.

All available requested local gates passed on the implementation commit:

- `cargo fmt --all -- --check`
- `cargo clippy --workspace --all-targets -- -D warnings`
- `cargo test --workspace --all-targets -- --quiet`: **191 passed** across the
  workspace (CLI 56, private BlueZ D-Bus 25, CLI command tests 13, discovery
  CLI 3, inventory CLI 1, private discovery BlueZ D-Bus 29, core 63, daemon
  command 1).
- `cargo build --workspace`
- `python3 -m unittest discover -s scripts/tests -v`: **54 passed**.
- `git diff --check`

The private BlueZ fixture tests exercise the production registration and
cleanup paths. They do not establish real controller, RF, or Watch behavior.

## Hosted GitHub Actions

Both final implementation-commit workflows passed all three jobs:

- [Final-head push workflow](https://github.com/nghianguyen150612/OpenContinuity/actions/runs/42805082655)
- [Final-head pull-request workflow](https://github.com/nghianguyen150612/OpenContinuity/actions/runs/42805092259)

Each workflow passed Rust checks, Qt Settings (Linux), and Swift tests/watchOS
compile. Hosted details:

- **Rust:** formatting, Clippy, all **191 tests**, build, and whitespace checks
  passed on both events.
- **Qt 6.4.2:** CMake/Ninja configuration, build, installation, and **4/4 CTest
  targets passed**. The four QtTest executables reported 62, 27, 42 and 12
  assertions respectively (**143 total**); the 12-assertion QML navigation test
  exercised real Scan/Cancel/navigation smoke behavior.
- **Swift/watchOS:** **36 XCTest cases**, zero failures; one suite-gated Rust/Swift
  interoperability case was explicitly selected and passed in the dedicated
  round-trip step. CryptoKit request/response verification, Rust → Swift → Rust
  signed transport, unsigned watchOS SDK compilation and unsigned development
  artifact packaging/upload all passed. The artifact is unsigned and requires
  local signing for installation.

These jobs establish software/build behavior only. They do not claim a signed
Watch installation or CoreBluetooth exchange on physical hardware.

## Physical qualification

- Arch BLE/RF and independent receiver: **NOT RUN**.
- Real BlueZ controller registration and RF cleanup: **NOT RUN**.
- Native Wayland/Hyprland: **NOT RUN**.
- Watch SE 2 signed installation: **NOT RUN / BLOCKED**.
- W0–W6 Watch SE 2 gates: **NOT RUN / BLOCKED individually**.
- P010 Watch SE 2 physical qualification: **NO-GO**.
- Native Apple Auto Unlock: **NO-GO**.

Imported host, Watch, operator, observer and fixture records remain unverified;
the offline validator never qualifies physical hardware. Every diagnostic and
validator report preserves `authorization: never_evaluated`.

## P018A diagnostic stderr liveness hardening

P018A follows the P018 final implementation report and addresses a potential
shutdown stall when a bounded diagnostic line is written to an undrained stderr
pipe. The Linux server now attempts **one** best-effort nonblocking write of a
single JSON-plus-newline record, through an independent open file description
at `/proc/self/fd/2`. Unlike a `dup` plus `F_SETFL`, this does not mutate
the original stderr's `O_NONBLOCK` flag. Regular-file redirection opens with
`O_APPEND` so earlier content is not overwritten.

The line is at most 2 KiB plus newline, less than Linux `PIPE_BUF`, hence
writes to a nonblocking pipe are atomic: a full pipe drops the record instead
of blocking or emitting a partial JSON line. A missing procfs fd, closed stderr,
or failed write also silently discards diagnostics; missing metadata can never
upgrade a trial to PASS. This liveness guard does not alter Bluetooth transport,
radio ownership, GATT cleanup, CLI stdout or the Watch wire protocol.

Two Linux Rust regressions cover successful complete-line pipe emission and
already-full nonblocking pipe rejection, including unchanged original fd flags;
an invalid descriptor is also rejected. These are software tests, not physical
controller validation. Non-pipe destinations such as regular files may still
experience filesystem I/O latency; this is not a guaranteed hard deadline for
arbitrary filesystems.

## P018A stderr hardening validation

The formatter failure at starting commit `d14492e36cf4042a5203f4781d479a7b65fe9f8e`
was caused by two rustfmt-only layout differences in the full-pipe test. The
production change uses one `write(2)` call on the reopened nonblocking stderr
description. For pipes, the 2 KiB JSON limit plus newline is below Linux
`PIPE_BUF`. A temporary calling-thread `SIGPIPE` mask suppresses only a newly
generated signal on `EPIPE`; the original mask is restored and no global signal
handler or stderr flags are changed. Missing procfs, a failed reopen, a closed
or full pipe, and failed writes produce no complete host record. The validator
requires a single newline-terminated host JSONL line, rejecting a partial
non-pipe write that happens to end after valid JSON.

On the P018A implementation checkout, these local commands passed:

- `cargo fmt --all -- --check`
- `cargo clippy --workspace --all-targets -- -D warnings`
- `cargo test --workspace --all-targets --quiet`: **196 passed** (CLI 61,
  private BlueZ D-Bus 25, CLI command tests 13, discovery CLI 3, inventory CLI
  1, private discovery BlueZ D-Bus 29, core 63, daemon command 1).
- `cargo build --workspace`
- `python3 -m unittest discover -s scripts/tests -v`: **55 passed**.
- `git diff --check`

The focused Linux diagnostic module ran **9 tests**. Four subprocess regression
tests cover one complete normal stderr line, a readerless pipe with `SIGPIPE`
set to its default action, a full pipe with a bounded child exit, and unavailable
procfs/failed reopen. The child compares `F_GETFL` before and after the attempted
write. The normal child emits exactly one record; failure cases return no
successful record. The Python evidence test rejects a host JSON prefix missing
its final newline.

The trace is emitted after the owned BlueZ advertisement/application cleanup
attempt. This prevents diagnostic output from preceding or delaying that
cleanup, but does not promise a hard process-return deadline for arbitrary
regular-file, terminal, device, procfs, or close I/O. Diagnostic delivery is
best-effort; missing output cannot establish a successful trial.

Hosted Qt and Swift/watchOS results from the preceding P018 head do not qualify
this P018A change. The exact P018A final-head push and pull-request workflow
links and conclusions will be recorded here after both events complete.
Check the PR #19 workflow history:
https://github.com/nghianguyen150612/OpenContinuity/pull/19/checks
