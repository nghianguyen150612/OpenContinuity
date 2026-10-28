# P004 enrollment and cryptographic contract

## P003 limitation and P004 correction

P003's simulator asked the endpoint under test for its `public_key()` and then used that value as the verifier's expected key. That makes the endpoint self-authenticating. P004 replaces that flow in the simulation with a separate `MockEnrollmentRecord` created before transport exchange. It contains an enrollment ID, Linux host identity, local account identity, Watch endpoint identity, verification key, explicit algorithm and key version, lifecycle state, and revocation flag. `ReplayGuard::verify_enrolled` validates the active record and checks those bindings before signature verification. The endpoint does not supply or replace the trust anchor.

Tests cover a matching enrolled endpoint, unknown enrollment, revoked/inactive enrollment, unsupported algorithm, wrong host/account/watch binding, and a same-identity endpoint signed by a different key. The substitution signature fails verification. These are deterministic Rust mock tests; the record is not persistent and is not a production enrollment ceremony or key store.

## Algorithms and separation

The current Rust test exchange is explicitly `Ed25519MockV1`, implemented with `ed25519-dalek`. Its deterministic signing seeds and nonce are mock-only. The API record uses a tagged algorithm and version; unsupported P-256 selection fails closed instead of interpreting bytes as Ed25519. Existing P003 JSON fixtures remain supported.

Apple documents CryptoKit P-256 ECDSA and Secure Enclave P-256 APIs; Apple Watch SE 2 key availability, custody and signing are **UNKNOWN** until an app is built and exercised on that device. See [watch app feasibility](P004_WATCH_APP_FEASIBILITY.md). A future interop profile should use P-256/SHA-256 with SEC1 uncompressed public keys (65 bytes) and fixed-width raw `r || s` signatures (64 bytes), with an independently reviewed Rust `p256` verifier. P004 has not implemented or tested this wire profile. Ed25519 and P-256 bytes must never be interchanged.

## Required future enrollment record

A separately reviewed enrollment flow must bind at least:

- Random enrollment identifier and lifecycle/revocation state.
- Stable Linux host identity and local account identity.
- Watch app endpoint identity.
- Watch signature algorithm, algorithm version, and public key.
- Linux host authentication algorithm/version and public key.
- Enrollment creation/expiry metadata and an explicit revocation mechanism.

P004 only models the Watch key side for deterministic tests. It has no database, secret store, UI enrollment ceremony, or production host key custody.

## Mutual authentication and protocol transcript

One-sided Watch signatures do not authenticate the Linux GATT server. A malicious peripheral could ask the user to approve a request while displaying a spoofed host name. Before showing approval UI, a future Watch app must validate a host certificate or pinned host public key established during explicit enrollment, then verify a host signature over a domain-separated challenge transcript. The Watch response must sign the same version, host/account/session/action, nonce, issue/expiry times, approval decision, user-confirmation bit, and Watch identity. Linux must verify the Watch signature against the separately enrolled key.

The P004 mock challenge format binds version, nonce, host, account, session, action, timestamps; its approval signature binds those fields plus endpoint identity, decision, explicit mock confirmation and response time. Host-signature verification and real enrollment remain **NOT IMPLEMENTED**. Any real protocol test must reject a challenge whose host proof does not validate before presenting it to the user. Use OS CSPRNG output for a real nonce; the deterministic SHA-256 counter in this source is strictly mock/test-only.

Use a strict message cap of 4096 bytes, exact 32-byte mock nonce, at most 30-second challenge lifetime, unknown-field rejection, explicit protocol version, domain separation, and replay set for current-process tests. A production replay store needs crash-safe semantics and durable revocation, neither of which P004 provides. BLE fragmentation/reassembly is not implemented; a future GATT framing layer must cap total reassembled bytes and reject partial, duplicate, reordered, timed-out, or overlong messages.

## Security meaning and limits

An approved, valid signature in the mock means only that the test key signed the mock transcript and the test state accepted it. It proves no physical wearer presence, Secure Enclave provenance, distance, resistance to relay, or integrity of the Linux host. A real signed response is not itself proof of wearer presence. RSSI, advertisement data, pairing state, iPhone proximity, or low latency must never be an authentication fallback. P004 returns `authorization: never_evaluated` in every report and has no privileged Linux integration.
