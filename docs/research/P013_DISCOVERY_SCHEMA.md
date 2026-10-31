# P013 discovery JSON schema v1

Command: `opencontinuity devices discover --seconds 10 --json`.
Optional `--adapter-index 1` selects a one-based index in this invocation's
adapter snapshot. Default duration is 10 seconds; accepted duration is 1–30.
Flags can be reordered; duplicates, unknown flags, paths, signs and non-integers
are rejected. `--json` is required. No command accepts shell text.

This schema is independent from P012 `devices list --json`, which is unchanged.

```json
{
  "schema_version": 1,
  "source": "bluez_le_discovery_session",
  "scan_requested": true,
  "scan_started": true,
  "scan_completed": true,
  "requested_duration_seconds": 10,
  "actual_duration_ms": 10001,
  "adapter_index": 1,
  "status": "completed",
  "cleanup_status": "complete",
  "devices": [
    {
      "id": "scan-1234567890abcdef-001",
      "display_name": "Example unverified peripheral",
      "provenance": "bluez_event_during_session"
    }
  ],
  "authorization": "never_evaluated"
}
```

The example is illustrative, not hardware evidence. No result fixture is
presented as a real scan.

| Field | Contract |
| --- | --- |
| schema_version / source | Exactly 1 / `bluez_le_discovery_session`. |
| scan_requested | True for every accepted invocation, even if preflight fails. |
| scan_started | True only after a successful StartDiscovery reply. |
| scan_completed | True only for normal bounded expiry with confirmed cleanup; false for all other outcomes. |
| requested_duration_seconds | Integer 1–30; default 10. |
| actual_duration_ms | Nonnegative monotonic collection time, excluding setup and cleanup; zero if not confirmed started. |
| adapter_index | Selected one-based current-snapshot adapter index, or null before selection. Not a persistent identifier. |
| status | Explicit outcome below. |
| cleanup_status | `not_needed`, `complete` or `incomplete`; independent from the terminating error. |
| devices | At most 128 sanitized observations, empty if not confirmed started. |
| id | Random 64-bit scan prefix and three-digit encounter index; not stable between sessions. No path/address-derived public ID. |
| display_name | P012 control/format sanitization, 80 Unicode scalar values; address-shaped names and BlueZ paths replaced by `Unnamed Bluetooth device`. Names remain personal, untrusted local labels. |
| provenance | Exactly `bluez_event_during_session`. |
| authorization | Always `never_evaluated`. |

Status values: `completed`, `cancelled`, `no_adapter`, `adapter_not_powered`,
`unsupported_le`, `adapter_busy`, `bluez_unavailable`, `system_bus_unavailable`,
`access_denied`, `timeout`, `malformed_data`, `cleanup_incomplete`, `data_limit`,
`unsupported_platform`. Adapter loss uses `no_adapter`; power loss uses
`adapter_not_powered`; owner change/bus loss during collection uses
`bluez_unavailable`. `system_bus_unavailable` describes connection failure.

`cleanup_incomplete` replaces an otherwise completed outcome when cleanup fails.
On cancellation or another error, that original status remains, with
`cleanup_status=incomplete` when appropriate. A lost StartDiscovery reply never
sets scan_started and reports uncertain cleanup. Complete cleanup can accompany
an access-denied start because clearing the owned filter succeeded.

Exit code **0** means completed; **1** means any other structured outcome.
Invalid CLI syntax exits **2**, with fixed usage text on stderr and no JSON or bus
access. Structured failures have no stderr. A valid empty result still exits 0.

Only allowlisted fields are serialized. There are no addresses, object paths,
RSSI, manufacturer/service payloads, secrets, pairing data or persistent IDs.
At most 4,096 events, 128 records and 80 scalar values/name bound the report below
64 KiB. Queues hold 64 messages each. Setup snapshots are capped at 1 MiB/1,024
objects/16 adapters; event bodies at 64 KiB; input names at 4 KiB. These are
application parsing limits, not a whole-process memory quota against a hostile
local bus. See the [security review](P013_DISCOVERY_SECURITY_REVIEW.md).

Qt validates exact members, duplicate members (including escaped equivalents),
types, enums, ranges, state consistency, ephemeral IDs, provenance and sanitized
names. It rejects unknown schema, malformed/truncated/oversized output, stderr
and exit/schema mismatches. Settings always requests and validates 10 seconds.
