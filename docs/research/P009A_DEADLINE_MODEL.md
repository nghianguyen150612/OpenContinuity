# P009A authenticated transport deadline model

P009A treats the P008 signed `expires_at_ms` as the cryptographic outer bound. Local phase limits use monotonic clocks. No transport deadline or consent action changes the signed issue or expiry timestamps. If the request expires, the host must start a new server session that signs a new request with a fresh nonce and session ID.

## Phase limits

| Phase | Limit | Starts at | Deadline behavior |
| --- | ---: | --- | --- |
| P008 signed request validity | 60 seconds maximum | Request signing at readiness | Signed wall-clock `issued_at_ms` / `expires_at_ms` are verified by P008. Linux and Watch also map the remaining interval to monotonic time. Expiry retires the session. |
| BlueZ setup | 30 seconds total | Before system D-Bus connection/preflight | Covers read-only adapter preflight, GATT application registration and advertisement registration. Each D-Bus method call has a 2-second timeout. No P008 request is signed before both registrations succeed. Expiry fails setup, cleans up owned registrations and reports the setup stage. |
| Authenticated server maximum runtime | 1–60 seconds | After successful BlueZ registration and request readiness | Ends the Linux process's active advertisement window. It cannot outlive the signed request. |
| Watch scan | 15 seconds | User starts Authenticated Research | Monotonic callback timer; timeout ends the scan. |
| Watch connection | 15 seconds | Explicit peripheral selection | Monotonic callback timer; timeout terminates the session. |
| Watch service/characteristic discovery | 12 seconds per discovery phase | Connection callback / service discovery | Monotonic callback timer; timeout terminates the session. |
| Watch BLE callback/inactivity | 8 seconds per operation | A read, control write or response write is issued | Applies to each CoreBluetooth callback. The Rust transport also has an 8-second inactivity limit, reset only by accepted progress. |
| Host request transfer | 20 seconds absolute; 8 seconds idle | First client ownership claim, normally the status read | GET/read/ACK progress does not extend the absolute deadline. Both limits are clamped to signed expiry. |
| Waiting for a client | 45 seconds | P009 readiness | Ends before the 60-second signed request limit. No connection means the request is retired. |
| Waiting for consent | 25 seconds maximum | Host accepts request `COMPLETE` after all request frames are ACKed | Includes foreground decision and LocalAuthentication. It is clamped to signed expiry; slow LocalAuthentication cannot extend the signed request. |
| Response transfer | 15 seconds absolute; 8 seconds idle | First accepted Watch response frame | Starts after consent, not at session creation. It is clamped to consent and signed-expiry deadlines. Each Watch write-with-response callback also has an 8-second limit. |

The active Rust loop checks phase deadlines at 100 ms intervals and checks the BlueZ owner/controller state at least once per second. D-Bus GATT calls have the configured 2-second method timeout. Watch callback timers and transport assemblers use monotonic uptime; wall-clock timestamps are reserved for P008 freshness and signature checks.

## Freshness and clock behavior

At readiness, Linux signs a P008 request with a 60-second maximum lifetime, verifies the request locally, and records both its signed wall-clock expiry and a corresponding monotonic deadline. BlueZ setup time therefore does not consume the Watch's request window. If the temporary enrollment expires during setup, initialization fails before signing.

The Watch verifies the host signature and signed timestamps after complete reassembly. It then stores the signed expiration as a monotonic deadline for consent and response transport. After LocalAuthentication returns, the Watch checks the signed wall-clock expiry and the monotonic deadline before signing or sending a response. Linux repeats expiry checks when claiming ownership, accepting protocol commands, assembling the response and verifying the completed response.

Wall-clock movement can cause an early rejection. Moving the wall clock backward cannot extend the local monotonic deadline. A failed or expired request is terminal; restarting the server does not reuse its nonce, session ID, response assembler or replay state.

## Boundary tests

Rust tests use supplied `Instant` values to cover long active transfers, idle timeouts, late response start, setup delay, and the exact request/response phase boundaries. Swift transport tests use controlled monotonic millisecond values. The LocalAuthentication expiry regression verifies that a simulated completion after the signed deadline produces no response. These tests do not establish callback timing over a physical BLE link.
