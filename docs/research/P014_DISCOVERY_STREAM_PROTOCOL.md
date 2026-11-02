# P014 JSONL discovery stream protocol v1

Command:

```sh
opencontinuity devices discover --seconds 10 --events-jsonl
```

`--events-jsonl` is mutually exclusive with `--json`. The existing P013
`devices discover --seconds 10 --json` command, schema, and exit behavior remain
available. Both formats use the same invocation, adapter snapshot, BlueZ owner,
discovery filter, StartDiscovery call, observation map, cleanup, and radio guard.

## Common envelope

Every line is one compact UTF-8 JSON object followed by LF. All events contain
exactly these common fields plus the fields listed for their category:

| Field | Contract |
| --- | --- |
| `schema_version` | Integer `1`. |
| `source` | `bluez_le_discovery_events`. |
| `type` | `session`, `device`, or `terminal`. |
| `authorization` | Always `never_evaluated`. |
| `scan_session_id` | 32 lowercase hexadecimal characters generated randomly per invocation; no persistence. |
| `sequence` | Starts at 1 and increases by exactly one for each accepted event. |

There is exactly one `session` event first and at most one `terminal` event last.
All lines use the same session ID. A session that fails before `StartDiscovery`
still produces a `session` event with `state: "not_started"` before its terminal
event. That event is written after setup cleanup has finished. A successful start
is reported immediately after the successful StartDiscovery reply.

## Session event

```json
{"schema_version":1,"source":"bluez_le_discovery_events","type":"session","authorization":"never_evaluated","scan_session_id":"0123456789abcdef0123456789abcdef","sequence":1,"state":"started","status":"started","scan_started":true,"requested_duration_seconds":10,"adapter_index":1}
```

`state` is `started` or `not_started`. `status` is `started` only after the
successful BlueZ reply; otherwise it is the fixed setup outcome. The adapter
index is null only when selection did not complete. It is scoped to the current
snapshot and is not an adapter identity.

## Device event

```json
{"schema_version":1,"source":"bluez_le_discovery_events","type":"device","authorization":"never_evaluated","scan_session_id":"0123456789abcdef0123456789abcdef","sequence":2,"id":"scan-0123456789abcdef-001","display_name":"Unverified peripheral","provenance":"bluez_event_during_session","change":"observed"}
```

`id` uses the first 16 session-ID characters and a three-digit encounter index.
It is not derived from a Bluetooth address, object path, or persistent device
identity. `change` is `observed` for a new in-memory device or `updated` when its
sanitized display name changes. Device events come from the same path-to-row map
used by the final report. Cached ObjectManager objects and name-only changes do
not create sightings.

`provenance` is always `bluez_event_during_session`. This does not prove an
independent RF sighting, proximity, Apple hardware identity, or authentication.

## Terminal event

The terminal event repeats the definitive P013 outcome fields and adds stream
counts and a bounded reason enum:

```json
{"schema_version":1,"source":"bluez_le_discovery_events","type":"terminal","authorization":"never_evaluated","scan_session_id":"0123456789abcdef0123456789abcdef","sequence":3,"scan_requested":true,"scan_started":true,"scan_completed":true,"requested_duration_seconds":10,"actual_duration_ms":10001,"adapter_index":1,"status":"completed","cleanup_status":"complete","device_count":1,"device_event_count":1,"termination_reason":"duration_elapsed"}
```

`device_count` is the unique row count; `device_event_count` counts both new and
updated device records. The UI verifies each count against the events it parsed.
`scan_completed` is true only for `status: completed` with complete cleanup.
The terminal is serialized only after StopDiscovery/filter cleanup and connection
release have been attempted. An output failure can prevent a valid terminal; the
nonzero process exit and missing terminal then mean cleanup is unknown to the
consumer.

Status values are the P013 fixed outcomes plus `output_blocked` and
`output_closed`. Termination reasons are `duration_elapsed`, `user_cancelled`,
`adapter_lost_or_unavailable`, `adapter_power_lost_or_off`,
`bluez_owner_lost_or_unavailable`, `deadline_expired`, `stream_or_input_limit`,
`stdout_blocked`, `stdout_closed`, `cleanup_failed`, and
`setup_or_observation_failed`.

Exit 0 means a completed scan with clean cleanup. Exit 1 means a structured
failure or cancellation. Exit 2 means invalid CLI syntax before D-Bus access.
The receiver also validates terminal/exit agreement; neither alone establishes
success.

## Fixed limits

- Scan duration: 1–30 seconds; Settings requests 10 seconds.
- 128 unique device rows and 4,096 input discovery messages.
- 512 streamed device/update records.
- 2 KiB maximum per JSONL record, including LF.
- 512 KiB maximum total JSONL output, including LF.
- A one-record writer queue and bounded write/cleanup deadlines.

Unknown fields, duplicate fields (including escaped-equivalent keys), wrong
sequence/session, invalid status, malformed UTF-8/JSON, truncation, or an absent
terminal cause the Settings consumer to reject the stream and mark its rows
stale. No arbitrary progress text is written to stdout.
