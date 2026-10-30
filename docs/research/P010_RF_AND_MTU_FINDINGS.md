# P010 RF, MTU and timing findings

`authorization: never_evaluated`

On 2026-10-30 the available executor was Debian 13, x86_64, kernel 6.18.57.
It was not the owned Arch host. No `/sys/class/bluetooth` controller was exposed;
BlueZ CLI/management utilities were unavailable, and system-service/D-Bus probes
failed. See [sanitized preflight](fixtures/P010_CLOUD_PREFLIGHT_2026-10-30.json).
No adapter was selected. Powered state, firmware, LE roles, manager availability,
advertisement capacity and registration permissions are BLOCKED or NOT RUN as
recorded separately. A successful shell-script exit is not a capability PASS.

| Physical stage | Outcome | Observation |
| --- | --- | --- |
| BlueZ GATT/advertisement registration | BLOCKED | No controller/BlueZ test host |
| Independent RF advertisement observation | BLOCKED | No owned scanner/radio |
| Watch discovery and characteristic reads/writes | BLOCKED | No physical Watch |
| P007 25-byte challenge / 44-byte digest exchange | BLOCKED | No physical path |
| P009 signed exchange and on-device LA | BLOCKED | No devices/enrollment |
| ATT MTU and CoreBluetooth long read | BLOCKED | No actual ATT observation |
| Disconnect/reconnect and BlueZ cleanup | BLOCKED | No registered physical resources |
| Physical latency and signed deadline feasibility | BLOCKED | No radio measurements |

The current conservative P009 framing carries five data bytes per 20-byte value,
up to 218 request frames. Each request frame needs a GET write callback, read
callback and ACK write callback. This is a potential round-trip bottleneck, not a
measured failure. New timing aggregates report real callback latency when opted in.
Do not publish invented discovery/connection/consent/cleanup numbers from this run.

P007 challenge/result exceed the default ATT value capacity. CoreBluetooth
`readValue(for:)` offers no public caller-selected offset. Existing BlueZ offset
handling and mock long-read tests do not establish Apple long-read behavior.
Privately approved btmon inspection can identify Read/Read Blob, MTU, offsets and
lengths; publish derived metadata only. Never commit packet captures. The Watch's
maximumWriteValueLength(.withResponse) is a value-size observation, not direct MTU
negotiation evidence. On truncated reads, retain exact actual/expected lengths,
callback outcome, versions and sanitized ATT observations before changing code.

No transport optimization is justified by measurements yet. If hardware shows the
request cannot fit its 20-second delivery window/60-second signed lifetime, the
smallest candidate is using larger frames within existing version-1 MTU-aware
codec limits after confirming actual public-API value limits and both endpoints'
status/frame-count assumptions. That requires separate compatibility review and
tests. Fewer round trips through notifications would need a reviewed protocol
change. Neither proposal is implemented here. Expiry and replay checks stay strict.
