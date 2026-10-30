# P007B long reads, cancellation and reconnect

`authorization: never_evaluated`

Physical behavior remains UNKNOWN. The protocol is unchanged: version 1; challenge 25 bytes; 15-byte frame header; 20-byte conservative writes with 5-byte bodies; Complete 1; Cancel 2; status 0 success/status 2 cancelled; 44-byte result/11-byte error. Verify the session, length and SHA-256 exactly. Cancellation hashes the literal nine UTF-8 bytes `cancelled`. No digest authenticates a device.

## Long reads

Enable Watch Diagnostics explicitly and record expected and actual lengths from accepted callbacks. The challenge event must be 25, the successful result 44 (or a classified 11-byte error). Record callback success/error/timeout and test stage separately. Do not publish bytes, session IDs or challenges. The application rejects incomplete values; it never concatenates unrelated callbacks. CoreBluetooth readValue has no public caller-selected offset argument.

For server-side ATT behavior, the existing BlueZ ReadValue path validates offset and MTU, then returns at most MTU minus one bytes. Result ownership persists until its final fragment is consumed. Source/mock tests establish this path, not actual Watch behavior. An operator may use a privately stored, explicitly approved btmon capture on the isolated owned host to determine ATT Read/Read Blob offsets and lengths. Captures contain persistent identifiers and raw payloads: never commit or publish them. Publish only derived counts, offsets, MTU if actually observed, lengths and error classifications after review. Watch diagnostics alone cannot prove how many ATT procedures occurred. If packet capture is unavailable, record underlying read count UNKNOWN.

If truncated: record exact expected/actual lengths, callback error class, server/OS/firmware versions and observed ATT behavior. Reproduce with an independent owned central. Investigate final-fragment and result-retention semantics before designing any remedy. A public-API remedy must have a regression test and preserve backward compatibility or explicitly version the protocol. No incompatibility is reproduced here and no wire changes are justified.

## Cancellation and reconnection matrix

Run each trial independently on a fresh explicit connection. Record every stage PASS/FAIL/BLOCKED/NOT RUN with UTC and sanitized evidence:

- Cancel while frames are being acknowledged: cancel_requested, serialized Cancel after the current write, response 44, status 2/length 9/digest verified. Cancel after a frame's acknowledgement but before Complete is a timing-sensitive case; repeat or record NOT RUN if the UI window cannot be exercised. Once Complete/result is in flight, use Disconnect; the UI does not promise a second Cancel after completion.
- Disconnect while writing and while a response is unread. Confirm terminal cleanup, no ongoing accepted callbacks, then explicit scan/connect and a complete new challenge. Compare freshness privately without publishing either ID; record only equal/different. If repeated or disconnect not confirmed, abort/restart the experiment and report FAIL/UNKNOWN rather than claim fresh interoperability.
- Inject stale frames only with a controlled owned test central, retain sanitized rejection codes. Existing deterministic tests are not physical rejection evidence.
- Suspend during scan/connect/ready/write/result and after Completed/Cancelled. Confirm scan stops, session invalidates and connection retires. Returning foreground must not auto-reconnect.
- Exercise server expiry, SIGINT and SIGTERM separately. Record server cleanup fields and independent BlueZ/RF observations. An optional explicitly approved BlueZ restart on an isolated host is disruptive; observe failure/cleanup and recover with a new server trial, not a hidden retry.

P006A orders disconnect signals actually delivered before subsequent calls on its D-Bus connection. Physical disconnection, a new advertisement or a new connection does not imply immediate signal delivery, device authentication or relay resistance.
