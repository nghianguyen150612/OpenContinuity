# P019F trusted deployment and kernel qualification

## Deliverable and qualification status

P019F is **PARTIAL**. It supplies a reviewed deployment contract, deterministic
contract/runner regressions, a read-only preflight and an explicit disposable-
host runner for the existing kernel adapters. It expands the ignored XFS
allocation test. It does not supply a privileged launcher, external guardian,
write sandbox or the guardian-dependent kernel scenarios. Those cases are
explicitly BLOCKED rather than reported as executable integration coverage.

Production `writer_containment_available()` still returns literal false,
`CRASH_SUPERVISION_QUALIFIED` is false, and Qt Receive stays disabled. The
harness invokes only fixed ignored test identities, never Owly or Receive.
There is no flag, environment variable or configuration path around those gates.

## Baseline review

Starting remote commit: `f90f5f851ce511878a3055356d8857354f38fb83`.
P019A atomic bundle publication and failed-session gating, strict doctor parsing,
P019C descriptor-based cgroup access, P019D lifecycle retention, and P019E quota
verification remain in place. The inspection identifies these unqualified
transitions:

- The outside same-UID pre-exec launcher cannot enter a protected subtree
  without common-ancestor migration authority. Giving that authority to Owly
  would also permit escape. The existing kernel case tests rejection only.
- `cgroup.events` recursive emptiness excludes only writers actually confined
  there. An IPC writer or escaped writable descriptor remains outside its proof.
- The inactive coordinator releases its cgroup reservation before staging
  validation/rename. A separate manager must keep all writer ingress sealed
  through the commit; the current empty check is not that seal.
- Consent is checked before `finalize()`, whose scan, permission changes and
  collision retries take time. Consent must be rechecked at each actual rename
  attempt inside a future trusted commit implementation.
- Supervisor SIGKILL bypasses cleanup. A same-UID watchdog can be signalled,
  ptraced or influenced through writable files/IPC and is not an authority.
- Project quotas do not stop alternate-path writes. XFS status flags are
  implemented, ext4 enforcement remains unknown, and neither has host kernel
  qualification here. A trusted administrator could change quota after a read;
  revalidation detects changes but does not lock configuration.

These findings cannot reach publication under the two false production gates.
P019F models the missing contract without treating its booleans as OS proof.

## Concrete trusted-host design

Use an independently provisioned disposable Linux VM with a system manager,
full cgroup v2 hierarchy and a dedicated XFS project-quota filesystem. Separate
the following responsibilities and credentials:

| Component | Authority and restrictions |
| --- | --- |
| Host system manager | Trusted root-owned service manager outside the writer subtree. Owns service identities, cgroup delegation and recovery journal. Stops writers if supervisor or guardian service disappears. Manager/kernel failure is outside the live guarantee and requires quarantine on restart. |
| Trusted launcher | Short-lived privileged helper under the manager. Uses manager-owned placement authority (for example a reviewed `clone3(CLONE_INTO_CGROUP)` implementation), constructs filesystem/IPC isolation, closes descriptors and drops supplementary groups, UID/GID and all capabilities before exec. It never exposes ancestor controls to the backend. |
| Guardian | Separate UID outside the writer subtree. Receives only identity-bound cgroup observation/kill handles and a supervisor pidfd from the manager at creation. Cannot administer quota or launch arbitrary work. Manager watches guardian loss as a stop condition. |
| Supervisor | Separate unprivileged UID. Coordinates consent, bounded relays and requests to the authenticated manager. Does not gain unrestricted host, quota, namespace or ancestor migration privileges. |
| Backend | Unique non-root UID per isolated session, distinct from guardian and supervisor. No capabilities, supplementary authority, cgroup control, quota administration, ptrace, manager socket or inherited writable outside descriptors. |
| Quota storage/publisher | Manager reserves one unique XFS project and exact block/inode limits. Supervisor/publisher pins destination and staging descriptors. Manager provides necessary query authority; it is never inherited by Owly. Published/retained inodes keep their project accounting. |

The launcher initially needs only the privileges needed for its operations:
write authority at source/destination cgroup common ancestor (or manager-owned
clone-into-cgroup authority), namespace/mount setup authority such as
CAP_SYS_ADMIN, and credential dropping via CAP_SETUID/CAP_SETGID/appropriate
capability control. Quota administration and potentially privileged status
queries belong to the provisioning manager, not supervisor/backend. Exact
capability reductions depend on the independently reviewed launcher; no broad
capability grant is proposed for OpenContinuity.

The sandbox must make the runtime executable/libraries read-only and expose
only project-backed staging/runtime as writable filesystem storage. Include
`/tmp`, `/var/tmp`, `/dev/shm`, procfs access, file descriptors and alternate
mounts in that inventory. Disable core dumps and deny backend project-setting
ioctls/mount/namespace/ptrace authority. Close everything except an explicit
stdin/stdout/stderr and required network descriptor allowlist. A read-only
host root alone is insufficient if another writable mount or descriptor leaks.

