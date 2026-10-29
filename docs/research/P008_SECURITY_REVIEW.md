# P008 security review

## Scope reviewed

The review covers the isolated `OC-WATCH-AUTH-RESEARCH/1` codecs, Rust P-256
verification and fixture CLI, Swift CryptoKit implementation, in-memory
test-only enrollment records, and foreground consent component. It does not
cover a deployed transport or production enrollment system.

## Controls implemented

- Signed host requests and Watch responses use P-256 ECDSA with SHA-256,
  distinct role domains and strict raw 64-byte signatures.
- Both peers require separately supplied, test-only pinned enrollment records.
  Endpoint-supplied keys never establish trust.
- The canonical codec has fixed field order, bounded lengths, strict ASCII
  identifiers, one exact action, strict version/algorithm handling, and no
  trailing bytes or DER conversion.
- Requests bind the host, numeric Linux UID, target Watch enrollment, 16-byte
  session ID, 32-byte CSPRNG nonce, issue/expiry time and action.
- Responses bind the exact request digest, original nonce/session, host and
  Watch enrollments, host/account, action, explicit decision, response time
  and manual consent result.
- Rust and Swift reject unknown, inactive, revoked, substituted, malformed or
  wrong-role keys; invalid signatures; expired/future messages; replay; wrong
  bindings; unsupported versions/algorithms; missing consent; and protocol
  signature reuse.
- The CLI reads only bounded fixture files and has no privileged or system
  authentication actions. All reports retain
  `authorization: never_evaluated`.

## Residual risks and missing gates

- Enrollment records and replay guards are test-only, in-memory state. Restart,
  crash or storage loss can erase replay knowledge; persistent crash-safe
  storage and authenticated enrollment are not implemented.
- Trust-on-first-use is prohibited. Without an out-of-band pinning ceremony,
  test records prove no genuine host or Watch identity.
- App signing keys and app-reported LocalAuthentication results are not
  remotely attested proof of hardware, wearer or software integrity.
- No secure proximity, anti-relay, BLE identity, or distance-bounding property
  is established. A wormhole can relay correctly signed requests and replies.
- Wall-clock freshness assumes both endpoints have suitable clocks. No clock
  skew allowance is granted; strict bounds may reject messages during clock
  drift.
- A compromised host can request an action that it signs. The UI limits this
  profile to the exact research action and displays its account/host binding;
  this is not an OS authorization boundary.
- The replay cache is bounded at 4096 entries and fails closed when full. It is
  an in-memory test mechanism, not a production storage design.
- Secure Enclave behavior, Watch SE 2 key custody, LocalAuthentication runtime
  behavior, real BLE exchange and signed physical installation remain
  **UNVERIFIED** or **NOT RUN**.

## Explicitly excluded

P008 does not modify PAM, polkit, login managers, desktop unlock, account
management, Bluetooth pairing, RSSI policy or Apple-private Auto Unlock. It
does not extract Apple Account credentials. The P007 synthetic SHA-256 GATT
acknowledgment is not authentication and is not used by this protocol. P008
does not claim to implement Apple's proprietary Watch-to-Mac protocol.

Signature verification can report only `signature_verified` or
`signature_verified_rejected` for a research fixture. It never yields
`authorized` or `linux_unlocked`.
