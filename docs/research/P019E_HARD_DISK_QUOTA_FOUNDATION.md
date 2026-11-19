# P019E hard aggregate disk quota foundation

## Status

P019E implements an offline Linux project-quota capability and reservation
foundation. It does not qualify a production storage boundary. Production
`writer_containment_available()` and `CRASH_SUPERVISION_QUALIFIED` remain
literal false constants, Qt Receive remains disabled, and no live Receive path
can reach this code. The result is **PARTIAL**: deterministic policy tests pass,
but kernel quota enforcement and backend write confinement are not qualified.

The implemented enforcement candidate is an administrator-provisioned,
exclusive XFS project-quota destination. ext4 is evaluated and its accounting
can be read, but capability acquisition rejects it as enforcement unknown.
The operator-selected destination itself must
be the configured quota root. Staging, Owly's nested temporary directories,
per-session TLS state, retained failures and a published bundle therefore stay
on the same filesystem and project. OpenContinuity does not mount a filesystem,
enable quotas, allocate a project ID, set a limit, run `sudo`, or change host
quota configuration.

## Storage threat model

Application tree scanning is not hard quota enforcement. A scan has a race
with concurrent writers and observes directory entries rather than all charged
allocation. The storage boundary must account for:

- concurrent upload streams and sequential files written between scans;
- completed, incomplete and Owly `.owly-staging-*` files;
- allocated blocks in sparse files separately from logical length;
- directory and filesystem metadata and exhaustion of available inodes;
- hard links, symlinks, special files and rename attempts, even though final
  staging validation rejects them;
- open but unlinked files, whose allocation remains charged until the last
  writer closes the inode;
- the generated TLS identity, runtime state, stale staging and crash-retained
  data across sessions;
- writers using a path, inherited descriptor, IPC recipient or external
  service outside the intended destination.

Project quotas enforce charged data blocks and project inode counts. Sparse
holes consume no blocks until allocated, while the existing logical-size scan
still rejects an oversized completed file. Open-unlinked inodes remain charged
by the kernel. A block and inode hard limit does not promise that every item of
global filesystem metadata is charged to the project, nor does it protect any
write that escapes to another project or filesystem. Retained data deliberately
continues consuming the same quota and can block later sessions.

The existing 512 MiB per-file, 1 GiB accepted-tree, count and depth checks
remain defense in depth and publication policy. They are not reclassified as
hard allocation limits. Cgroup I/O controls throttle or account I/O; they do
not cap aggregate allocated storage.

## Selected kernel mechanism

The adapter recognizes only XFS and ext4 and accepts enforcement evidence only
for XFS. It opens every absolute path one
component at a time with `O_DIRECTORY|O_NOFOLLOW`, compares destination and
configured-root device/inode identities, and requires an owner-only root. It
then verifies `FS_IOC_FSGETXATTR` reports the exact nonzero project ID and
`FS_XFLAG_PROJINHERIT`.

For XFS, `quotactl_fd(Q_XGETQSTATV)` must report project accounting and
enforcement, and `Q_XGETQUOTA` must report the configured project and exact
hard block and inode limits. XFS block units are converted from 512-byte basic
blocks. For ext4, the generic project `Q_GETQUOTA` query must succeed with both
hard-limit fields valid; generic quota block units are 1024 bytes. That query
does not independently establish active enforcement, so the ext4 snapshot
always records enforcement unknown and cannot acquire a lease. ext4 support
requires a separately verified status mechanism before qualification.
Failure, permission denial, unsupported ABI, zero/changed limits, unknown
enforcement, usage beyond a hard limit, or an initially exhausted limit fails
closed. These read-only queries may themselves require privileges supplied by
the future trusted manager. Such privileges must never be passed to Owly.

An atomic `.opencontinuity-airdrop-quota-owner` directory serializes
OpenContinuity sessions. The lease holds descriptors and the marker's
device/inode identity. Replacement, nonempty ownership state, failed
observation or failed removal leaves a stale reservation. Nothing automatically
deletes a reservation or retained data after uncertainty. The marker prevents
another conforming OpenContinuity session; trusted provisioning must also make
the project ID exclusive because it cannot exclude arbitrary external users of
the same project ID.

The P019D coordinator now acquires the quota lease after its containment
reservation and before staging. Runtime TLS state is allocated under the quota
root rather than outside its accounting boundary. Staging and runtime
directories are checked for inherited project identity. After complete writer
settlement, the coordinator revalidates the reservation, quota state and both
directories before any cleanup or publication decision, then revalidates before
releasing the session reservation. A changed or unobservable state blocks
publication and retains data that may still be unsafe to inspect or remove.
There is no polling-only fallback.

## Atomic publication compatibility

P019A's one `renameat2(RENAME_NOREPLACE)` bundle publication remains the only
publication operation. The destination is the quota root and staging is a
child of that pinned destination descriptor, so the rename is same-filesystem
and same-project. Rename does not change an inode's project ID; published files
remain charged to the configured hard limits. Releasing the application owner
marker does not disable quota accounting or release published allocation.

XFS and ext4 reject a rename/link into a project-inheriting directory when the
inode project differs. A cross-mount rename fails with `EXDEV`. These failures
must remain failures: there is no copy-and-delete fallback, and a rename error
leaves the private source unpublished. A dedicated filesystem is compatible
only when it is mounted by trusted provisioning at the actual selected
destination so staging and publication share a mount. A separate staging mount
is incompatible with the atomic rename invariant.