Use private IPC/mount namespaces and authenticated broker access. Deny Unix
sockets, user/system D-Bus, systemd user-manager access and other local writer
services unless explicitly reviewed. Network access needed by a future AWDL
worker is a separate allowlist, not host network administration. No radio or
network policy is installed by this phase. Distinct UIDs prevent ordinary
cross-process signalling; remove ptrace/capability exceptions and verify actual
host LSM/Yama policy rather than assuming UID separation solves every channel.

Manager service dependencies alone require testing. A scope can outlive its
caller. Candidate service policies include main-process lifetime, control-group
kill, bounded stop, and dependencies that stop the backend on guardian loss.
Test actual SIGKILL behavior and restart, not unit-file names. This repository
does not install or generate an enforceable service configuration.

## Identity-bound launcher/guardian contract

`scripts/p019f_contract.py` defines `Binding`, `Observation`, the `Manager`
protocol and an offline transition validator. Bind one random session nonce,
manager epoch and boot identity to a supervisor process identity, pinned
subtree, quota filesystem/project/generation, destination and staging identity.
The real manager must hold live descriptors and supervisor pidfd, authenticate
the peer via a protected channel and credentials, and prevent replay. A tuple
in JSON, a numeric PID, inode, pathname or marker is never sufficient authority.

Launch acknowledgement requires placement before exec, distinct credentials,
closed privileged descriptors, live guardian, unchanged quota and write
confinement. Sealing requires closed writer ingress, recursive emptiness,
unchanged quota and a live guardian. Commit requires renewed consent/deadline,
successful backend/relays/limits/staging validation and still-live authority.
Every uncertainty permanently retains the offline session. Supervisor loss
requests bounded subtree stop, but never publication even if stop succeeds.
Replay, reordered messages, changed identities and manager restart are rejected.
The validator has no publication operation and is not connected to Receive.

On manager/guardian loss, a qualified external system manager must quarantine
the reservation and stop the complete subtree. A new manager epoch cannot
reuse the old contract. Restart recovery requires a trusted persistent journal,
fresh pinned identities, exclusive ingress closure and verified emptiness;
otherwise retain staging and quota reservations for operator inspection. Never
kill or delete by reconstructing a stale pathname/PID alone. Quota IDs remain
reserved while published, retained or open-unlinked allocations exist.

## Harness operation

Default read-only preflight:

```sh
python3 scripts/p019f_qualify.py
python3 scripts/p019f_qualify.py --manifest /operator/provisioning.json
```

No provisioning record means BLOCKED, with kernel tests NOT_RUN. Preflight
checks a root-owned, non-writable-by-others regular JSON record, current boot,
distinct non-root role UIDs, exact device/inode resource identity, private
ownership, XFS/cgroup2 mount type, empty quota root and empty recursive cgroup
population. Paths are opened component by component without symlink traversal.
It neither creates a reservation nor uses P019C's mutating `preflight()` helper.
`HOST_PREFLIGHT_PASS` covers these static prerequisites only: actual quota
queries, ancestor migration protection and backend isolation are independently
checked by the relevant kernel tests/manager qualification, not inferred here.

Example record shape (values must be supplied by the trusted provisioner):

```json
{
  "schema": 1, "disposable": true,
  "boot_id": "CURRENT-BOOT-ID",
  "supervisor_uid": 1000, "backend_uid": 1001, "guardian_uid": 1002,
  "cgroup": {"path": "/operator/exclusive-cgroup", "device": 123, "inode": 456},
  "quota": {"path": "/operator/empty-xfs-project", "device": 789, "inode": 1011},
  "project_id": 4242, "hard_bytes": 8388608, "hard_inodes": 128
}
```

The administrator must externally create a disposable VM, enable XFS project
accounting/enforcement, assign the exclusive project and inheritance, set exact
limits, reserve an empty delegation and provision the separate credentials.
The record is an operator assertion of exclusivity backed by identity checks;
the harness cannot establish that an administrator or external writer will not
interfere. Run serialized with trusted host provisioning held stable. Do not
point it at existing user data, host hierarchy root or shared services.

Explicit active invocation, from a clean reviewed commit:

```sh
python3 scripts/p019f_qualify.py --manifest /operator/provisioning.json \
  --run quota --i-own-disposable-host --private-evidence-dir /operator/private-evidence
python3 scripts/p019f_qualify.py --manifest /operator/provisioning.json \
  --run placement_rejection --i-own-disposable-host \
  --private-evidence-dir /operator/private-evidence
```

The evidence directory must already exist privately and belong to the operator.
The runner rechecks pinned identities, sanitizes inherited integration settings,
executes fixed test identities serialized with a bounded 180-second deadline
and 4 MiB output budget, and requires one executed/passed/unignored test plus
a completion marker. Cargo/test tooling and the clean source checkout belong
to the trusted harness. This is not an interface for running arbitrary commands.
No test resource is automatically deleted by the runner after failure/timeout.
Its timeout only kills its own newly created cargo process group; escaped or
uninterruptible fixtures require trusted manager quarantine and inspection.

## Kernel coverage and missing scenarios

