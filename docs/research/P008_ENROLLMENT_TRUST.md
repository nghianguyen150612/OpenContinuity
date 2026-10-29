# P008 enrollment and trust boundaries

P008 ships an explicit test-only enrollment model so each direction verifies
against a trust anchor stored separately from the signed endpoint message.
There is no automatic trust-on-first-use and no BLE discovery or pairing path
that creates an enrollment.

## Test records

Each record identifies the enrollment, role identity, P-256 algorithm and key
version, lifecycle state, public key, and `test_only: true` provenance.

- A host record binds host enrollment ID, host identity, numeric Linux UID and
  host public key.
- A Watch record binds Watch enrollment ID and identity, host enrollment ID,
  host identity, numeric Linux UID and Watch public key.

Both implementations reject unknown, suspended, revoked, non-test, wrong
algorithm, wrong version, malformed or substituted records. Public keys must be
65-byte SEC1 uncompressed P-256 points. Keys supplied in a request or response
are not accepted as enrollment records.

`opencontinuity research watch-auth` is offline and fixture-only. The Rust
request builder can obtain a nonce and session ID from the OS CSPRNG, but no
runtime CLI, Bluetooth, account, key-store or authorization flow calls it.
Test private scalars exist only in Rust unit-test code and the Swift unit-test
target. Shared CI fixtures contain public keys and signatures only.

## Trust stages not implemented

These remain prerequisites for any production or privileged design:

- Out-of-band verification of host and Watch identities and public keys.
- An authenticated enrollment ceremony and protected persistent key pinning.
- Keychain or Secure Enclave key custody qualification on Watch SE 2.
- Crash-resistant replay storage, expiration cleanup, revocation and rotation.
- Trusted Watch device attestation, if Apple exposes a suitable public API.
- Linux authorization design and independent security review.

The P008 verifier authenticates a test key under a test enrollment record. It
does not prove that the key belongs to a genuine Apple Watch, that a wearer is
present, that the Watch is nearby, or that the host is uncompromised. A relay
can carry valid messages. BLE pairing and RSSI are not identity or secure
distance proofs.

No production key or private key is present in source compiled into the Watch
app, CI fixtures, logs or documentation. `ResearchConsentView` receives a
short-lived in-memory signing key from a test harness; it does not generate,
persist, export or enroll a production key. Secure Enclave support and actual
Watch SE 2 key custody remain **UNVERIFIED**.
