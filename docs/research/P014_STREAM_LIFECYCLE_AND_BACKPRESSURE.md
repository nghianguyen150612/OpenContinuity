# P014 stream lifecycle and backpressure

P014 changes the output path around P013's one production discovery state
machine. It does not create a second scan, a second BlueZ connection, or a
parallel observation classifier.

## BlueZ ownership remains P013

The CLI still validates an explicit 1–30 second request and selected adapter,
pins the current unique BlueZ owner, subscribes before setup, reads one bounded
ObjectManager snapshot, obtains the cooperative same-UID radio guard, installs
an LE-only per-client filter, and calls StartDiscovery once. Device events are
drawn from the same in-memory path-to-device map as `--json`.

After a successful StartDiscovery reply, the process attempts one matching
StopDiscovery against that pinned owner. It clears only its own filter and then
closes its dedicated connection. A failed or uncertain Start reply never causes
an unowned Stop. Owner replacement, bus loss, adapter/power loss, cancellation,
input caps, and output failure all converge on this same cleanup path. Cleanup
results remain independent from the primary scan outcome.

SIGINT/SIGTERM set the existing cancellation flag. The async loop checks output
health while waiting for BlueZ signals and yields on a short local timer, so an
idle stream is also checked. The requested scan timer and cancellation continue
to race the observation future. Output backpressure cannot make the requested
radio interval run indefinitely.

## Bounded writer

The JSONL writer runs outside the async BlueZ event loop. It receives records
through a synchronous channel with capacity one. The producer serializes one
record at a time and enforces the 2 KiB line limit, 512 KiB total byte limit,
and 512 device/update limit before enqueueing. The writer flushes each line and
records completion or a closed-pipe error. It does not retain an unbounded log.

The producer checks pending-write age during collection. A full queue or a write
that makes no progress within two seconds ends collection as `output_blocked`;
an OS write error is `output_closed`. Both statuses still run the normal P013
cleanup. After cleanup, the process waits at most two seconds for earlier output
before attempting the terminal event, then bounds the terminal write too. A
successful terminal is therefore never sent before cleanup. If stdout stays
blocked, the CLI returns nonzero and may have no terminal event; Settings reports
cleanup unknown/incomplete and marks live rows stale.

The CLI's output thread can be blocked inside the operating system while a pipe
consumer stops reading. Its bounded queue lets the BlueZ future detect that
condition; after cleanup the main process returns, and process exit ends any
detached writer thread. This is a local process-lifetime bound, not a general
memory sandbox for zbus, the kernel pipe, or the OS.

## Software evidence

Private `dbus-daemon` tests exercise the actual zbus production path. The stream
test observes one session line, one new-device line, one update line, and a
terminal line with monotonically increasing sequence numbers. The BlueZ method
log remains exactly `SetDiscoveryFilter(LE)`, `StartDiscovery`, one owned
`StopDiscovery`, and filter clear. Separate tests cover SIGTERM and a closed
stdout reader; a deliberately small unread pipe checks that a blocked consumer
does not prevent bounded cleanup. These are software lifecycle tests, not RF or
hardware qualification.

Existing P013 mock tests still cover owner loss, lost Start replies, negative
Start, duplicate signals, event/device caps, cleanup failures, and another
client's session surviving our Stop. The `--json` command continues to use the
same P013 report serializer and cleanup path.
