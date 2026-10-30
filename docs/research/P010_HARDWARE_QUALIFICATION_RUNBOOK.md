# P010 physical qualification runbook

`authorization: never_evaluated`

Status: physical acceptance BLOCKED in the cloud workspace. Use the owned Arch host,
Watch SE 2, paired iPhone, compatible Mac and a distinct owned BLE scanner. Do not
start P011. Software CI cannot supply physical observations.

## Baseline inspection

P007B already implements BlueZ application/advertisement ownership and cleanup,
long-read offset handling, bounded clients, serialized callback lifecycle and a
64-event opt-in Watch trace. P009A already supplies the separate signed service,
GET/read/ACK cursor, sequential response writes, temporary enrollment and bounded
request/consent/response deadlines. P008 handles signed transcript verification.
P010 adds timing metadata and evidence validation; it preserves both wire formats.
See [prior lifecycle findings](P007B_LONG_READ_AND_RECONNECT.md),
[enrollment](P009A_TEMPORARY_ENROLLMENT.md) and
[physical adversarial plan](P009A_PHYSICAL_TEST_PLAN.md).

## Read-only host preflight

Run `python3 scripts/qualify-p007b-host.py --adapter hci0` with the actual explicitly
selected adapter. It never powers, pairs, scans or registers. Each command result
is recorded separately. PASS means a probe executed, not that its property value
is suitable. The legacy script's successful exit does not mean its internal probes
passed. Run `bash scripts/qualify-bluez-host.sh` privately to inspect property values,
controller firmware/LE roles and supported minus active advertisement instances.
Do not publish its raw output (it can contain addresses and usernames).

Record Arch release/kernel, BlueZ, controller model/firmware/LE support, selected
hciN, Powered, system D-Bus/BlueZ availability, GattManager1,
LEAdvertisingManager1 and supported/active advertisement instances separately.
Do not infer registration permissions from introspection. Record them NOT RUN
until an approved bounded server actually attempts registration. Policy denial is
FAIL for that attempt; no policy changes or automatic retries with privilege.

## Operator-controlled physical sequence

Obtain explicit operator approval locally before starting either server; this
runbook itself does not approve radio operation. Use the existing
[P007B commands](P007B_REAL_BLE_RUNBOOK.md) for the bounded P007 server and
[P009A commands](P009A_TEMPORARY_ENROLLMENT.md) for signed enrollment/server start.
Set `OC_P010_DIAGNOSTICS=1` only for the approved server invocation to emit numeric
JSON timing metadata to stderr. No host listener or persistent trace is enabled by
this variable alone. Enable Watch Diagnostics explicitly before scanning.

For each trial create a new evidence manifest:

```sh
python3 scripts/qualify-p010-evidence.py --template > /private/path/evidence.json
python3 scripts/qualify-p010-evidence.py /private/path/evidence.json
python3 scripts/qualify-p010-evidence.py /private/path/evidence.json --require-physical
```

Template stages start BLOCKED. Record NOT RUN for an available test that was not
executed. Use FAIL only for an executed failure. Use PASS only for successful
execution with proof. Set source `physical` only for real observations, `simulated`
for offline trials, `host_preflight` for inventory. Physical acceptance requires
all stages PASS and available model/tool/software metadata. A valid incomplete
manifest is useful evidence but fails `--require-physical`.

Record stage UTC, expected/actual lengths, elapsed milliseconds, error class and
an opaque `proof-NNN.json` reference. Keep referenced sanitized observations beside
the manifest; each should state the expected behavior and actual observation,
operator/tool context and timestamp. The validator checks manifest declarations;
it cannot attest operator honesty, verify proof contents or verify evidence-file
signatures. P009 signatures authenticate only research messages, not this manifest.
Review proofs independently. Never relabel a simulation as physical.

1. P007: separately observe GATT registration, advertisement registration and RF
   content with an independent scanner. Confirm only the private experimental UUID.
2. Discover the service and all five characteristics on the physical Watch. Read
   the complete 25-byte challenge, send sequential acknowledged frames and read
   the complete 44-byte success. Verify SHA-256. An 11-byte error is failure evidence,
   not a successful exchange. Record cancellation's distinct digest separately.
3. Exercise cancellation/error, mid-transfer disconnect, reconnect with a new
   challenge, foreground suspension, timeout, SIGINT/SIGTERM and owned-resource
   removal. Verify BlueZ object absence and independent cessation of RF; a process
   exiting is insufficient cleanup evidence.
4. Establish fresh temporary enrollment. Independently compare BOTH fingerprints,
   UID and bindings. Test signed rejection first, then explicit approval with actual
   successful on-device LocalAuthentication. Observe actual GET/read/ACK traffic,
   host pin verification before consent, response writes and Linux P008 verification.
   Watch response-complete ACK is not Linux signature verification.
5. Use separate proof references for independent trials (e.g. rejection vs approval).
   End every result with `authorization: never_evaluated`.

## Timing and adversarial trials

Watch diagnostics retain 64 recent samples and per-stage count/total/max across
up to 65535 samples. Mean latency is total/count; GET/read/ACK is one cycle from
GET initiation to ACK callback. CoreBluetooth write limit is an observed maximum
value length, not negotiated ATT MTU. Request delivery starts after status parsing;
consent timing starts after verification; LA timing starts at explicit approval.
Response delivery ends at response-Complete callback, not Linux verification.
Rust reports registration milestones elapsed from setup start, server lifetime
and cleanup duration. Failed early setup may have no timing sample; retain the
existing server error classification. Use synchronized private observations to
measure end-to-end Linux verdict; never subtract unrelated monotonic clocks.
Aggregates survive sample eviction; disabling diagnostics clears all Watch data.
Keep metadata with device/software versions in the evidence manifest.

Run the P009A negative matrix with owned equipment: wrong host/Watch fingerprint,
expired/revoked enrollment, wrong UID/Watch identity/transfer ID, out-of-order and
duplicate chunks, missing ACK, disconnect during each direction, old callback after
reconnect, suspension during consent, LA denied/cancelled and expiry during LA.
Separately approve any disruptive BlueZ owner restart; test advertisement failure,
server timeout and cleanup. Preserve fail-closed behavior. Software tests for these
paths cannot PASS their physical counterparts. Unsupported LA is BLOCKED for
approval; rejection remains an independent signed decision.
