# P019 AWDL backend architecture

## Components

`opencontinuity_core::airdrop` owns portable state enums, provenance labels,
versioned status fields and the receive lifecycle. Linux-only CLI code performs
read-only probes and supervises the optional external receiver. The Qt Settings
bridge calls only `opencontinuity airdrop doctor --json`; it never invokes a
radio operation. AirDrop remains independent from the existing BlueZ GATT
services.

```text
Settings ── fixed absolute CLI argv ──> read-only doctor
                                             │
                                             ├── local sysfs/procfs
                                             ├── optional fixed-path `iw`
                                             └── fixed-version `owly doctor`

operator terminal ── explicit consent/declaration ──> bounded CLI supervisor
        │                                                │
        └── already prepared `awdl0` <── Owly/owlyd ─────┤
                                                         ├── `owly receive`
iPhone ───────────────────── AWDL + mDNS/TLS ────────────┘
                                                         └── private staging
                                                              → validated files
                                                              → chosen directory
```

The diagram describes intended process boundaries, not a measured radio path.
OpenContinuity neither creates `awdl0` nor owns an `owlyd` lease. Receive activation is blocked pending writer containment. The read-only
preflight still requires the local read-only preflight sees exact Owly 0.4.2,
an already-up `awdl0`, a link-local address, and a successful Owly doctor
report. A preflight result is not a connection or an interoperability claim.

## State semantics

The backend vocabulary is `unsupported`, `unknown`, `available`, `blocked`,
`ready`, `active`, and `failed`. Control plane, data plane, advertisement,
radio and receiver availability are separate fields. The current CLI cannot
return actual per-service advertisement readiness or transfer byte progress;
those values remain `unknown`. `receiverAvailable` means that current local
preconditions were observed, not that an iPhone can discover the receiver.

The report is schema version 1 and always says
`authorization: never_evaluated`, `physicalQualification: not_qualified`.
Status queries do not start Bluetooth discovery, Wi-Fi scanning, AWDL, an mDNS
advertisement, or a receiver process. `iw` queries are fixed, read-only calls.

## Process supervision and ownership

The receive command accepts only the Owly backend, an existing destination, a
bounded duration, and both explicit `--radio-prepared` and `--consent` flags.
It additionally requires terminal stdin/stderr and a same-user private
`XDG_RUNTIME_DIR`. A nonblocking `flock` prevents another OpenContinuity
receive. The child executable comes from fixed absolute paths and gets a fixed
argument vector, a generic local receiver name, a new session/process group,
and `RLIMIT_FSIZE`. The dormant supervisor only kills its owned direct child with bounded polling.
It cannot establish descendant quiescence and does not signal numeric process
groups after leader reaping. Child output is sent only to the local
terminal's stderr; JSON status stays on stdout.

No `owly radio up`, `owlyd` startup, D-Bus privilege request, daemon install,
channel selection, interface mode change, firewall edit or NetworkManager
change occurs. Such radio preparation is an independent operator-controlled
physical flow documented in the RTL8852BE qualification plan.

## Current limits and extension point

The pinned Owly CLI prompts once per incoming request and writes completed
items into the supplied staging directory. Its public command does not expose
the library's `max_connections`, decompressed-total, entry-count or structured
progress controls. OpenContinuity places its own file/count/directory/depth/
total budgets around the staging tree, polls that tree during operation, and
sets a per-file OS limit. The polling interval means an in-flight transfer can
temporarily exceed the configured aggregate before termination. This boundary
is called out in the validation and security documents; Settings keeps Receive
disabled. A direct library integration must use independently reviewed
configurable limits and compatible Rust tooling before it replaces this
process boundary.

The Qt status provider validates exact schema fields, state enums, primitive
types, duplicate JSON members, output size, process deadline and exit status.
Generation checks discard stale callbacks. The UI only exposes a manual status
refresh and clearly shows unknown/block reasons; it does not list fictitious
peers or expose a receive control that cannot present the backend's consent UI.

## P019A publication boundary

[Receive hardening](P019A_RECEIVE_PUBLICATION_HARDENING.md) supersedes the
original process-group cleanup assumption. Receive now fails closed before
backend startup. The tested storage primitive publishes one private bundle via
a descriptor-relative no-replace directory rename, only after validation and
independently verified writer quiescence. No such quiescence provider is enabled.
Failed sessions never publish. Unknown writers retain containment rather than
triggering unsafe deletion. Atomic visibility is not crash durability.

## P019B containment assessment

The current host cannot create a delegated cgroup or contact a systemd user
manager, so the receive gate stays closed. Owly 0.4.2's inspected receive path
uses tasks and threads but launches no OS child; this is not a security
boundary for compromised code. A supported implementation requires an
exclusive delegated cgroup subtree, placement before exec, recursive kernel
population observation, subtree kill and bounded empty confirmation. See
[P019B process containment](P019B_PROCESS_CONTAINMENT.md).

## P019C foundation update

The Linux CLI contains a separate cgroup v2 foundation for a future explicitly
delegated subtree. It places a child before exec and observes/kills the entire
delegation because members may migrate within it. The current host lacks
delegation; the real-kernel test was not run, and the receive availability gate
remains false. See
[P019C cgroup foundation](P019C_CGROUP_CONTAINMENT_FOUNDATION.md).

## P019D lifecycle update

The disabled Receive path now composes lease acquisition, pre-exec placement,
tree settlement, bounded relay completion, independent population verification
and private data cleanup. The parent and all higher ancestor controls must be
protected against migration. A trusted external launcher and a manager that
can kill the subtree on supervisor loss are still required. See
[P019D lifecycle and crash supervision](P019D_RECEIVE_LIFECYCLE_AND_CRASH_SUPERVISION.md).