The cgroup kernel case validates rejected outside-to-inside same-UID placement,
empty population, cleanup and release. It is not successful trusted placement.
Trusted placement, descendant inheritance, `setsid`/process-group escape,
intra-subtree migration, cancellation, expiry, recursive kill, supervisor
SIGKILL while writing, guardian death and stale manager restart require a
real trusted manager adapter. They remain BLOCKED, with the above contract
tested using injected observations only. No same-UID mock is substituted.

The ignored XFS test now additionally checks sparse length versus allocated
blocks, open-unlinked allocation remaining charged, and private failed-session
allocation retention. It retains the previous same-project atomic bundle and
published-accounting checks, concurrent temporary-file growth and block/inode
exhaustion loops. It verifies positive actual allocations, accounting at the
limits and failure after bounded writes in addition to EDQUOT. Individual
writes can be short or fail after prior successful writes; the loop accounts
actual returned byte counts. It does not yet force and qualify a positive
short-write on every target kernel. Cross-project/cross-mount rename rejection,
quota changes by the manager and revalidation under changed enforcement remain
BLOCKED until explicit additional provisioned resources/control adapters exist.
No quota configuration is modified by this test. ext4 stays fail-closed.

## Evidence semantics and privacy

The report distinguishes `MOCK_PASS`, `HOST_PREFLIGHT_PASS`, `KERNEL_PASS`,
`NOT_RUN`, `BLOCKED` and `FAILED`. `--mock` executes only the contract/harness
unittests and records MOCK_PASS; it cannot promote kernel cases. Imported or
operator-supplied PASS reports are never accepted. A kernel status is assigned
only to the fixed test actually executed, never to its entire subsystem.

Every report binds records to exact Git commit, dirty state, random execution
ID, UTC observation time, sanitized kernel/mount/resource facts and a digest
of the provisioning configuration. Kernel output has a digest and exit status;
quota execution includes numeric observed block/inode accounting. Private raw
logs are created exclusively with mode 0600, capped, and retained on failure.
Public stdout omits configured paths, hostnames, boot IDs, source devices,
personal identifiers, radio addresses, payloads and raw panic/error text. Raw
logs may contain paths; never publish them without review. Redirect the JSON
report to an operator-owned evidence file for durable retention. No crash
durability or cryptographic host attestation is claimed by a report digest.

Hosted CI is offline software evidence. `productionQualified` always remains
false and the authorization/physical fields are fixed. A GitHub green workflow
cannot qualify containment, quota, physical RF or Apple interoperability.

## Qualification matrix

| Boundary | Implemented code | Mock evidence | Kernel evidence here | Outstanding blocker |
| --- | --- | --- | --- | --- |
| Trusted launcher | Contract only; existing placement rejection | MOCK_PASS | NOT_RUN | Privileged placement + credential/FD drop adapter |
| External crash guardian | Protocol + loss/epoch policy only | MOCK_PASS | NOT_RUN | Independent credentials, system-manager death/restart qualification |
| Delegated cgroup enforcement | Existing pinned controller and rejection test runner | MOCK_PASS | NOT_RUN | Successful trusted placement, descendant/escape/kill tests |
| XFS project quota | Existing adapter; expanded allocation test and runner | MOCK_PASS | NOT_RUN | Disposable XFS execution; short-write/cross-boundary/config-change cases |
| Backend write confinement | Contract and deployment design | MOCK_PASS (policy only) | NOT_RUN | Enforced namespace/UID/IPC/descriptor sandbox |
| Publication commit boundary | Existing atomic primitive; offline seal/commit policy | MOCK_PASS | NOT_RUN | Authority held sealed through rename and commit-time consent |
| Physical AirDrop interoperability | No live Receive | NOT_RUN | NOT_RUN | RTL8852BE/iPhone testing after security qualification |

Current host: Linux 6.18.57, uid 1000, workspace overlayfs, visible cgroup v2,
no systemd manager or provisioned XFS/delegation. Default preflight reported
BLOCKED; both kernel integrations are NOT_RUN. No host service, mount, quota,
cgroup, network or radio setup was attempted.

`authorization: never_evaluated`

`physicalQualification: not_qualified`

RTL8852BE AWDL and iPhone 13 AirDrop: NOT RUN. Watch SE 2 W0–W6: NOT
RUN/BLOCKED. Native Apple Auto Unlock: NO-GO.

## References

- [Linux cgroup v2 migration/delegation/kill](https://docs.kernel.org/admin-guide/cgroup-v2.html)
- [clone3 placement and CLONE_INTO_CGROUP](https://man7.org/linux/man-pages/man2/clone3.2.html)
- [pidfd process identity](https://man7.org/linux/man-pages/man2/pidfd_open.2.html)
- [systemd service lifetime](https://www.freedesktop.org/software/systemd/man/latest/systemd.service.html)
- [systemd kill semantics](https://www.freedesktop.org/software/systemd/man/latest/systemd.kill.html)
- [XFS quota semantics](https://man7.org/linux/man-pages/man8/xfs_quota.8.html)
- [Linux capability boundaries](https://man7.org/linux/man-pages/man7/capabilities.7.html)
