# P019D Receive lifecycle and crash supervision

## Status

The internal Linux lifecycle composes the existing Receive supervisor, cgroup
lease and private staging. The production `writer_containment_available()`
function always returns false. The CLI checks it before cgroup acquisition,
staging creation or Owly Receive launch. Qt Receive remains disabled.
Successful offline sessions are also blocked from publication by the separate
crash supervision qualification gate, a constant set to false.

## Why the lifecycle changed

At the P019C baseline, the inactive Receive path spawned Owly directly,
then armed staging retention and installed cancellation handlers. Error paths
stopped the direct child. Reaping that process could leave forked descendants
running, including descendants that created another process group or session.
A survivor could continue writing with the staging path or an open descriptor.
A survivor could also keep a terminal pipe open even after the leader exited.
Source inspection found Tokio tasks and threads in Owly 0.4.2's Receive path.
The source inventory cannot constrain a compromised backend.

The same problem applied to launch bookkeeping errors, partial relay setup,
signals received during startup, cancellation, expiry, output failure or
overflow, staging limit failures, and nonzero exit. A supervisor killed during
fork or exec, polling, termination, or finalization cannot run its cleanup
code. Cgroup kill can fail, and a task can remain stuck after a kill request.
A delegated process can migrate within the subtree, and any process or
manager with sufficient permissions can move it across the boundary. An
external same-UID process could also receive a staging descriptor or be asked
to write via an IPC service. Recursive cgroup emptiness only accounts for
writers confined to that boundary.

## Internal lifecycle

1. The CLI validates arguments, explicit consent, terminal access and the
   read-only backend and radio checks. It holds the existing private singleton
   lock. After the disabled production gate, it resolves the fixed backend,
   registers both cancellation signals with scoped cleanup, and assigns the
   session deadline. The deadline gates the final decision as well as active
   polling. Userspace deadlines cannot interrupt a stuck launch or kernel
   syscall. That failure mode needs independently managed stop and recovery.
2. The coordinator acquires and holds the actual delegation reservation. It
   creates the active session before allocating staging or runtime identity.
   It checks consent interruption before acquisition and again before launch.
   Both filesystem resources are marked for retention before launch.
3. The real adapter consumes the launch command while its private placement
   descriptor is live. The pre-exec hook must succeed before the executable
   can run. P019D continues to use the existing fixed Owly arguments, private
   runtime, per-file limit and bounded terminal relays. Relay setup and read
   failures are reported. Nonblocking pipe readers can stop after the shared
   relay deadline even if a writer holds a pipe open.
4. The coordinator settles the whole delegation before waiting for pipe EOF
   and child reaping. A surviving descendant, cancellation, timeout or
   unsuccessful execution triggers subtree kill and a bounded empty check.
   An initial observation error also triggers a bounded stop attempt and
   retains the uncertain outcome. Once reaping and relays finish, a separate
   recursive population read must report zero. Unexpected repopulation is
   a containment failure even if a further stop succeeds.
5. Cgroup cleanup and reservation release must succeed before the coordinator
   discards private data. If either fails, or the child or relays cannot be
   joined, the coordinator retains staging and runtime identity. A clean
   failure can discard both. An otherwise successful but unqualified session
   retains both and returns failure with zero published files and bytes.
   A stale owner reservation blocks reacquisition and requires inspection.
   No restart path reuses or removes preserved data automatically.

Publication also requires a successful backend exit, unexpired consent,
no cancellation, relay completion within the output cap, staging limits and
validation, verified writer quiescence, and a successful no-overwrite atomic
rename. The existing P019A storage primitive performs final limits and
validation before the single bundle rename. P019D cannot call it for a
publishable session. Before activation, the external guardian must hold
exclusive writer ingress closed across final validation and rename, and the
publisher must recheck consent at the commit boundary. Atomic visibility does
not make the rename crash durable. Strict Owly doctor validation still applies.

## Placement constraint found in P019C

Linux checks write access to both the destination `cgroup.procs` and the
source/destination common ancestor's `cgroup.procs`. Checking only the
delegation's immediate parent misses moves whose common ancestor is higher.
P019D preflight walks pinned cgroup directory descriptors to the visible
hierarchy root and requires write-open denial for every ancestor. It rejects
partial mount views it cannot traverse. This is a conservative check in the
current mount and credentials, with stable host provisioning assumed.

The check also exposes a placement conflict. An unprivileged child starting
outside that boundary cannot move itself in when it has the same denied
authority. Pre-exec failure stops launch. Granting the backend those ancestor
permissions would reopen migration. A qualified deployment needs a trusted
launcher that can start the process inside the boundary and remove its
extra authority before backend exec, independently of the supervisor's
credentials. The ignored kernel test now checks rejection of outside
same-UID placement under these conditions. It does not certify a placement
method for a working receiver.

## Crash supervision decision

| Mechanism | Role and limit |
| --- | --- |
| Manager-owned service or unit | Preferred deployment candidate. A privileged manager owns placement and subtree cleanup. A systemd service with a defined main process, control-group kill and bounded stop escalation is one candidate. The test must establish actual main-process loss handling and recursive emptiness, including after supervisor SIGKILL. A scope alone may outlive its original caller. |
| Independently supervised guardian | Candidate where the manager keeps a separately credentialed guardian alive outside the backend kill subtree. It owns verified control descriptors, polls the supervisor identity and treats guardian loss as a manager stop condition. A supervisor child can die alongside the supervisor or be killed by the backend. |
| pidfd death notification | The external guardian can poll a pidfd obtained at supervisor launch. Readability identifies that supervisor's exit without PID reuse. A pidfd observes one process. Subtree ownership, kill and repeated population checks still come from cgroups and the manager. |
| Process groups and parent death signal | Useful for limited process management. Descendants can leave a process group. A parent death signal applies to the configured process, so it provides no recursive proof and cannot substitute for external supervision. |