A size-limited tmpfs also imposes a kernel allocation boundary, but consumes
RAM and potentially swap, is not persistent, and cannot be atomically renamed
into a normal destination on another mount. It is therefore not the selected
publication design. A loopback or dedicated filesystem can provide a bounded
device, but mounting, ownership, recovery and destination placement belong to
trusted host provisioning and require their own qualification.

Before rename, a crash leaves charged private staging and a stale reservation.
After rename, the complete bundle is atomically visible and still charged.
Atomic visibility does not claim persistence: directory and file data need an
explicit durability protocol if crash durability becomes a requirement.
Project-ID reuse is forbidden while any published, retained, open-unlinked or
otherwise charged inode remains.

## Trusted provisioning and write confinement

A future deployment manager must, outside OpenContinuity:

1. enable project quota support on a disposable or dedicated XFS/ext4
   filesystem and establish that enforcement is active;
2. assign a unique project ID recursively to an owner-only destination, enable
   project inheritance, install exact hard byte and inode limits, and reserve
   that ID until every charged inode is gone;
3. expose the required quota status queries without giving Owly quota-control
   authority; and
4. bind the trusted containment/launcher policy to the same pinned destination
   and project identity.

A filesystem quota controls only charged writes. A compromised same-UID Owly
process can otherwise write to another writable directory or mount, retain or
inherit an outside descriptor, ask a user service to write, use sockets or
shared memory, or alter same-user state. P019E ships no mount namespace,
Landlock/seccomp policy, dedicated UID, broker, or manager-owned descriptor
allowlist. The future launcher must close inherited descriptors and confine
filesystem and IPC write ingress before exec. It must also prevent the backend
from changing project attributes or quota controls. Revalidation detects many
changes at the publication boundary; detection is not confinement and cannot
undo allocation outside the project.

This write-confinement dependency is independent of cgroup process
containment. Production placement, recursive cgroup qualification, an external
crash guardian, closed writer ingress during final validation/rename, and
consent revalidation at commit remain unresolved activation blockers.

## Tests and host evidence

Deterministic tests use the production quota lease policy with an injected
kernel probe. They cover unsupported filesystems, unavailable quota queries,
unknown enforcement, wrong project ID, missing inheritance, wrong hard block
or inode limits, exhausted capacity, destination mismatch, nonprivate roots,
ownership conflicts, descendant verification, changed observations, replaced
and nonempty reservations, cleanup failure and repeated sessions. Coordinator
tests cover missing capability, directory verification failure, retained state
after revalidation/release uncertainty, stale reservations, safe retry and the
unchanged successful-but-unqualified publication block. Existing P019A tests
continue to cover no-partial publication, cross-path failures, limits, hard
links, symlinks and special files. These are application-policy tests, not
proof that a kernel enforced EDQUOT.

The ignored opt-in test requires both
`OPENCONTINUITY_RUN_QUOTA_INTEGRATION=1` and an explicitly supplied disposable,
exclusive configured quota root. It checks real allocation/inode accounting,
same-project atomic publication, retained accounting after rename, concurrent
temporary-file growth, hard block exhaustion and hard inode exhaustion. It
caps accepted test configurations at 64 MiB and 4096 inodes and must be run
serialized on a disposable filesystem. It never configures quotas. A
qualification run must additionally record sparse allocation, partial writes,
open-unlinked allocation, crash retention, restart/stale reservation behavior,
cross-project and cross-mount rename rejection, and cleanup failure on the
target kernel/filesystem combination.

Local validation passed 265 Rust tests with two integrations ignored, 55 Python
tests, formatting, warnings-denied Clippy, build and whitespace checks. Qt/QML
and Swift/watchOS validation requires hosted CI.

This host is Linux 6.18.57, uid 1000, with `/workspace` on overlayfs. It is not
an XFS/ext4 project-quota test root and has no trusted provisioning. The opt-in
test was **KERNEL NOT RUN**. Mock PASS does not establish filesystem
enforcement.

## Readiness

The foundation is **PARTIAL**. It establishes a fail-closed adapter and the
correct lifecycle integration points, while production Receive and publication
remain unreachable. Receive cannot be enabled safely until kernel quota tests,
exclusive write confinement, real cgroup placement/containment, external crash
supervision, closed writer ingress and commit-time consent are independently
qualified.

`authorization: never_evaluated`

`physicalQualification: not_qualified`

RTL8852BE AWDL and iPhone 13 AirDrop are NOT RUN. Watch SE 2 W0–W6 is NOT
RUN/BLOCKED. Native Apple Auto Unlock is NO-GO.

## Primary references

- [Linux quotactl interface](https://man7.org/linux/man-pages/man2/quotactl.2.html)
- [XFS quota administration and project inheritance](https://man7.org/linux/man-pages/man8/xfs_quota.8.html)
- [XFS project quota status ABI](https://git.kernel.org/pub/scm/linux/kernel/git/torvalds/linux.git/tree/include/uapi/linux/dqblk_xfs.h)
- [Generic quota ABI](https://git.kernel.org/pub/scm/linux/kernel/git/torvalds/linux.git/tree/include/uapi/linux/quota.h)
- [XFS cross-project rename check](https://git.kernel.org/pub/scm/linux/kernel/git/torvalds/linux.git/tree/fs/xfs/xfs_inode.c)
- [ext4 cross-project rename check](https://git.kernel.org/pub/scm/linux/kernel/git/torvalds/linux.git/tree/fs/ext4/namei.c)
