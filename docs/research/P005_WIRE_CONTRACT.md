# P005 experimental GATT wire contract

> Historical P005 contract: P006 changes session/result lifecycle and removes the pending status-1 response. The current contract is [P006 session hardening](P006_GATT_SESSION_HARDENING.md) and [Watch client contract](P006_WATCH_CLIENT_CONTRACT.md).

Version 1 is a transport exercise only. All multi-byte integer fields are little-endian. The transport cannot authorize Linux.

## GATT characteristics

Service UUID: `8e6f0001-6f4e-4c4f-9b4f-4f70656e436f` (primary).

| Characteristic UUID | Flags | Contract |
| --- | --- | --- |
| `8e6f0002-6f4e-4c4f-9b4f-4f70656e436f` | read | Protocol version, one byte (`01`). |
| `8e6f0003-6f4e-4c4f-9b4f-4f70656e436f` | read | Challenge: `version:u8 || session_id[8] || challenge:u8[16]` (25 bytes). Both ID and challenge are fresh OS CSPRNG bytes. ATT reads may use offsets; a response is capped at `MTU - 1` bytes. |
| `8e6f0004-6f4e-4c4f-9b4f-4f70656e436f` | write | Ordered frame writes, with response. A D-Bus write is rejected above the negotiated `MTU - 3`. |
| `8e6f0005-6f4e-4c4f-9b4f-4f70656e436f` | read | Result: `version:u8 || status:u8 || session_id:u64[8] || payload_length:u16 || SHA256(payload):u8[32]`. Status 0 is a synthetic acknowledgment, 1 awaits completion, 2 is cancelled. Error result is `version || 0x80 || session_id || error_code`. |
| `8e6f0006-6f4e-4c4f-9b4f-4f70656e436f` | write | Control: `"OC" || version:u8 || session_id[8] || command:u8`. Command 1 completes a fully received message; command 2 cancels. |

## Data frame

Every exchange write is:

```text
magic[2] = "OC"
version:u8 = 1
session_id:u8[8]
sequence:u8
flags:u8              bit 0 START, bit 1 END; all other bits invalid
total_length:u16      required on START, zero on later chunks
payload:bytes         non-empty
```

The fixed header is 15 bytes. ATT’s minimum MTU is 23, so a minimum-MTU write carries at most five payload bytes. At MTU 247, a write carries at most 229 payload bytes. Clients must use the MTU in BlueZ’s write options and split messages; a complete message is never assumed to fit one ATT operation. Maximum reassembled payload is 1024 bytes. Sequence starts at zero and increases by one. START must be first, END must coincide with exactly `total_length`, and a partial message expires after 10 seconds. Duplicate, reordered, stale-session, malformed, oversized, wrong-version, and invalid-length frames fail closed. Any frame failure rotates the session ID/challenge; disconnect or 30 seconds idle drops that client’s state. A client’s state is keyed by its BlueZ Device object path.

After sending the final frame with END, the client writes the Complete control. The Linux side returns a non-authorizing acknowledgment with the payload’s length and digest. Cancellation returns status 2. A client reads the response through ATT offsets as needed. A new challenge starts a fresh session after expiration; there is no persistent session or replay database.

## Client states and timeouts

`disconnected → connected → read_version → read_challenge → write_frame(0..n) → complete_or_cancel → read_response → disconnected`.

The Linux process advertises for at most the CLI duration (1–300 seconds), polls adapter and Device connection state, and cleans up on SIGINT, SIGTERM, timeout, adapter removal/power loss, or D-Bus loss. The server does not prompt for approval and does not assume background execution on watchOS. Reconnection starts a new isolated client state and requires a new challenge.
