# P019C delegated cgroup containment foundation

## Status and scope

This phase adds a Linux cgroup v2 foundation that can be compiled and tested
without access to cgroups. It is deliberately **not wired into the experimental
Receive supervisor**. `writer_containment_available() -> false` remains in
force; Receive still exits before staging creation or Owly startup. Passing
mock tests or compiling the module does not establish host containment.

The foundation is suitable for further review and integration only when an
operator explicitly supplies a dedicated delegated cgroup v2 subtree. The
current Codex host has no such delegation, so the real-kernel test remains
NOT RUN. This is not production-ready containment.

## Implemented component

Linux-only `airdrop_containment.rs` provides:

- Explicit root selection through
  `OPENCONTINUITY_AIRDROP_CGROUP_ROOT`; there is no hierarchy-root discovery
  fallback, sudo, systemd invocation, mount operation or privilege change.
  Preflight validates the empty subtree, creates and removes a temporary owner
  marker, and reports unavailable if cleanup fails. A future production
  supervisor must repeat validation and hold the owner reservation throughout
  its receive session.
- The configured path is opened component-by-component with `openat` and
  `O_NOFOLLOW`, then pinned by a directory descriptor. Preflight checks the
  cgroup2 filesystem type, effective-user ownership, no group/world write bits
  on the root and required control files, and effective-user access to
  `cgroup.procs`, `cgroup.events`, `cgroup.controllers`, and `cgroup.kill`.
- The supervisor's current cgroup is resolved from `/proc/self/cgroup` and
  cgroup2 mountinfo, then compared with the pinned delegation by device/inode
  while walking only within the mounted hierarchy. The parent `cgroup.procs`
  must reject a write-open; this verifies the DAC boundary required to prevent
  unprivileged migration across the delegation boundary.
- An empty-root check and an atomic owner cgroup directory. The fixed owner
  child prevents a second OpenContinuity instance from reserving the same root
  concurrently. If a process crashes, the marker is left behind; recovery
  requires operator inspection and cleanup. The module does not guess that a
  stale reservation is safe to remove.
- A fixed active-session cgroup created with exclusive `mkdirat` relative to
  the pinned owner descriptor. This atomic child reservation permits only one
  active session and leaves a stale session fail-closed after a crash. Every
  control operation is relative to an open cgroup directory descriptor after
  initial validation. Duplicated descriptors are explicitly marked
  close-on-exec so the backend cannot inherit controller handles.
- A pre-exec hook that writes `0` to the session's pinned `cgroup.procs`
  descriptor. Failure aborts exec; Owly cannot run outside the intended
  session cgroup.
- Recursive population observation at the delegated root. Monitoring only the
  session node would miss a process moved to another cgroup inside the
  delegation. Cancellation, expiry, unsuccessful leader exit, or any
  unexpected remaining population invokes `cgroup.kill` on the whole
  delegated subtree, then polls `cgroup.events` for `populated 0` for at
  most ten seconds.
- Cleanup only after the whole delegated tree is empty. Nested empty cgroups
  are removed using descriptor-relative operations. Failed kill, observation,
  timeout or cleanup returns a retained error; Drop does not signal processes
  or delete cgroups. The only successful lifecycle outcome is a successful
  direct child exit with the whole root already empty. Killing unexpected
  descendants yields a stopped outcome, which cannot qualify for publication.

The state machine has a `Controller` seam. Fake controllers exercise the
application's state transitions. They do not emulate or prove cgroup kernel
semantics.

## Security assumptions and escape limits

The operator must provision an exclusive delegated subtree whose parent
`cgroup.procs` is inaccessible to the delegatee. The supervisor must run
outside that subtree. Cgroup v2 migration checks require write access to the
destination `cgroup.procs` and to the source/destination common ancestor's
`cgroup.procs`; the parent permission check is intended to enforce the
outside boundary. The delegation root must start empty and be used only by this
session.

Processes may migrate among cgroups *inside* a delegated subtree. Therefore the
component monitors and kills the entire subtree, not just its session child.
A process cannot migrate out across the delegation boundary under the
documented cgroup v2 DAC rules unless it gains access to the ancestor control
file, receives extra privilege, or an external manager moves it. Those cases
are outside the guarantee and must fail closed operationally.

Supervisor crash recovery is not implemented. If the supervisor is killed
while Owly or a descendant remains, cgroup v2 does not automatically kill that
process. The owner marker remains, and a subsequent OpenContinuity acquisition
fails closed while the delegated root is populated or reserved. That protects
a later session from reusing the tree, but it does not stop the old writer or
prevent further writes to its staging directory. A manager-owned transient unit
or another external watchdog that kills the complete delegated subtree on
supervisor loss is required before Receive can start. No PDEATHSIG or leader
pidfd is presented as a descendant cleanup guarantee.

