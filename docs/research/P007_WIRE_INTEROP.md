# P007 wire interoperability

`authorization: never_evaluated`

The shared JSON vector is `experiments/watchos/Tests/OpenContinuityWatchTests/Fixtures/gatt-v1.json`. Regenerate it from P006's Rust codec with `cargo run -p opencontinuity-core --example generate_p007_fixture > experiments/watchos/Tests/OpenContinuityWatchTests/Fixtures/gatt-v1.json`. Rust test `p007_shared_swift_fixture_matches_rust_wire_encoders` in `gatt_wire.rs` checks the checked-in vector's challenge size, frame bytes, control commands, complete result, cancellation result and error result against P006 encoders and SHA-256. Swift package tests consume the same fixture. Byte parity is established only after both test suites pass; Swift execution is presently not established.

The frame header is `OC || 01 || session_id[8] || sequence:u8 || flags:u8 || total_length:u16-LE`, followed by nonempty payload. At the 20-byte conservative limit, each frame carries five payload bytes. START is bit 0 and includes the total length; later frames encode length zero. END is bit 1. Messages are 1–1024 bytes, at most 256 frames. Control command 1 is Complete and 2 is Cancel.

A successful completion response is `01 || 00 || id[8] || payload_length:u16-LE || SHA256(submitted_payload)[32]` (44 bytes). Cancellation is status 2, length 9, SHA-256 of UTF-8 `cancelled`. Errors are `01 || 80 || id[8] || error_code` (11 bytes). This matches the current Rust `status_response` implementation. The earlier local codec's status-1 assumption was corrected.

The client checks `maximumWriteValueLength(for: .withResponse) >= 20` but never interprets it as an ATT MTU. Each write uses response mode and waits for `didWriteValueFor` before the next write. Challenge and response reads must arrive as complete values of exactly 25 and 44 or 11 bytes. CoreBluetooth has no public explicit offset argument for these reads; long-value behavior must be tested with a physical Watch and the Linux host. Partial values fail closed.

The Rust test validates deterministic vectors against the existing Rust encoder and server response layout. It does not exercise BLE, BlueZ over RF, Swift execution, or runtime cross-language interoperability. Do not treat matching checked-in bytes alone as runtime interoperability.
