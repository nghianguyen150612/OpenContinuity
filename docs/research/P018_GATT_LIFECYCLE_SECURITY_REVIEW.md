# P018 GATT lifecycle security review

## Implemented boundaries

P006 and P009 retain the existing service UUIDs, frame formats, radio guard, explicit runtime bounds, pinned BlueZ owner and owned-resource cleanup. Diagnostics are opt-in through `--diagnostics`, behind the existing `--experimental` guard. They store fixed enums, a random local trial label, monotonic relative time and bounded counters. Host trace writes once to stderr after cleanup; callbacks only attempt a short in-memory `try_lock`. Contended records mark evidence incomplete. The output is capped at 15 regular events plus terminal and 2 KiB.

The Watch reuses P007B storage, is disabled by default, caps event/sample history, exports at most 16 KiB and clears capture on opt-out/background. It records fixed events without peer identifiers, payloads, signatures, enrollment material or callback descriptions. Stale generation callbacks are rejected. Explicit foreground scan remains required.

The evidence checker rejects duplicate JSON keys, unknown fields, malformed and oversized records, sequence/timing/phase errors, missing terminals and mismatched labels. Its summary is never a physical qualification verdict. Submitted files and shared labels are unauthenticated.

## Limits and residual risk

BlueZ's server-side D-Bus method reply does not expose a remote peer ACK callback; host records the local operation, not peer receipt. Host and Watch clocks are independent and are never compared. The host trace does not export free-form BlueZ error strings or private resource paths. The source reports no device identity, and a local label can be copied or fabricated. The private fake BlueZ integration suite covers existing registration/cleanup behavior, while P018's new Rust trace test exercises the production P006 state path; it does not invoke a real controller.

No RF cleanup, real Wayland, signed Watch install, SE 2 callback behavior, long-read interoperability, wearer identity, proximity, Apple identity, or login authorization is established. P010 remains NO-GO; native Auto Unlock remains NO-GO. `authorization: never_evaluated`.
