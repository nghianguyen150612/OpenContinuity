# P019G trusted manager prototype and publication boundary

## Status

P019G is **PARTIAL**. The inactive coordinator now retains its real
containment and project-quota lease objects through the entire final staging
scan, permission pass and atomic bundle rename. A non-copyable internal
`PublicationSeal` borrows those live handles and revalidates them at each
`renameat2(RENAME_NOREPLACE)` attempt. Consent, cancellation and deadline are
checked at that boundary. A rootless `p019g-manager` prototype provides
authenticated local process IPC, identity/epoch sequencing and pidfd process
observation. It is deliberately not connected to Owly or Receive.

The prototype does not provide an externally trusted host manager, recursive
cgroup authority, trusted placement, a guardian-of-the-guardian, write
confinement or authority for publication. Consequently it cannot satisfy the
active trusted-manager requirement for a successful Receive commit. The two
production gates remain unchanged: `writer_containment_available()` returns
literal `false`, and `CRASH_SUPERVISION_QUALIFIED` remains `false`. Qt Receive
is disabled. No production bypass was added.

## Publication boundary correction

Previously the coordinator verified an empty subtree, called containment
cleanup and released its exclusive reservation, then scanned and renamed the
staging tree. Project quota remained reserved, but the writer boundary did
not. A cancellation/deadline check before `finish_staging()` also preceded a
potentially lengthy tree scan, permission changes and up to eight collision
attempts. That check could not authorize a later rename.

The coordinator now keeps both live lease objects through finalization. The
private `PublicationSeal` borrows the containment controller and session, the
quota lease, the staging identity/path and the current admission state. It
cannot be copied or constructed from a serialized observation. Immediately
before each actual rename syscall it checks consent, cancellation and deadline,
requires a fresh recursive empty observation, revalidates quota enforcement
and identity, and verifies staging remains under the expected project. The
filename is generated before this check. Between the final authorization and
`renameat2` there is only the bounded atomic commit-state transition and the
syscall. `RENAME_NOREPLACE` preserves the one-directory, no-overwrite
publication semantics; a collision triggers a new full authorization check.

The atomic commit gate has one linearization point: the successful
`OPEN -> ADMITTED` compare-and-swap immediately before a rename attempt. A
cancellation signal that changes the state first prevents the syscall. If
admission wins first, that already-authorized attempt may complete; a
successful rename changes the gate to `COMMITTED`, and a later cancellation
cannot retract a committed bundle. If an admitted attempt collides and
cancellation arrives before the next attempt, the next authorization fails.
The current CLI consent grant is immutable for an invocation; SIGINT/SIGTERM
is the revocation event. No interactive consent revocation mechanism exists.
The current monotonic deadline is rechecked at every attempt.

On rename failure, validation error or revoked authorization, finalization
returns before containment cleanup/release. Only after the operation has
definitively returned does the coordinator settle and release containment.
Quota remains held through rename or failure and is released after a fresh
post-finalization validation. A release failure after a successful rename
cannot roll back the visible bundle; it reports incomplete cleanup and leaves
the stale reservation to block reuse. Crash durability is not implied by the
atomic rename.

These offline tests use a real local filesystem rename and fixture lease
objects. They establish application ordering and policy only. They do not
prove kernel cgroup containment, trusted guardian authority, staging
immutability against a same-UID process, or production publication safety.
Subprocess crash tests exit after the final staging scan but before commit
admission, and immediately after a successful rename/committed transition.
They verify that the first case leaves staging unpublished and the second
leaves one complete bundle visible, with fixture ownership markers retained
after either abrupt exit. The commit gate itself is process-local and is not
claimed to survive a supervisor crash.

## Rootless manager/guardian prototype

`apps/opencontinuity-cli/src/bin/p019g-manager.rs` is an independently
executable Linux prototype. The `serve` mode runs as a separate process from
the supervisor whose PID is supplied by its trusted caller. It opens and holds
a real pidfd for that supervisor, binds an explicitly identified instance,
session and manager epoch, and accepts only a private Unix socket under a
same-UID directory with no group/world permissions. Each connection is
authenticated using kernel `SO_PEERCRED`; both PID and configured UID must
match. Identity values in a frame never replace peer credentials.

Messages are fixed 64-byte binary frames with magic, protocol version,
operation, 16-byte instance/session/epoch identifiers and a strictly increasing
sequence. The expected sequence persists across reconnects. Unknown versions,
wrong identities, replay, reordering and wrong kernel peer credentials fail
closed. Socket reads/writes have one-second timeouts, and supervisor death is
polled through the pidfd at a 20 ms interval. A 0600 reservation marker is
created atomically and deliberately left behind on failure or supervisor loss;
the prototype never treats an old pathname as recovery authority. A manager
restart with the old reservation fails closed.

Ordinary protocol tests run as an unprivileged user and use harmless shell
processes. They exercise normal exit, SIGKILL, pidfd detection, wrong PID
credentials, version/identity/sequence rejection, supervisor survival after
guardian death, and stale reservation refusal. A descendant survival test
confirms only that pidfd observation of one process says nothing about a
grandchild. It makes no containment claim.

The prototype holds no cgroup or quota descriptors, does not create or place
the supervisor, does not transfer a pidfd over IPC, and does not return a
publication capability. Its caller supplies the supervisor PID and expected
UID; integration with a trusted service manager would need to bind these to
its own spawn result and credentials. An unexpected guardian exit has no
external recovery process here. The marker intentionally remains stale for
manual validation.

## Trusted placement and cgroup permission model