A future host implementation needs the following contract:

- The manager binds one supervisor identity and one exclusive writer subtree to
  the reservation. The manager or launcher places workers before exec and
  closes access to privileged descriptors and ancestor controls. The
  supervisor operates outside the writer subtree. Kill never targets a numeric
  PID or PGID reconstructed at restart.
- Credentials and access control isolate the backend from its guardian and
  control plane. A same-UID backend can normally signal other processes with
  that UID. Depending on host policy it may inspect or manipulate processes,
  alter same-user control files, or use a user manager or other IPC service to
  run work outside the subtree. The backend must lack access to those controls
  and to IPC or descriptor recipients able to act as external writers.
- On SIGKILL, unexpected exit, IPC loss or failed startup, the manager stops
  the entire owned subtree and verifies recursive emptiness with a deadline.
  If it loses authority, cannot identify the original lease, or cannot prove
  emptiness, it records an incomplete cleanup and leaves data private. On
  restart it revalidates the lease identity before taking any action. It
  never treats a stale name as sufficient authority or resumes publication.
- A compromised backend must be unable to signal the guardian, change the
  containment controls it relies on, or defeat the manager's own stop
  behavior. Test this with isolated credentials and a disposable manager
  service on a separately provisioned host.

P019D implements the local coordinator and cgroup controller interface.
It ships no guardian, manager service, privileged launcher or recovery
automation. The development host exposed cgroup v2 at `/sys/fs/cgroup` with
membership `0::/`. UID 1000 lacked write access to the root controls, and
PID 1 was `tail`. No delegated subtree or service manager was available.

## Verification plan

- **Mock PASS.** The coordinator tests run real local child and relay code
  with a fake containment controller. They cover success with publication
  blocked, placement failure, cancellation, expiry, nonzero exit, output
  cap and relay failure, surviving and migrated descendants, kill and
  observation failures, stuck population, repopulation, and cleanup failure.
  Subprocess exits without unwinding model startup, active launch boundary
  and finalization crashes. They check retained directories, stale owner
  reservations and retry refusal. They do not test manager action on death.
- **Kernel NOT RUN.** The ignored placement test requires both opt-in with
  `OPENCONTINUITY_RUN_CGROUP_INTEGRATION=1` and an explicit exclusive
  `OPENCONTINUITY_AIRDROP_CGROUP_ROOT`. A qualified-host extension must use
  a trusted launcher and owned inert fixtures only. First confirm child
  placement before exec, then leave a descendant alive after leader exit
  and move a fixture within the delegation. Check that root population stays
  visible and subtree kill clears it. Repeat with cancellation during a
  write, a descendant holding a pipe, and expiry. Check retained data on
  incomplete cleanup. Run only after independently verifying the exclusive
  subtree and verifying the host can keep unrelated tasks out of it.
- **Crash supervision NOT QUALIFIED.** On a separately provisioned host,
  kill the supervisor during startup, active writing, and finalization.
  Confirm identity-bound guardian detection, recursive kill and bounded
  empty verification before any cleanup. Exercise guardian death, restart,
  stale leases, and refusal to reuse preserved staging. No kernel or guardian
  scenario ran here.

Hard aggregate disk quota is an independent activation blocker. The current
aggregate scanner polls usage and allows temporary disk consumption beyond the
limit between scans. Receive activation also requires real kernel and manager
qualification and completion of the publication commit boundary above.

P019E adds an inactive project-quota lease to this coordinator. Staging and
runtime state are created only after an externally provisioned XFS/ext4 hard
block/inode limit is verified, and quota identity is checked again after
writer settlement. This removes polling as the proposed storage boundary but
does not qualify kernel enforcement on the current overlayfs host or prevent
the backend from using alternate writable paths. See
[P019E hard quota foundation](P019E_HARD_DISK_QUOTA_FOUNDATION.md). The trusted
launcher, crash guardian, write-confinement and commit-boundary requirements in
this document remain unchanged.

`authorization: never_evaluated` and `physicalQualification: not_qualified`.
Physical RTL8852BE AWDL and iPhone 13 AirDrop are NOT RUN. Watch SE 2 W0–W6 is
NOT RUN/BLOCKED. Native Apple Auto Unlock is NO-GO.

## References

- [Linux cgroup v2 delegation, migration, population and kill](https://docs.kernel.org/admin-guide/cgroup-v2.html)
- [Linux pidfd_open and poll semantics](https://man7.org/linux/man-pages/man2/pidfd_open.2.html)
- [Linux kill permission rules](https://man7.org/linux/man-pages/man2/kill.2.html)
- [Linux parent death signal](https://man7.org/linux/man-pages/man2/PR_SET_PDEATHSIG.2const.html)
- [systemd service lifecycle](https://www.freedesktop.org/software/systemd/man/latest/systemd.service.html)
- [systemd kill and escalation](https://www.freedesktop.org/software/systemd/man/latest/systemd.kill.html)
