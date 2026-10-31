# P014 privacy and security review

## Trust boundary

JSONL is an untrusted presentation stream from the local CLI. It is versioned
separately from P013's final JSON document and is independently parsed by Qt.
The parser requires exact fields, one session, monotonic sequence numbers,
session-scoped IDs, known enums, event ordering, bounded counts, and terminal/
process-exit agreement. Invalid or incomplete output cannot be shown as a
successful scan.

## Data minimization

Public stream fields contain only a random scan-session token, scan-scoped
counter IDs, sanitized display names, event provenance, bounded counts, fixed
status enums, and lifecycle timing. The stream does not include Bluetooth
addresses, BlueZ object paths, RSSI, raw advertisements, manufacturer/service
payloads, IRKs, pairing credentials, persistent device IDs, Apple Account data,
P008/P009 transcripts, or private keys. Device names are local untrusted labels
and can still contain personal data despite sanitization.

The device ID uses a truncated session token plus encounter index. It changes
between invocations and does not become an inventory key. Rows remain separate
from the P012 known-device snapshot. There is no telemetry, upload, pairing,
connection, Bluetooth trust write, privileged helper, or authorization call.
Every event retains `authorization: never_evaluated`.

## Resource and lifecycle controls

The CLI bounds duration, input events, unique devices, streamed records, line
bytes, total bytes, the asynchronous writer queue, write waits, setup, cleanup,
and connection release. Output backpressure terminates collection and follows
P013 cleanup. Confirmed StartDiscovery ownership is required for StopDiscovery;
the call is attempted once against the original BlueZ owner. Filter cleanup is
separate. Another client's discovery session is not stopped.

Settings caps QProcess read buffers/output, parses complete lines incrementally,
rejects duplicate/unknown fields, checks process generation, and marks rows stale
after cancellation, failure, timeout, crash, invalid terminal, or forced kill.
QML displays strings as plain text. Closing or navigating away requests graceful
cancellation and a bounded fallback; forced termination never claims confirmed
cleanup.

## Findings and residual limits

- BlueZ merges discovery filters. An event tagged `bluez_event_during_session`
  is not independent RF proof, distance, proximity, Apple hardware identity, or
  authentication.
- Cached ObjectManager rows are not sightings, and name-only changes do not
  produce records. Device events come from the same sanitized in-memory map as
  the P013 report.
- A lost StartDiscovery reply remains uncertain; no speculative Stop is sent.
  BlueZ disconnect cleanup is only a fallback, and a kill/daemon failure cannot
  prove that explicit cleanup completed.
- The app-selected CLI, Qt runtime/plugins, D-Bus daemon, BlueZ, and kernel remain
  trusted local components. Normal kernel and framework buffers are outside a
  strict whole-process memory quota.
- The cooperative radio guard excludes only same-UID OpenContinuity sessions
  using the same configured bus/adapter. It is not a global radio lock.
- Mock D-Bus, Qt, CI, and software event evidence do not establish Arch RF
  behavior or Apple Watch interoperability.

No issue in this review changes P010 Apple Watch SE 2 qualification: it remains
**NO-GO**. Apple Watch Auto Unlock, AirDrop/AWDL, AirPods controls, and Linux
authorization remain unsupported.
