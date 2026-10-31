# P014 Settings live scan UI

The Devices page retains two independent actions:

- **Refresh known devices** reads the P012 snapshot and never starts discovery.
- **Scan Bluetooth LE for 10 seconds** is an explicit P013/P014 radio action.

The active scan displays the distinct **Unverified BlueZ observations** heading,
live model rows, a unique observed-device count, elapsed time, Cancel, and the
terminal cleanup outcome. Rows carry a local source/trust label and use the
existing plain-text Card component. An empty successful scan says that no
observations arrived; it does not claim that no device is nearby. These rows are
never copied into the known-device inventory model.

## Native process bridge

`DeviceDiscoveryProvider` starts the trusted absolute CLI path with fixed
program/argument fields and no shell:

```text
devices discover --seconds 10 --events-jsonl
```

The provider stays asynchronous. Its read buffer is bounded, total stdout is
capped at 512 KiB, stderr at 4 KiB, and each line at 2 KiB. It holds incomplete
UTF-8 bytes until LF, so a multibyte character split across reads is parsed only
after the complete line arrives. Multiple lines in one read are consumed in
order. A final partial line is rejected as truncation.

`DeviceDiscoveryReport` validates exact fields, duplicate JSON keys (including
escaped equivalents), schema/source, authorization, session ID, sequence,
event order, ephemeral IDs, sanitized names, device counts, terminal status,
cleanup state, and process exit agreement. A single model row is inserted for a
new ID; an `updated` event changes that row's display name. Old callbacks are
checked against both QProcess identity and a generation number.

Malformed JSON, invalid UTF-8, excessive output, a missing or inconsistent
terminal, nonzero/terminal disagreement, process crash, or forced kill rejects
the stream. In that case cleanup is unknown/incomplete and visible rows become
stale. A process exit alone never confirms cleanup.

## Cancellation and window lifecycle

Cancel sends SIGTERM and keeps consuming output while the CLI performs bounded
owned cleanup and emits its terminal event. The provider validates that event and
the final exit code before showing a confirmed cleanup result. If the CLI does
not finish within the configured grace period, QProcess is killed and the UI
reports uncertainty. Navigating away, loss of window activity, and close request
the same graceful cancellation. Application shutdown waits boundedly for the
child and processes any available final report before retiring it.

Scanning is never automatic on launch or navigation. No polling task reads
Bluetooth state; the only timer updates local elapsed display and stale age.
Settings supports Qt 6.4.2 and newer.

## Tests

QtTest covers fragmented Unicode, coalesced and split events, malformed and
truncated lines, duplicate keys, wrong schema/session/sequence, invalid terminal
counts, exit mismatch, missing terminal, overflow, missing CLI, cancellation,
forced termination, stale rows, live model updates, and no automatic scan. The
real QML smoke test clicks Scan and Cancel, observes a new row before terminal,
verifies a name update in that same row, navigates all eight pages, and confirms
that the known-device model stays separate. Tests use a local fixture process;
they do not qualify Bluetooth hardware.
