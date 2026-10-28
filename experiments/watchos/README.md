# watchOS experiment (incomplete)

`GattWireCodec.swift` is an uncompiled initial implementation of the P006
synthetic wire format. It requires Foundation and CryptoKit. No watchOS app,
CoreBluetooth integration, or tests are present yet. Do not use it as evidence
of interoperability or authentication. Cancellation validates the server's
literal `cancelled` payload, as defined in `apps/opencontinuity-cli/src/gatt.rs`.
