# P019 validation

## P019B containment assessment

The historical results below do not validate P019B. See
[receive publication hardening](research/P019A_RECEIVE_PUBLICATION_HARDENING.md).
Receive now reports writer_containment_unavailable before backend startup.
Atomic bundle storage and strict pinned doctor parsing have deterministic
software regressions. Descendant containment and graceful process-group
escalation are unsupported; no receive publication is enabled.

P019A baseline was 235 Rust tests and 55 Python tests. P019B adds an
evidence-backed containment assessment; it does not add a cgroup implementation
because the current unprivileged host has no delegation. Current run results are
recorded after validation below; these do not imply real-kernel cgroup tests.
Qt/QML, Swift/CryptoKit and watchOS gates are unchanged. Physical qualification
remains not_qualified and authorization remains never_evaluated.


P019 is software work only. This report records the current branch's tested
state separately from physical qualification. The local and hosted results
below are tied to implementation commit
`939d4978d0fa33b9dfc8e27f382be646fa92f501`; the validation-record commit is
documentation-only.

## P019B environment and decision

On Debian 13, kernel 6.18.57, cgroup v2 is mounted at /sys/fs/cgroup, but the
process is in visible cgroup root `0::/`. That root and cgroup control files
are root-owned and not writable by uid 1000. PID 1 is tail, not systemd;
systemctl --user has no bus and XDG_RUNTIME_DIR is unset. No real cgroup
integration test can be run without an externally delegated subtree. The
receive gate remains writer_containment_unavailable. Details and primary
references are in [P019B process containment](research/P019B_PROCESS_CONTAINMENT.md).

The pinned Owly source launches no separate OS child in the receive path, but
threads/tasks and this source inspection are not an enforceable boundary.
Cgroup migration, user-manager access, disk quota and physical qualification
limitations remain explicit. No host cgroup or service was modified.

P019B validation on this host: `cargo fmt --all -- --check` PASS;
`cargo clippy --workspace --all-targets -- -D warnings` PASS;
`cargo test --workspace --all-targets` PASS, 235 tests;
`cargo build --workspace` PASS; Python unittest discovery PASS, 55 tests;
`git diff --check` PASS. The process-containment regression cases are the
existing fail-closed/software-fixture tests (including known-writer publication
blocking); no kernel cgroup lifecycle case ran because delegation is absent.
Qt 6.4.2/QML, Swift/CryptoKit/watchOS remain hosted CI gates.


## Implemented paths

- Rust v1 AirDrop status contracts and receive lifecycle.
- Read-only Linux sysfs/procfs and fixed-path `iw`/Owly probes.
- Fixed-version Owly 0.4.2 receive-process supervision, terminal consent,
  deadlines, signals, process groups and JSON terminal result.
- Private staging, path/type/link/size checks, no-overwrite rename, private
  file permissions and temporary runtime identity state.
- Qt strict-schema status provider and Settings page with manual read-only
  refresh and disabled GUI Receive action.
- Rust and Qt fixture tests. Fixtures do not emulate AWDL, Wi-Fi drivers, an
  iPhone or AirDrop RF behavior.

## Local validation

Environment: Codex Cloud Linux workspace on Debian, Rust/Cargo 1.85.0,
Python 3.12.14. The checkout is based on P018 merge
`a3448cc428ff1e8afe7c393ea3be109eefdea5dc`. Results below were run against
the final P019 source state, including the nonblocking terminal relay and
executable trust checks.

| Gate | Result |
| --- | --- |
| `cargo fmt --all -- --check` | PASS |
| `cargo clippy --workspace --all-targets -- -D warnings` | PASS |
| `cargo test --workspace --all-targets` | PASS, 225 tests (85 CLI unit, 25 BlueZ D-Bus, 15 CLI command, 3 discovery CLI, 1 inventory CLI, 29 discovery D-Bus, 66 core, 1 daemon CLI) |
| `cargo build --workspace` | PASS |
| `python3 -m unittest discover -s scripts/tests -v` | PASS, 55 tests |
| `git diff --check` | PASS |
| Qt 6.4.2 CMake/Ninja/CTest and QML smoke | Not available locally (`cmake`, `ninja`, `qmake6`, and `qtpaths6` absent); hosted CI required |
| Swift, CryptoKit, watchOS compile and packaging | Hosted macOS CI required |

