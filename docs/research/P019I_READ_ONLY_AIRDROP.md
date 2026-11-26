# P019I — Read-only AirDrop/AWDL diagnostics

## Implemented scope

`airdrop doctor [--json]` and `airdrop backend status [--json]` read Linux
sysfs/procfs and optional fixed-path `iw` queries: `dev <interface> link`,
`reg get`, and `phy <phy> info`. These are query operations and do not scan,
configure channels/regulatory state, activate AWDL, request privilege, write
transfer files, or change PAM/login/unlock settings. Missing tools and hardware
remain unavailable/unknown. Non-Linux reports are explicitly unsupported.
Qt refresh is manual; Receive is disabled. CLI Receive routing, staging,
publication, containment, quota, manager and guardian implementations are absent.

## R1 side-effect correction

Reviewed Owly version 0.4.2 at upstream commit
[`6f2e8ea99aed0687558d78a1dd49154ab3a5fa76`](https://github.com/t4t5/owly/tree/6f2e8ea99aed0687558d78a1dd49154ab3a5fa76).
`crates/owly-cli/src/doctor.rs::report` first connects to the system bus,
builds `RadioProxy`, and invokes `Probe`. `crates/owlyd/src/api.rs::radio`
disables property caching, but does not disable service activation. Packaged
`packaging/io.github.t4t5.Owly.service` specifies fallback `Exec=/bin/false` and
`SystemdService=owlyd.service` (the unit runs `owlyd`). D-Bus can therefore activate the daemon before
answering a diagnostic call. `crates/owlyd/src/service.rs::serve` invokes
`backend.recover()` before serving; injection recovery can undo journalled radio
changes. The earlier claim that doctor only contacts an already-running daemon
was incorrect.

The `Probe` handler itself calls the capability prober; fallback configuration
reads `/etc/owly/radio.toml`, sysfs/nl80211 and `/etc/ufw/ufw.conf` through
`owly-radio/src/probe.rs`, `injection/mod.rs`, and `sys.rs`. These probe paths
read configuration/capabilities rather than invoke interface, channel, regulatory
or firewall mutators. That does not make the complete activating CLI safe.
**Live Owly execution is disabled unconditionally**, including version execution.
A detected trusted-path installation reports `backend_probe_unsupported`; no
backend version or control-plane readiness is claimed. Strict 0.4.2 schema and
duplicate-key parsing remains in offline tests only. No bypass is provided.

## Executable and deadline assumptions

`iw` candidates are fixed paths, with a regular non-symlink final component.
Every resolved parent is root-owned and not group/other writable. The runner
reopens with `O_NOFOLLOW`, verifies root ownership, executable bits, absence of
setuid/setgid and file capabilities, and ELF format. It holds the file open and
executes `/proc/self/fd/<fd>`, preventing pathname replacement from selecting a
different inode after verification. The process environment is cleared; only
fixed PATH and C locale are supplied. The working directory is `/`.

System administrators, the existing host process, dynamic loader, system
libraries and kernel are trusted. Root can modify a file in place or system
libraries; these checks do not authenticate package provenance or provide
malicious-process containment. Existing ACL grants are reflected in the group
mode mask. Unreadable capability metadata fails closed. No privilege is added.

Each query has a one-second execution/pipe deadline with up to two seconds for
leader termination. Nonblocking readers cap both retained bytes and work per
tick, drain both streams, reject truncation, and close descriptors on timeout.
Descendants cannot extend the return deadline or leave reader threads alive;
this is not a guarantee of terminating arbitrary descendants. Three queries
fit within Qt's ten-second normal diagnostic budget (at most roughly nine
seconds including termination allowances). Qt timeout/cancel kills its CLI,
fences stale callbacks, and accepts no partial result. Host scheduling and
uninterruptible kernel tasks cannot be given a hard real-time guarantee.

## Licensing and evidence categories

Owly workspace, CLI, radio and AirDrop crates declare MIT; nested
`crates/omdrop-owl/omdrop-awdl/LICENSE` declares GPL-2.0-only. No upstream
implementation is copied or linked, and no Owly dependency is added. Linux
uses existing libc; no new dependency is introduced. Rust CI uses 1.85.

| Category | Status |
| --- | --- |
| Implemented | Local sysfs/procfs reads, bounded optional iw queries, Qt manual display |
| Mock-tested | Strict pinned offline doctor parser, missing/unsupported reports, exit/output/deadline checks, descendant pipes, disabled Receive |
| Host-observed | Runner-specific software evidence only; see validation document |
| Physical AWDL | RTL8852BE / iPhone 13 AirDrop: NOT RUN |
| Receive production qualification | Not implemented; no staging or publication |

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
