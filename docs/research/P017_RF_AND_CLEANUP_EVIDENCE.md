# P017 RF, cleanup and evidence provenance

Software validation and physical acceptance are independent. No real controller,
separate RF receiver or Watch was used in this workspace. Hardware stages are
NOT RUN; the physical qualification gate is BLOCKED.

## Generated envelope (schema version 1, milestone P017)

The tool produces a local JSON envelope with fixed stage IDs and statuses:

| Stage | Meaning | Software interpretation |
| --- | --- | --- |
| S0 | Arch/BlueZ/CLI/Settings read-only readiness | `host_preflight`; pass is prerequisites only |
| S1 | Explicitly consented process launched | Process creation only; not proof radio started |
| S2 | Event/terminal validation plus direct child exit | Pass only for a matching-duration completed terminal, reported complete cleanup, normal exit 0 |
| S3 | Independent RF corroboration | Separate receiver and controlled advertising required |
| S4 | Native Qt Wayland/Hyprland operation | Real compositor, manual interaction required |
| S5 | Exact owned BlueZ cleanup | Private method/reply observer and hardware review required |
| W0 | Signed Watch SE 2 installation | Signing/provisioning and installed app required |
| W1 | Linux GATT/advertisement registration and independent RF | Explicit bounded server only |
| W2 | Watch foreground scan, connect and characteristic discovery | Physical Watch central only |
| W3 | P006 synthetic exchange | Complete challenge/frame/result lengths and digest |
| W4 | P009 signed rejection | Both independent test pins and Linux verification |
| W5 | P009 explicit approval and LocalAuthentication | Actual foreground device result and Linux verification |
| W6 | Watch/server cancellation, timeout and cleanup | Actual callbacks, resource removal and RF cessation |

Every `stages` entry has `status` (`pass`, `fail`, `blocked`, `not_run`) and
`provenance` (`host_preflight` for generated runner observations). S3/S4/S5/W0–W6
remain `not_run` in generated observations. Tests use `mock_ci` in independent
operator declarations only for non-pass results. A generated software S2 pass
cannot attest the physical S3 or S5 stage.

Independent top-level fields are `execution_started` (bool), `observed_events`
(validated count, 0–512), `terminal_valid` (bool), `reported_cleanup` (`not_run`,
`unknown`, `reported_complete`, `reported_incomplete`, `reported_not_needed`),
`exit_verified` (bool), `exit_code` (integer or null), `process_outcome` (`not_run`,
`exited`, `cancelled`, `timeout`, `output_limit`). P016's `summary` is included only
for a valid transcript: bounded counts and fixed classifications, with no names,
identifiers or session IDs. P016's `complete_transcript_exit_unverified` label
still describes the transcript alone; use P017's separate process fields to
understand the runner's direct observation. `reported_complete` is a CLI claim,
not an independent audit. The original CLI JSONL schema is unchanged.

All envelopes preserve `authorization: never_evaluated`,
`independent_rf: not_run`, `hardware_attestation: not_verified`,
`physical_qualification: blocked` and
`apple_auto_unlock: no_go`. No software branch automatically certifies hardware.
The `host` object is the sanitized P015-derived readiness described in the
[tool guide](P017_SAFE_HOST_QUALIFICATION_TOOL.md).

## Operator declarations and human review

Generate a strict input envelope without probes or radio:

```sh
umask 077
python3 scripts/p017_arch_qualify.py --evidence-template > /private/operator/stages.json
python3 scripts/p017_arch_qualify.py --validate-existing \
  --input /private/operator/p017-trial/scan.jsonl \
  --operator-evidence /private/operator/stages.json
```

The input must have exactly `schema_version: 1`,
`authorization: never_evaluated` and `stages`, containing every physical stage
S3/S4/S5/W0–W6. Each row must contain exactly `status`, `provenance`,
`evidence_ref`. Provenance is `mock_ci`, `host_preflight`, `operator_hardware`
or `independent_observer`. The evidence reference is null or `proof-NNN.json`.
A reported pass needs a reference and hardware/observer provenance; S3 pass needs
`independent_observer`. Unknown keys, duplicate keys, arbitrary paths, invalid
statuses, simulated passes, excessive size/depth and malformed JSON are rejected.
Do not put metadata, fingerprints, private evidence contents or names in this
input. Paths/proof contents are never opened or trusted by this validator.

Validated declarations appear under `operator_reported` only. They never update
observed `stages`, establish trusted attestation, or clear the blocked physical
gate. A self-asserted JSON pass is not proof. A human operator must review raw
private evidence and the independent observer's method, discrepancies and timing.
Keep the signed-off worksheet locally with controlled access; publish only a
reviewed sanitized report. This tool supplies no hardware certificate/signature.

## Private worksheet and cleanup observer

For each trial record locally: UTC start/end, source revision, model/firmware and
software versions (no serials), consent, selected test adapter, expected/actual
counts and timing, stage status, provenance and an opaque proof reference.
Use a fresh record for completion, SIGINT, SIGTERM, cancellation, timeout and
failed cleanup. Attach no raw identifiers to the public envelope.

Before a 10-second trial, arrange a separate owned receiver and a controlled
third-party peripheral with known advertising on/off intervals. Observe before,
during and after the trial; compare receiver observations with CLI event timing.
Record sample counts, clock alignment uncertainty and discrepancies. Do not derive
RF success from BlueZ events or use a Watch as the advertising peripheral.

Arrange a separately consented, bounded, private D-Bus diagnostic observation on
the owned test host. Begin before the trial; stop within a fixed 20-second window.
Use a locally reviewed observer capable of correlating the CLI's unique bus sender,
pinned BlueZ unique owner and method replies. If permission or such an observer
is unavailable, S5 is BLOCKED; do not alter system-bus policy to obtain it. Public
evidence records only: filter attempted/accepted, Start attempted/confirmed, Stop
attempted/confirmed, empty-filter reply confirmed and connection close/process
exit observed, plus timing/error classifications. Keep sender names, serials,
paths and other-client traffic private. No automatic bus monitor is installed.

The production implementation pins BlueZ's owner, confirms its Start acquisition,
attempts at most one matching Stop and clears its filter on that same peer. It
releases bounded signal subscriptions before waiting for cleanup replies (P016),
then closes its connection and releases the adapter guard. A lost Start reply
leaves ownership uncertain and must not cause Stop on an unowned session.
BlueZ owner replacement never licenses calls to the replacement owner. Preserve
other clients: shared `Discovering=true` may remain after our confirmed Stop;
`Discovering=false` and process exit alone cannot prove owned cleanup.

Physical failure cases: unavailable/off adapter before start; consented user
cancel; unexpected CLI exit; missing terminal; incomplete Stop/filter reply;
owner loss on an isolated test host only with separate disruption consent. Record
uncertain cleanup visibly and stop further trials until the operator verifies
resource state. Do not restart services, power adapters or kill unrelated clients.