An unprivileged process cannot migrate itself into a protected delegated
subtree if it lacks the required write access. For cgroup v2 migration, Linux
requires write access to the target `cgroup.procs` and to the `cgroup.procs`
of the common ancestor of source and destination. Giving the backend ancestor
migration access also gives it an escape route. `clone3(CLONE_INTO_CGROUP)`
can create a child directly in a cgroup-v2 target using a pinned cgroup
directory descriptor; the kernel documentation states that the ordinary
cgroup placement restrictions still apply. A trusted manager/launcher could
hold that descriptor, create the worker inside the boundary before exec, then
close privileged descriptors and drop credentials before untrusted code runs.
The supervisor and backend must never receive ancestor migration controls.

This repository does not implement clone3 placement or a manager-owned
launcher. The host has no exclusive delegated subtree, and the prototype has
no cgroup handle. Placement is **BLOCKED**. There is no root or capability
grant to Owly and no host cgroup configuration change.

## Filesystem and IPC sandbox boundary

The minimum future Owly profile must permit its fixed executable, dynamic
loader and required shared libraries read-only; read-only resolver/runtime
configuration as required by its mDNS/network code; UDP multicast/DNS-SD and
TCP sockets on the already-prepared AWDL interface; and writes only below the
project-quota-backed staging and per-session runtime directories. The
supervisor must preserve only reviewed stdin/stdout/stderr and required
network descriptors. Owly does not need access to user/system D-Bus, a systemd
user manager, shell execution, host cgroup controls, mount administration,
quota ioctls, ptrace, or arbitrary local Unix sockets based on the inspected
fixed CLI path. `XDG_STATE_HOME` points at session runtime state.

A qualified launcher should use a distinct backend UID/GID, close all other
descriptors, drop supplementary groups and capabilities, disable core dumps,
and establish a mount namespace where the executable/libraries and host
configuration are read-only while the two approved writable roots are on the
configured XFS project. It must account for `/tmp`, `/var/tmp`, `/dev/shm`,
procfs, inherited file descriptors, Unix sockets, alternate mounts and IPC
brokers. Landlock can add path-based access restrictions on supported kernels;
seccomp can reduce syscall surface only after verifying compatibility with
Rust, mDNS and socket setup. Neither substitutes for network/IPC isolation,
mount setup, UID separation or descriptor allowlisting. A new network namespace
would hide the existing `awdl0` interface unless a trusted, separately
qualified network design provides it, so it cannot be assumed compatible.

No enforced sandbox profile is implemented or tested in this phase. The
existing project quota limits allocation charged to that project; it does not
stop alternate-path writes. Sandbox qualification is **BLOCKED**.

## P019F radio-guard failure investigation

The first-attempt Rust check failures on 2026-11-13 were traced to the
`radio_guard::tests::guard_excludes_only_same_adapter_and_releases_on_drop`
test. The runner log archive is no longer readable from this environment, so
the exact emitted panic line cannot be confirmed. Source review found no async
release path: the guard is one abstract Unix listener, and dropping its last
listener closes the socket before the next synchronous bind. The test did use
a PID-only namespace, and its final reacquisition assertion immediately
dropped the returned listener, making it less explicit about its lock lifetime.

The regression now adds a random per-test suffix, holds the reacquired
listener, proves another bind is rejected while held, then drops it before
reacquiring again. It contains no sleep or retry. This isolates radio-guard
exclusion from Receive lifecycle and narrows test-namespace collisions. The
missing runner log means this is a regression improvement, not a confirmed
root cause for the historical failure. Current full Rust checks must pass on
the new commit before the issue is considered closed.

## Qualification status

| Boundary | P019G result |
| --- | --- |
| Offline protocol | PASS for fixed-frame parsing, real peer credential checks, epoch identity and replay/order policy |
| Publication policy | PASS for fixture lease ordering through a real local `RENAME_NOREPLACE` bundle commit |
| Real process lifetime | PASS for local supervisor/guardian processes and pidfd observation |
| Kernel containment | NOT_RUN; no exclusive delegated cgroup subtree or manager-owned placement |
| Kernel quota | NOT_RUN; no disposable XFS project quota host |
| Filesystem/IPC sandbox | BLOCKED; no launcher or enforceable profile |
| Production Receive | BLOCKED; no external trusted manager/guardian seal, placement, descendant containment, sandbox or qualified quota |
| Physical qualification | NOT RUN; `authorization: never_evaluated`, `physicalQualification: not_qualified` |

Local software gates: formatting PASS, Clippy with warnings denied PASS,
workspace build PASS, workspace tests PASS (279 passed / 2 ignored), Python
tests PASS (76), and `git diff --check` PASS. The two ignored Rust cases are
the opt-in cgroup and project-quota kernel tests; neither was run.

RTL8852BE AWDL / iPhone 13 AirDrop: NOT RUN. Watch SE 2 W0–W6:
NOT RUN/BLOCKED. Native Apple Auto Unlock: NO-GO.

## References

- [Linux cgroup v2 delegation and migration](https://docs.kernel.org/admin-guide/cgroup-v2.html)
- [clone3 `CLONE_INTO_CGROUP`](https://man7.org/linux/man-pages/man2/clone3.2.html)
- [Linux pidfd process identity](https://man7.org/linux/man-pages/man2/pidfd_open.2.html)
- [Linux Landlock documentation](https://docs.kernel.org/userspace-api/landlock.html)
- [Linux capabilities](https://man7.org/linux/man-pages/man7/capabilities.7.html)
