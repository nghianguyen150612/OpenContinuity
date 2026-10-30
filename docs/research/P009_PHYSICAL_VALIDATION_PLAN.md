# P009 physical validation plan

Use the updated [P009A physical test plan](P009A_PHYSICAL_TEST_PLAN.md) for the current enrollment UI and phase deadlines. This document retains the P009 hardware scope and non-authorization boundaries.

Physical validation must use a dedicated Linux host with an already powered adapter and a foreground Watch app. It must record actual host/Watch versions, test-only enrollment fingerprints compared out of band, BlueZ registration and cleanup, GET/read/ACK delivery, sequential write-with-response callbacks, signature verification, consent/rejection, LocalAuthentication and expiry behavior. Exercise cancellation, malformed/stale transfer data, disconnect, reconnect, owner restart, app backgrounding and replay rejection.

P009A implements the temporary test enrollment workflow but has not established physical interoperability. No result from Rust mocks, Swift package tests, simulator or hosted CI substitutes for BLE hardware evidence. No test authorizes Linux, unlocks a session, proves genuine Watch identity, proximity, wearer presence or Secure Enclave custody. Every result remains `authorization: never_evaluated`.
