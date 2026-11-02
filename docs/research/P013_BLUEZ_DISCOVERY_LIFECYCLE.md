# P013 BlueZ discovery lifecycle

P013 is an explicit radio operation, separate from P012's read-only inventory.
The locked zbus version is **5.12.0**. The implementation uses its asynchronous
Connection, typed method bodies, MatchRule, MessageStream and receive Sequence
APIs. It does not use the inventory's detached synchronous snapshot worker.

1. CLI parsing requires `devices discover` and `--json`. Duration defaults to
   10 seconds; only integers 1–30 are accepted. Adapter index defaults to 1 and
   must be 1–16. Invalid arguments exit before any bus access.
2. Register SIGINT/SIGTERM flags, open a dedicated system-bus connection and
   resolve the current unique `org.bluez` owner. Install owner-change and
   owner-filtered signal subscriptions before calling StartDiscovery.
3. Read ObjectManager once. Select adapters in lexical object-path order, as in
   P012, using a one-based index scoped to **this invocation's snapshot**. A
   previous inventory index is not a durable adapter identity. Require a known
   `Powered=true` value. Never write Powered, Discoverable or Pairable.
4. Acquire a cooperative abstract Unix-socket guard for the effective UID,
   configured system-bus address and adapter. P007/P009 GATT servers acquire the
   same guard before registration. Collision returns `adapter_busy`. This is not
   exclusion of other applications, users or differently addressed bus aliases.
5. Require `GetDiscoveryFilters` to include Transport, recheck the BlueZ owner,
   and send `SetDiscoveryFilter({Transport: "le"})`. Unsupported filtering fails
   closed. There is no BR/EDR fallback.
6. Send StartDiscovery exactly once. Set `scan_started` only after its successful
   reply. Keep that reply's receive sequence; discard queued signals received
   before or with the reply. Cached ObjectManager Device1 entries are never
   copied to scan results.
7. Observe owner-filtered ObjectManager InterfacesAdded/InterfacesRemoved and
   PropertiesChanged events. Only selected-adapter Device1 paths with activity
   fields (RSSI, ManufacturerData, ServiceData, AdvertisingData or TxPower) can
   produce a record. Name-only changes are ignored. Activity values are never
   serialized. Duplicate paths update the same ephemeral record. Events from
   another sender, adapter or interface cannot produce a record.
8. A monotonic timer, SIGINT/SIGTERM, device/event cap, adapter loss or power loss,
   bus error or BlueZ owner change ends collection. The timer is polled ahead of
   the event stream, so a signal flood cannot indefinitely postpone expiry.
9. If StartDiscovery succeeded, attempt **one** StopDiscovery, even after
   cancellation or observation errors. Clear the client's filter separately,
   including when StopDiscovery fails. Both calls target the original unique
   owner and selected path; never redirect cleanup to a replacement daemon.
10. Explicitly close the connection and release subscriptions and guard. Serialize
    only the sanitized report. No daemon or background worker survives the CLI.

## Deadlines and ownership

Connection plus setup share a five-second deadline. Method replies have a
two-second timeout. StopDiscovery and clearing the filter each have a separate
two-second bound; connection close has a one-second bound. The normal outer
budget is requested duration + ten seconds (at most 40 seconds), subject to OS
scheduling. Actual duration is measured, not assumed or truncated to the request.

Never infer ownership from Adapter1.Discovering: that property is shared. BlueZ
maintains discovery sessions and filters per D-Bus client. Releasing this client's
session does not release another client's session. BlueZ may continue scanning
when another client still owns discovery. Empty SetDiscoveryFilter clears only
this client's filter. No global controller configuration or StopDiscovery on a
borrowed connection is used.

A StartDiscovery timeout/cancellation can leave acquisition uncertain. In that
case `scan_started=false`, no unconfirmed StopDiscovery is sent, and cleanup is
reported incomplete. Filter removal and connection close still run. BlueZ's
client-disconnect cleanup is the fallback, including after SIGKILL; forced death
cannot prove that explicit StopDiscovery executed. A negative permission/busy/
unsupported/power reply is a confirmed non-acquisition and needs only filter
cleanup. Failures are fixed status enums; raw D-Bus exception strings are omitted.

## What an observation means

BlueZ merges filters across clients and can emit cached, repeated or other-client
activity. Even with our LE transport request, a signal is not an independently
verified RF sighting or an attested LE-only event. Records say
`bluez_event_during_session`; they do not say nearby, genuine Apple hardware or
authenticated. A successful empty scan does not prove an absence of peripherals.

Reference: [BlueZ Adapter1 public API](https://github.com/bluez/bluez/blob/master/doc/org.bluez.Adapter.rst).

P014 adds an incremental JSONL presentation mode over this same lifecycle. It
does not add a BlueZ connection or StartDiscovery call. See the [P014 stream
lifecycle and backpressure design](P014_STREAM_LIFECYCLE_AND_BACKPRESSURE.md).
