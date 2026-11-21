# P019H: first kernel qualification

Status: **HOST BLOCKED**. No kernel qualification is claimed.

Read-only assessment on 2026-11-21 used starting commit
`f76fbb57515b3d6be29a98e9a23d092c2fd83122`; the final commit is the
commit containing this document. The starting working tree was clean and the
remote branch matched. PR #20 was open, draft and unmerged; six hosted checks
on the starting SHA succeeded. CI is not kernel or production security evidence.

## Host and provisioning

The available managed environment reported Linux `6.18.57`, cgroup v2 visible
at `/sys/fs/cgroup`, and no visible systemd manager. The process had no
effective capabilities; the cgroup mount was root-owned and not writable by
the current user. The workspace filesystem was overlay; `findmnt` showed no
XFS mount. These are observations of this environment, not proof about other
hosts. No independent disposable host, operator manifest, exclusive delegated
subtree, XFS project with active enforcement, distinct provisioned role
credentials, or trusted launcher was supplied or verified. Resource identities,
XFS features and delegation configuration are therefore unavailable. No host
mount, quota, cgroup, service or kernel configuration was changed.

The runner's default read-only invocation at the starting SHA reported host
preflight **BLOCKED** and both kernel tests **NOT_RUN**. No manifest was supplied;
there was no HOST_PREFLIGHT_PASS, KERNEL_PASS or FAILED kernel test. No quota
allocation, write result, migration or cleanup observation exists. No active
qualification was attempted.

## Runner review and prerequisites

The P019C–G foundation and P019G-R1 guardian permission evidence were reviewed
alongside the runner and ignored tests. The runner requires a root-owned manifest
bound to the boot and resource device/inode, distinct non-root role IDs, private
empty resources and appropriate filesystem types. Active execution requires a
clean commit, explicit opt-in and private evidence directory. It selects fixed
Rust test names, bounds runtime and output, retains private logs, and requires
an assertion marker and one passed test before reporting KERNEL_PASS. It does
not invoke production Receive. Static preflight does not prove quota enforcement,
ancestor migration restrictions, or exclusivity against an external actor.

The runner does not itself lock both resources across concurrent invocations;
P019F requires trusted provisioning and serialized execution. Independent
exclusivity and serialization must be established before active testing; do not
infer them from an empty directory or a preflight pass. No runner change is
claimed or tested here. The quota test exercises publication and retained
allocations, sparse and open-unlinked accounting, concurrent writers and hard
limits; the cgroup test is a placement rejection and cleanup test, not successful
trusted placement or general containment qualification.

## Remaining gates

Provision an independently administered disposable VM with a reserved, exclusive
non-root cgroup delegation and separate supervisor, guardian and backend identities;
provide a dedicated empty XFS project with active project-quota enforcement and
bounded limits, a current administrator-owned manifest, private evidence storage,
operator opt-in and independently verified serialization. Stop on any uncertainty
about exclusivity, migration, enforcement or cleanup. Trusted manager placement,
external crash supervision and backend filesystem/IPC sandboxing remain blocked
and separate from the two proposed kernel tests.

Production Receive remains disabled: `writer_containment_available()` and
`CRASH_SUPERVISION_QUALIFIED` remain false. Qt Receive and production Owly Receive
remain disabled; no automatic AWDL activation is authorized.
`authorization: never_evaluated`; `physicalQualification: not_qualified`.
RTL8852BE AWDL / iPhone 13 AirDrop: NOT RUN. Watch SE 2 W0–W6: NOT RUN/BLOCKED.
Native Apple Auto Unlock: NO-GO.
