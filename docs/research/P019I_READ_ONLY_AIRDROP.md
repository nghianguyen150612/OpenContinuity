# P019I — Read-only AirDrop/AWDL diagnostics

## Scope

This extraction is based on `main` and contains only the local diagnostic path,
its report schema, a manually refreshed native Qt Settings display, and related
regressions. The CLI exposes `airdrop doctor [--json]` and the alias
`airdrop backend status [--json]`. The diagnostic reports local Wi-Fi and
AWDL-interface observations, then (when installed at an approved system path)
checks the exact Owly `0.4.2` version and parses its doctor report. A fixed
argument vector, a 2-second version deadline, a 3-second doctor deadline, and
bounded output are used. Missing binaries, unsupported platforms, malformed
reports, unexpected process exits, timeouts, and output overflow do not become
success.

The Qt Settings page refreshes only after an explicit click. It displays local
radio/backend states and the invariant qualification fields. The Receive button
is disabled, and the command is absent from this branch. No backend launcher,
receive coordinator, quota manager, guardian, staging code, or publication path
is included. A local `ready` state means only that the corresponding local
checks passed; it does not establish advertisement, Apple identity, AirDrop
interoperability, or transfer support.

## Actual diagnostic side effects reviewed

Owly is pinned to upstream commit
[`6f2e8ea99aed0687558d78a1dd49154ab3a5fa76`](https://github.com/t4t5/owly/tree/6f2e8ea99aed0687558d78a1dd49154ab3a5fa76), version 0.4.2. Source inspection covered `owly-cli/src/doctor.rs`,
`owly-radio/src/probe.rs`, `owly-radio/src/injection/mod.rs`, and the daemon's
`Probe` D-Bus method. The CLI doctor connects to an already-running `owlyd` and
asks it for `Probe`; if unavailable, it loads `/etc/owly/radio.toml` and probes
through read-only sysfs/nl80211/firewall configuration observations. Owly's
probe implementation documents that it needs no root and changes nothing.
Neither path calls `radio up`, acquires a radio lease, sets channel or
regulatory-domain state, creates an interface, edits firewall rules, activates
AWDL, starts `owlyd`, or runs `modprobe`. P019I never launches the daemon.
Owly daemon startup has a separate recovery path that can undo a prior radio
journal; the diagnostic does not start the daemon and does not invoke that path.

OpenContinuity's Linux path reads `/sys/class/net`, `/proc/net/if_inet6`,
`/proc/sys/kernel/osrelease`, `/proc/self/status`, and limited sysfs device
metadata. Optional fixed-path `iw` calls are `dev <interface> link`, `reg get`,
and `phy <phy> info`; each is a read-only query. The CLI never asks for root,
uses a shell, changes network configuration, or searches `PATH` for Owly/iw.
A system-installed Owly binary is accepted only when it is a regular,
root-owned, non-writable, non-setuid/non-setgid executable at `/usr/bin/owly`
or `/usr/local/bin/owly`, and its version is exactly 0.4.2.

The reviewed Owly workspace and the `owly-airdrop`/`owly-radio` manifests
identify MIT licensing; its nested kernel-derived `omdrop-awdl` component is
GPL-2.0-only. P019I copies no upstream implementation and adds no Owly
library/source dependency. GPL-only OWL/OpenDrop implementations are not copied
or linked into this project.

## Evidence categories

| Category | P019I status |
| --- | --- |
| Implemented diagnostics | Fixed-path local Wi-Fi/AWDL reads, bounded `iw` and Owly doctor subprocesses, exact Owly version pin, strict schema validation, Qt manual status display |
| Mock-tested behavior | Missing/unsupported state mapping, malformed and unknown doctor fields, duplicate keys, invalid status, nonzero process exit, timeout, bounded output, Settings process/report validation, disabled Receive control |
| Host-observed behavior | Local results are diagnostic observations only; see `docs/P019I_VALIDATION.md` for the exact runner result and CI evidence |
| Physical AWDL interoperability | NOT RUN; RTL8852BE / iPhone 13 AirDrop remains NOT RUN |
| Receive production qualification | Not implemented in this extraction; no file can be staged or published |

`authorization: never_evaluated`

`physicalQualification: not_qualified`

Watch SE 2 W0–W6: NOT RUN/BLOCKED. Native Apple Auto Unlock: NO-GO.

## Reconciliation with PR #20

PR #20 remains the original Draft branch with P019A–P019H history and Receive
security work preserved. This PR is an independent baseline from `main`; it does
not modify PR #20's branch. If P019I is accepted first, reconcile PR #20 by
retargeting/rebasing its existing work onto the accepted P019I commit, then
review the resulting diff against that commit. Retain the P019C–P019H
containment, quota, manager, guardian, staging, and publication files as
separate Receive research/security work, but do not restore their CLI runtime
routing or Settings Receive instructions as production functionality. Remove
duplicate diagnostic/core/UI files from the later PR by keeping the P019I
versions and resolving only necessary type/schema dependencies. Re-run all
security review and CI on the reconciled final SHA. Do not merge either PR as
part of this extraction.