The first parallel Rust workspace run had two transient private BlueZ test-bus
timeouts. Subsequent full workspace runs passed without test changes. The
latest run, after adding executable trust-mode and nonblocking terminal
descriptor coverage, passed all 225 tests. This is recorded rather than
hidden.

## Hosted CI

Both final-head workflows completed successfully for implementation commit
`939d4978d0fa33b9dfc8e27f382be646fa92f501`:

- [Branch push workflow](https://github.com/nghianguyen150612/OpenContinuity/actions/runs/43000306906): Rust checks, Qt Settings, and Swift/watchOS all passed.
- [Draft PR #20 workflow](https://github.com/nghianguyen150612/OpenContinuity/actions/runs/43000318545): Rust checks, Qt Settings, and Swift/watchOS all passed.
- Qt 6.4.2/Ninja ran all 5 CTest targets successfully, totaling 162 QtTest cases; the run includes the real QML window, navigation, and manual AirDrop refresh smoke test.
- Swift XCTest suites passed. Rust-to-Swift-to-Rust signed transport and CryptoKit verification passed; unsigned watchOS compilation and development artifact packaging also passed.

The first Qt run on `e172da6` found that clicking AirDrop's manual refresh
changed focus before the next keyboard navigation assertion. The UI test now
explicitly returns focus to the navigation control after refresh; the corrected
QML smoke test passes in both workflows above. The final validation-record
commit is documentation-only; its own push and PR checks are verified
separately from these implementation-source results.

## Physical qualification

Arch/RTL8852BE radio and real BlueZ/AWDL cleanup: NOT RUN. iPhone 13 receiver
listing and file transfer: NOT RUN. RTL8852BE support on the target laptop:
UNKNOWN. Native Wayland/Hyprland: NOT RUN. P010 Watch SE 2 W0–W6: NOT
RUN/BLOCKED and NO-GO. Native Apple Watch Auto Unlock: NO-GO.

Every software report keeps `authorization: never_evaluated` and
`physicalQualification: not_qualified`. No hosted test may promote physical
qualification.

## P019C cgroup foundation

P019C adds a Linux-only, testable cgroup v2 foundation, but keeps the
production Receive capability gate unconditionally false. The component is not
connected to Owly launch or publication. It requires an explicit exclusive
delegated subtree, verifies the parent migration boundary, places children
before exec, observes recursive delegated-root population, and uses bounded
`cgroup.kill`/empty checks. It does not recover a populated subtree after the
supervisor crashes; an external watchdog/service manager remains required.
Mock controllers test application policy; they do not prove kernel containment.

Final local gates passed: formatting, Clippy with warnings denied, workspace
build, whitespace check, **248 Rust tests passed / 1 ignored**, and **55 Python
tests passed**. The 13 new containment unit tests passed. The ignored test is
the opt-in real-kernel cgroup integration; it was not run. Qt 6.4.2/QML
and Swift/CryptoKit/watchOS remain required hosted CI gates. Only workflows
tied to the exact final SHA count as validation; the historical P019B green
runs do not validate P019C.

The current Debian 13 / kernel 6.18.57 host has no writable delegation and no
systemd manager. The opt-in cgroup integration test was NOT RUN. The aggregate
disk quota remains unresolved and blocks Receive. See
[the P019C foundation report](research/P019C_CGROUP_CONTAINMENT_FOUNDATION.md).

## P019D internal lifecycle

The internal coordinator acquires the actual cgroup lease before staging. It
settles the whole subtree before joining terminal relays and separately
verifies recursive emptiness before cleanup. Uncertain sessions preserve
staging and runtime identity. Successful offline sessions also remain
nonpublishable under the disabled crash supervision qualification gate.
Production CLI and Qt Receive remain disabled.

Local validation passed formatting, Clippy with warnings denied, workspace
build, diff checks, **255 Rust tests with 1 ignored**, and **55 Python tests**.
Seven coordinator tests passed, including abrupt subprocess exit fixtures.
The ignored real-kernel placement rejection test was **NOT RUN**. The host
has no exclusive delegation. **Crash supervision is NOT QUALIFIED**.
The fake controller checks lifecycle policy and retention. Qualified placement,
recursive kernel lifecycle tests and externally supervised crash recovery
require separate host provisioning. Hard aggregate disk quota remains an
independent activation blocker.

Hosted Rust, Qt/QML and Swift/CryptoKit/watchOS results are tracked on the push
and PR checks for the P019D commit. See the
[P019D lifecycle and crash supervision report](research/P019D_RECEIVE_LIFECYCLE_AND_CRASH_SUPERVISION.md)
for the kernel plan and placement limitation.

## P019E hard aggregate disk quota foundation

P019E adds a Linux XFS/ext4 project-quota capability and exclusive reservation
to the disabled internal lifecycle. It verifies the exact destination,
project inheritance, hard byte/inode limits and observable enforcement before
storage creation, accounts transient TLS state under the same root, and
revalidates quota identity after writer settlement. P019A's atomic bundle
rename remains same-filesystem and same-project. There is no userspace-only
fallback. Production containment and crash-supervision gates remain literal
false constants, and Qt Receive remains disabled.

The deterministic tests cover quota capability, identity, inheritance, limits,
capacity, ownership, retention, release and lifecycle retry policy through the
production coordinator. They are mock policy evidence only. The opt-in kernel
test requires a disposable, exclusive, externally provisioned project-quota
root; `/workspace` is overlayfs on this host, so **KERNEL NOT RUN**.

Local P019E validation on Rust 1.85.0 and Python 3.12.14: formatting PASS;
warnings-denied Clippy PASS; workspace build PASS; whitespace PASS; **265 Rust
tests passed / 2 ignored**; and **55 Python tests passed**. The ignored tests
are the real cgroup and real project-quota integrations. Qt 6.4.2/QML and
Swift/CryptoKit/watchOS toolchains are unavailable locally and remain hosted
CI gates. Final-head workflow URLs and hosted results are recorded in Draft
PR #20's description after hosted validation.

Project-quota provisioning, alternate-path write confinement, real cgroup
placement, external crash supervision, closed writer ingress and commit-time
consent remain independent activation blockers. See
[P019E hard quota foundation](research/P019E_HARD_DISK_QUOTA_FOUNDATION.md).

Final adapter review keeps ext4 capability unavailable: generic quota limits
and accounting do not independently prove enforcement. XFS is the implemented
enforcement candidate; its real-kernel qualification remains NOT RUN. A
regression also verifies runtime creation through the pinned quota descriptor.

## P019F deployment contract and qualification harness

P019F adds a non-production manager/launcher ownership contract, a read-only
host preflight and opt-in disposable-host runner. It expands the ignored XFS
test with sparse/open-unlinked accounting, retained allocation checks and
exhaustion accounting assertions. Reports distinguish MOCK_PASS,
HOST_PREFLIGHT_PASS, KERNEL_PASS, NOT_RUN, BLOCKED and FAILED, tied to exact
commit and sanitized host configuration. The runner never invokes Receive or
provisions host resources. All production gates remain false.

The current host preflight is BLOCKED. Both kernel tests are NOT_RUN. Successful
trusted placement, guardian crash/restart, write confinement, cross-project/mount
publication and quota configuration-change tests require additional trusted
host adapters and remain BLOCKED. P019F is PARTIAL; its offline contracts do
not qualify enforcement. Local results and final-head hosted workflows are
recorded in Draft PR #20. See the exact seven-boundary matrix in
[P019F qualification](research/P019F_TRUSTED_DEPLOYMENT_AND_KERNEL_QUALIFICATION.md).

Local P019F checks: formatting, warnings-denied Clippy, workspace build and
whitespace PASS; **265 Rust tests passed / 2 ignored** and **76 Python tests
passed** (21 new contract/harness tests). `--mock` emits MOCK_PASS only for
the offline suite, host preflight BLOCKED and kernel NOT_RUN. Qt/QML and
Swift/CryptoKit/watchOS remain hosted gates; those toolchains are unavailable
on this host. No real quota, cgroup, guardian or AirDrop case ran.
