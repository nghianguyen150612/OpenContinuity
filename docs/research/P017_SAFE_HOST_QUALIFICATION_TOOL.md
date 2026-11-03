# P017 safe host qualification tool

Software tooling is implemented; actual Arch/RF/Wayland/Watch qualification is
NOT RUN. `authorization: never_evaluated`. P017 remains provisional because the
original P001–P060 planning source is unavailable.

Run from the repository root with Python 3 on an operator-owned Arch host:

```sh
python3 scripts/p017_arch_qualify.py
python3 scripts/p017_arch_qualify.py --check --cli /trusted/bin/opencontinuity
```

The default invokes P015 read-only probes through an injected, bounded runner.
It checks kernel numeric version, Arch identity, BlueZ version, system bus,
org.bluez accessibility, sequential sanitized adapter indices, Powered and
Transport-filter availability, CLI version and Settings executable presence.
Settings presence is installation readiness only, not a launch or plugin test.
Transport availability is a prerequisite, not proof an LE filter is accepted on
hardware. BlueZ service accessibility is reported independently of binary version.
OS IDs use a fixed distribution allowlist and versions are numeric (or rolling/
unreported); no host name, full custom kernel suffix,
controller path, hardware address or personal version label is exported.
Readiness is blocked when the bus, BlueZ, powered LE adapter, CLI, Settings or
Arch host is unavailable. The P015 probe checks that BlueZ is already owned and addresses that unique
bus owner for subsequent probes, preventing incidental service activation even
if the daemon exits. No service is started or configured by the tool.

## Explicit radio operation

Only an operator's local action and affirmative flags enable a trial:

```sh
python3 scripts/p017_arch_qualify.py --active-discovery --seconds 10 \
  --cli /trusted/bin/opencontinuity --i-own-host --i-consent-to-radio-scan
```

This command is an example, not consent to run a scan in CI or this workspace.
The flags attest authority over the host/adapter and consent to radio scanning.
The tool refuses missing consent/ownership, duration outside 1–30, unavailable
prerequisites and unsafe executables before starting discovery. Check that no
experimental GATT server holds the project's adapter radio guard. Run discovery
and GATT sequentially on the selected adapter. The default discovery adapter is
the existing CLI's first adapter; inspect the private inventory before consenting.
The runner intentionally exposes no arbitrary CLI arguments or shell commands.

The only active argv is:
`opencontinuity devices discover --seconds N --events-jsonl`.
The existing CLI owns its Start/Stop/filter/connection lifecycle. The tool adds a
monotonic N+12 second deadline (covering the CLI's five-second setup, five-second cleanup and terminal-delivery budget), drains stdout with a 512 KiB total cap, discards
stderr, and handles SIGINT/SIGTERM by requesting graceful SIGTERM. It permits two
seconds for graceful termination, then kills the private process group and waits
at most one further second. Normal completion also removes unexpected descendants.
Probe executions have three-second deadlines and 16 KiB output limits. A forced
kill, timeout, cancellation, nonzero exit, missing/invalid terminal or incomplete
cleanup never passes S2. `exit_verified` means the direct child was reaped; it is
independent of exit success and does not prove BlueZ or RF cleanup.

Executable paths must be canonical absolute paths, use the expected basename,
be executable regular files, and have account/root-owned parents without group
or other write permission. Relative PATH entries, shell syntax, explicit symlinks
and writable ancestors are rejected. PATH-resolved system aliases are canonicalized
before checking. Install to a trusted account-owned prefix; a shared `/tmp` build
is deliberately rejected. This assumes a trusted local account, Python runtime,
environment and installed code; it is not executable signing or protection from
a same-account attacker replacing a binary after validation. Do not import an
untrusted shell environment or custom D-Bus destination for a physical trial.

No sudo, adapter writes, pair/trust changes, D-Bus policy changes, telemetry,
background discovery or automatic Watch connection is provided.

## Private optional transcript and offline validation

Prefer publishing the summary only. If a local transcript is needed, first create
an empty operator-owned directory with mode 0700 outside the repository:

```sh
install -d -m 700 /private/operator/p017-trial
python3 scripts/p017_arch_qualify.py --active-discovery --seconds 10 \
  --cli /trusted/bin/opencontinuity --i-own-host --i-consent-to-radio-scan \
  --transcript-dir /private/operator/p017-trial
python3 scripts/p017_arch_qualify.py --validate-existing \
  --input /private/operator/p017-trial/scan.jsonl
```

The directory must already exist and have a canonical absolute path. A fresh
`scan.jsonl` is reserved before radio execution using a directory descriptor,
O_EXCL and O_NOFOLLOW, mode 0600. Existing files, symlinks and unsafe directories
are refused. Only bounded stdout is written; even a rejected transcript is private
and must not be uploaded. It can contain personal display names. The tool does
not create a raw file by default. Operator input files must be regular and at
most 512 KiB; malformed inputs and errors are reported without filenames or data.

Offline validation runs no preflight, subprocess or radio operation. It reuses
P016 strict transcript validation. A valid imported transcript leaves S2 blocked
because origin and process exit are unverified. CLI exit 0 from the qualification
tool means a software check/validation completed without executed failure; it
never means physical acceptance. Readiness may be blocked with exit 0 in default
check mode; inspect each stage. Active prerequisite blocking exits 1. Invalid
options exit 2; rejected inputs and executed failures exit 1.

See [evidence schema](P017_RF_AND_CLEANUP_EVIDENCE.md) and
[manual matrix](P017_ARCH_BLUEZ_PHYSICAL_MATRIX.md).
