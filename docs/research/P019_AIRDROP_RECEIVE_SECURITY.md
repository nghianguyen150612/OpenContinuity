# P019 AirDrop receive security review

## Trust boundary

An incoming AirDrop request and all sender metadata are untrusted. Owly 0.4.2
owns TLS, `/Discover`, `/Ask`, `/Upload`, DVZip and CPIO parsing, per-request
interactive acceptance, and extraction into its own temporary staging. The
OpenContinuity process does not set Owly's `--accept` flag and does not accept
links by opening them. Everyone-mode visibility does not mean authenticated
Apple identity; OpenContinuity does not possess Apple's identity records or
Contacts Only credentials.

The external executable is trusted local software, not a sandbox. Fixed
absolute paths are accepted only for root-owned regular non-symlink files that
are executable, not group/world writable, and do not have setuid/setgid bits.
Fixed argv prevents shell interpretation and PATH substitution, but does not
cryptographically authenticate a binary replaced by a privileged local actor.
Upstream may display sender name, file names and offered metadata
on terminal stderr as part of its consent prompt; this output is local and is
never copied into JSON or diagnostics.

## Explicit activation and cancellation

Status commands only read sysfs/procfs and fixed `iw`/Owly doctor queries.
Receive requires an existing AWDL interface, explicit CLI flags, interactive
terminal consent, an operator-selected existing destination, a private
same-user runtime directory, and a singleton lock. It does not activate or
reconfigure a radio. A separate physical runbook is required for any Owly
radio setup that may interrupt Wi-Fi or require privileges.

P019A currently blocks receive activation with writer_containment_unavailable.
The following describes the dormant supervisor, not an available receive path.
The session has a 15–3600 second deadline. The direct child may be killed with bounded polling; group termination is
not established. Escaped descendants cannot be accounted for, so publication
is blocked. Unknown writers require retained staging and runtime state. Owly's generated
TLS identity is placed under a unique private directory in `XDG_RUNTIME_DIR`
and that directory is retained if writers cannot be proven stopped; it is
removed only when safe.
Abrupt power loss can leave runtime files until the OS clears the runtime
directory; unlinking is not secure erasure.

Owly stdout/stderr are relayed only to a verified terminal, with a shared
64 KiB cap. Each relay opens `/proc/self/fd/2` with `O_NONBLOCK` on a separate
file description, so it does not change descriptor 2's status flags. If
procfs is unavailable, the descriptor is no longer a TTY, or terminal output
fails or exceeds the cap, the receive session stops and reports failure; the
status JSON remains on stdout. The relay drains child pipes while stopping so
they cannot strand the backend on a full pipe. This is best effort: terminal
output may be dropped on failure, and the result is never treated as a
successful transfer because output delivery failed.

## File handling

The operator must select an existing destination. OpenContinuity resolves that
path once, creates a random 0700 staging directory on the same filesystem, and
passes only that path to Owly. It rejects symlinks, hard-linked files, special
files, dotfiles, path separators, control/bidi characters, long names, deep
trees, excessive files/directories and over-budget sizes. Interrupted Owly
`.owly-staging-*` directories are removed; partial data is never deliberately
published. Before finalization, directories become 0700 and files 0600. One
descriptor-relative renameat2(RENAME_NOREPLACE) publishes the whole validated
tree as AirDrop Receive <random-id>, with bounded collision retries. Atomic
visibility does not imply crash persistence. Received content
is not opened, parsed by OpenContinuity, extracted again, executed or treated
as trusted.

Limits are 100 files, 128 directories, depth 8, 512 MiB per completed file and
1 GiB total accepted tree. `RLIMIT_FSIZE` also limits a single child-created
file. The supervisor inspects staging every 100 ms and terminates when it sees
a budget violation. Owly 0.4.2's CLI does not expose direct controls for its
16-connection limit and 16 GiB upload/extraction defaults, so the aggregate
limit can be transiently exceeded between inspections; there is no hard
filesystem quota. This is a known reliability and disk-exhaustion risk. For
that reason the Qt Receive action remains disabled, and P019 does not claim
production-safe high-volume receiving. A future integration should use an API
with configurable upload, total, connection and entry limits or provide a
separately authorized quota-backed staging filesystem before enabling the UI.

## Failures and evidence

Unknown schema, unsupported Owly version, missing radio state, unavailable
runtime directory, conflicting receive lock, child failure, timeout,
cancellation, output cap or cleanup failure never becomes a successful
interoperability claim. The terminal JSON distinguishes the process lifecycle
from data-plane and transfer outcome; where Owly has no machine-readable event,
those states remain unknown. A reported file count only measures the validated
files that were finalized locally.

All results retain `authorization: never_evaluated` and
`physicalQualification: not_qualified`. No real RF, iPhone identity, Apple
Account, Contacts Only, signature validation, Linux login authorization or
Apple Watch behavior is asserted by CI.

## P019A correction

The earlier finalize path published after failed sessions. This is fixed with
explicit eligibility checks and a fail-closed writer-containment gate. See
[P019A hardening](P019A_RECEIVE_PUBLICATION_HARDENING.md) for the current
blocked receive behavior, JSON counts, retained-state policy and limitations.

## P019B containment decision

The current runtime has cgroup v2 mounted but no delegated writable subtree;
systemd and its user bus are unavailable. Receive remains blocked before
backend launch. A future supported deployment needs an exclusive delegated
subtree with an inaccessible parent migration boundary and an external
supervisor that verifies recursive emptiness. See
[P019B process containment](P019B_PROCESS_CONTAINMENT.md). Process groups,
pidfds, and inspection of Owly's current lack of subprocess launches do not
establish containment against compromised same-UID code.

## P019C containment foundation

An isolated cgroup v2 component now models an explicitly delegated exclusive
subtree and monitors its recursive population so intra-subtree migration
remains visible. It has not been integrated into the receive supervisor. The
false availability gate and publication eligibility checks remain unchanged;
the real-kernel integration test is NOT RUN on this host. Hard aggregate disk
quota is still an independent blocker. See
[P019C foundation](P019C_CGROUP_CONTAINMENT_FOUNDATION.md).

## P019D lifecycle update

The internal coordinator acquires a cgroup lease before staging and retains
both staging and runtime identity if settlement or cleanup is uncertain.
Publication also requires crash supervision qualification, which remains
unavailable. Production Receive and Qt Receive stay disabled. See
[P019D lifecycle and crash supervision](P019D_RECEIVE_LIFECYCLE_AND_CRASH_SUPERVISION.md).

## P019E quota update

The inactive coordinator now acquires an externally provisioned XFS/ext4
project-quota lease before creating staging or runtime TLS identity. The quota
root must be the exact private destination and must expose the configured
inherited project ID plus exact hard block and inode limits. Staging, nested
Owly temporary data, retained failures and a published bundle therefore stay
charged to one project. The lease is revalidated after recursive writer
quiescence and before reservation release. Any unknown or changed state blocks
publication and there is no scanner-only fallback.

Atomic publication remains one same-filesystem, same-project
`RENAME_NOREPLACE`; cross-mount or cross-project failure is not converted into
a copy. The current overlayfs host did not run the ignored kernel enforcement
test, so actual EDQUOT behavior is NOT RUN. A project quota cannot stop Owly
from writing through another path, inherited descriptor or service. Trusted
write confinement, containment and crash supervision remain independent
activation blockers. See
[P019E quota foundation](P019E_HARD_DISK_QUOTA_FOUNDATION.md).

XFS is the implemented enforcement candidate. ext4 accounting does not prove
enforcement and remains fail-closed pending a qualified status mechanism.