This component does not isolate the backend from the cgroup filesystem, hide
system/user buses, sandbox code, or defend against a privileged actor. Owly
0.4.2's inspected receive path creates Tokio tasks and blocking threads but no
separate OS child. That source inventory does not protect against compromised
code parsing untrusted transfers. Until the Receive supervisor calls this
component and verifies all other publication conditions, no receive path may
use it to authorize publication.

The module does not apply memory, PID, I/O or disk limits. In particular, the
staging aggregate-size scanner remains approximate. A hard aggregate disk
quota remains an independent release blocker. Qt Receive remains disabled.

These assumptions follow the Linux kernel's documented cgroup v2 rules:
writing `0` to `cgroup.procs` moves the writing process, migration permission
depends on the destination and common-ancestor control files, `cgroup.events`
reports recursive `populated` state, and `cgroup.kill` kills processes in the
target cgroup and descendants. See the [kernel cgroup v2 documentation](https://docs.kernel.org/admin-guide/cgroup-v2.html).

## Host capability findings

On 2026-11-17 the available Debian 13 / kernel 6.18.57 environment exposed
cgroup v2 at `/sys/fs/cgroup`, but `/proc/self/cgroup` reported `0::/`.
The root and control files were owned by uid 0; `cgroup.procs` was mode 0644
and `cgroup.kill` mode 0200, so uid 1000 could not use the required controls.
PID 1 was `tail`, not systemd; no systemd user bus or
`XDG_RUNTIME_DIR` was available. No cgroup, service or mount was changed.
Consequently this environment cannot run the real-kernel integration test.

No Linux configuration is qualified or supported for Receive yet. A candidate
host must provide all preflight checks above, including writable `cgroup.kill`
(the kernel must expose this file), an explicitly delegated cgroup v2 subtree,
the supervisor outside it, and a protected parent migration boundary. The code
rejects unavailable, namespaced or otherwise unverifiable configurations.
This foundation has not been qualified on any real delegated host.

## Tests

The focused containment module suite passed 13 tests; one opt-in real-kernel
integration test was ignored. Existing P019A failed-session and known-writer
publication regressions also remain in the workspace suite. Coverage includes
invalid or missing capability, ownership/mode validation, parent migration
boundary, actual `Command::spawn` pre-exec placement success/failure,
create/launch failure state, cancellation, expiry, unsuccessful exit, leader
exit with surviving/migrated descendants, subtree kill failure, observation
errors, population timeout, cleanup failure, publication eligibility, and
repeated state-machine sessions.

The ignored integration test runs only with both
`OPENCONTINUITY_RUN_CGROUP_INTEGRATION=1` and an explicitly supplied
`OPENCONTINUITY_AIRDROP_CGROUP_ROOT`. It creates one owner marker and session
under that root, starts only `/bin/sh -c 'exit 0'` after pre-exec placement,
verifies empty population and removes only its owned cgroups. It never creates
the delegated root or changes host configuration. It was NOT RUN here.

Mock results validate Rust policy and error handling only. They do not prove
that a host's cgroup delegation or kernel behavior passes preflight.

## Required work before Receive activation

Before any future integration can replace the false gate:

1. Review this component on a host with a separately prepared exclusive cgroup
   v2 delegated subtree; run the opt-in kernel test and record the exact kernel,
   mount/delegation layout and results.
2. Add real-helper lifecycle integration cases for a leader with descendants,
   cancellation/expiry, subtree kill, and confirmed empty population.
3. Integrate the component with the production supervisor without opening any
   path before capability creation and child placement are verified.
4. Tie `QuiescentSuccess` to the existing consent, deadline, output-relay,
   receive-limit and staging-validation gates. Any uncertainty must preserve
   staging and block publication.
5. Add crash supervision that kills the complete delegated subtree if the
   OpenContinuity supervisor disappears, and validate it with real-kernel tests.
6. Independently solve hard aggregate disk-consumption protection before
   enabling the experimental CLI or Qt Receive.

Always preserve `authorization: never_evaluated` and
`physicalQualification: not_qualified`. RTL8852BE AWDL and iPhone 13
AirDrop remain NOT RUN; Watch SE 2 W0–W6 remains NOT RUN/BLOCKED, and native
Apple Auto Unlock remains NO-GO.

## P019D follow-up

P019D connects this foundation to the internal Receive coordinator while
preserving the disabled production gate. It also corrects the placement
assumption above. Migration checks apply at the common ancestor, which may
be higher than the parent. With all visible ancestors protected, an ordinary
same-UID child outside the delegation cannot write itself into the subtree.
The ignored kernel test now checks that rejection. Positive placement and
descendant lifecycle tests require a trusted external launcher. See
[P019D lifecycle and crash supervision](P019D_RECEIVE_LIFECYCLE_AND_CRASH_SUPERVISION.md).
