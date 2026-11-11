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
OpenContinuity neither creates `awdl0` nor owns an `owlyd` lease. A live
receiver can start only if the local read-only preflight sees exact Owly 0.4.2,
an already-up `awdl0`, a link-local address, and a successful Owly doctor
report. A preflight result is not a connection or an interoperability claim.

## State semantics

The backend vocabulary is `unsupported`, `unknown`, `available`, `blocked`,
`ready`, `active`, and `failed`. Control plane, data plane, advertisement,
radio and receiver availability are separate fields. The current CLI cannot
return actual per-service advertisement readiness or transfer byte progress;
those values remain `unknown`. `receiver_available` means that current local
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
and `RLIMIT_FSIZE`. SIGINT and SIGTERM request a short graceful stop, followed
by process-group SIGKILL on deadline. Child output is sent only to the local
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
