# P019B process containment assessment

## Decision

P019B does not enable receive. The existing
`writer_containment_available() -> false` gate remains in place because this
checkout's nonprivileged runtime has no delegated containment boundary and no
system service manager that can create one. No process is started and no
staging is created when this gate blocks the receive command. This is an
evidence-backed result for the available privilege model, not a claim that
Linux cannot provide containment when an administrator or service manager
delegates it.

The process-group implementation cannot prove writer quiescence. Reaping the
leader, sending signals to a PGID, or checking a PGID does not account for
escaped descendants or PID/PGID reuse. The P019A publication gate remains
fail-closed.

## Host observations

Observed in the Codex Cloud Linux environment on 2026-11-17:

- Kernel exposes a cgroup v2 mount at `/sys/fs/cgroup`; `/proc/self/cgroup`
  reports `0::/`. The process is at the visible hierarchy root.
- The mount root, `cgroup.procs`, `cgroup.subtree_control`, and
  `cgroup.kill` are root-owned. The effective user cannot write any of them
  or create a child cgroup. There is no delegated subtree.
- PID 1 is `tail`, not systemd. `systemctl --user` reports no user bus,
  `systemctl` reports that systemd is not the init system, and
  `XDG_RUNTIME_DIR` is unset.
- The current process has no supplementary group or elevated capability that
  grants cgroup control.

These checks are read-only. No host service, cgroup, mount, radio or network
configuration was changed. A writable cgroup v2 mount alone is not delegation.

## Mechanism evaluation

### Cgroup v2

The kernel's cgroup v2 documentation describes the required basis: all
processes have one cgroup; forks inherit their parent's cgroup; a delegated
sub-hierarchy can be managed by its delegatee; `cgroup.events` reports
recursive `populated`; and `cgroup.kill` kills a cgroup subtree while
handling concurrent forks. The kernel also documents the delegation boundary:
a delegatee cannot migrate processes into or out of the delegated subtree,
because migration requires write access both to the destination
`cgroup.procs` and to the source/destination common ancestor's
`cgroup.procs`.

A viable implementation would need an **exclusive delegated subtree** owned by
the supervisor, with an inaccessible parent `cgroup.procs`. The supervisor
must be outside the subtree it monitors. It must verify delegation by opening
the required control files and creating a child cgroup, place the backend
before exec, monitor recursive population, invoke subtree kill on cancellation
or unexpected descendants, wait with a deadline for `populated 0`, then
remove session cgroups. Any failed check or cleanup must preserve staging and
block publication.

The delegation itself is essential. A process group is not a substitute.
A delegated cgroup does not constrain processes to one session cgroup: a
delegatee may move processes within its delegated subtree. Therefore an
exclusive whole-subtree boundary must be monitored and killed. The guarantee
also assumes the kernel-enforced delegation boundary is intact and that the
supervisor's cgroup controls are not exposed to an unrelated actor. A process
with additional privilege, access to an ancestor cgroup control file, or an
external service able to move it outside the delegation can escape. P019B
does not claim protection against those cases.

The host has no usable delegation, so neither creating a cgroup nor placing
Owly into one can be performed without an external configuration change or
privilege. The task explicitly disallows root escalation and host cgroup
changes. Attempting to write to the root-owned control files would be
unsupported and is not part of this change.

### systemd transient units

The systemd documentation says `systemd-run --scope` creates a transient
scope whose processes are managed by the service manager, and `Delegate=`
establishes an owned subtree for a unit. Either could be part of a deployment
that supplies and verifies the appropriate manager-owned unit and lifecycle.
A user scope alone is not accepted as an escape-proof boundary: user-manager
availability and permissions vary, and a same-user process can request work
from its user manager. We have no running systemd manager or user bus here.
Starting or configuring system services would also exceed this task's
nonprivileged, no-host-change boundary.

### Pinned Owly process model

The pinned source is t4t5/owly commit
`6f2e8ea99aed0687558d78a1dd49154ab3a5fa76`, version 0.4.2. In the receive
path, `owly-cli/src/airdrop.rs` starts Tokio tasks for mDNS and the receiver;
`owly-airdrop/src/driver/receiver.rs` starts Tokio tasks per accepted
connection; staging uses Tokio blocking tasks/threads. The inspected receive
path does not launch a separate OS child process. These are threads/tasks in
the Owly process, so reaping that process normally ends them. This source
inspection is useful evidence, but not an independently enforceable boundary:
a defect or compromise in code parsing untrusted input could execute arbitrary
same-UID code, including code that forks or attempts cgroup migration.

A pidfd would improve identity-safe signaling of the one direct child but
would not contain descendants. It cannot replace delegated cgroup ownership.

## Publication and cleanup

Publication remains eligible only when backend success, valid consent,
unexpired/noncancelled session, successful output relays, satisfied receive
limits, verified writer quiescence and staging validation are all established.
The current runtime cannot establish writer quiescence, so receive is blocked
before Owly launch. Unknown writers must never be cleaned up or published.
Atomic bundle rename semantics from P019A do not weaken this gate.

Cgroup process containment would not impose a disk quota. The staging-size
scanner remains approximate; aggregate disk-consumption protection remains
unresolved. Qt Receive stays disabled.

## Validation boundary

This environment cannot run a real delegated-cgroup integration test because
no delegated subtree exists. P019A's deterministic software tests cover the
fail-closed gate and known-writer publication blocking. P019C adds fake
controller state tests plus an ignored opt-in integration test, but neither
mock tests nor source tests exercise kernel semantics unless that integration
test runs inside a separately prepared, exclusive delegated subtree.

No physical radio or Apple-device behavior is tested here. All reports retain
`authorization: never_evaluated` and `physicalQualification: not_qualified`.
Real RTL8852BE AWDL and iPhone 13 transfer remain NOT RUN. Watch SE 2 W0–W6
remains NOT RUN/BLOCKED; native Apple Auto Unlock remains NO-GO.

## Primary references

- Linux kernel, [Control Group v2](https://docs.kernel.org/admin-guide/cgroup-v2.html):
  process membership/inheritance, delegation, migration permissions,
  `cgroup.events`, and `cgroup.kill`.
- systemd, [systemd-run](https://www.freedesktop.org/software/systemd/man/latest/systemd-run.html):
  transient service and scope lifecycle.
- systemd, [systemd.resource-control](https://www.freedesktop.org/software/systemd/man/latest/systemd.resource-control.html):
  `Delegate=` and `DelegateSubgroup=`.
- Owly source at the pinned commit above:
  `crates/owly-cli/src/airdrop.rs`,
  `crates/owly-airdrop/src/driver/receiver.rs`, and
  `crates/owly-airdrop/src/driver/staging.rs`.


## P019C follow-up

P019C adds an isolated cgroup v2 foundation and fake controller-state tests,
while retaining the false Receive gate. It does not change the P019B host
finding or claim the ignored real-kernel integration was run. See
[P019C foundation](P019C_CGROUP_CONTAINMENT_FOUNDATION.md).
