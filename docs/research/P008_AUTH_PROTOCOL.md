# P008 research authentication protocol

**Status:** experimental software protocol. The provisional P008 is separate
from the P005–P007 synthetic GATT v1 transport. This protocol does not unlock
or authorize Linux.

## Protocol profile

| Property | P008 value |
| --- | --- |
| Name and version | `OC-WATCH-AUTH-RESEARCH/1` |
| Algorithm | ECDSA over NIST P-256, SHA-256 |
| Public key | SEC1 uncompressed point, exactly 65 bytes, prefix `0x04` |
| Signature | IEEE P1363/raw `r || s`, exactly 64 bytes |
| Signature input | SHA-256 over the role-specific domain prefix followed by canonical bytes |
| Action | Exact ASCII string `research_confirmation` |
| Linux account | Numeric Linux UID as an unsigned 32-bit integer |
| Session ID | Fresh 16-byte CSPRNG value |
| Nonce | Fresh 32-byte CSPRNG value |
| Timestamps | Unsigned 64-bit milliseconds since Unix epoch, big-endian |
| Maximum canonical message | 1024 bytes |
| Maximum request lifetime | 60 seconds |

The signature is detached from the canonical message in both platform APIs:
the verifier receives exactly the canonical bytes and a separate fixed-size
raw-signature value. A transport envelope, if added later, must carry the
canonical bytes followed by exactly 64 signature bytes and enforce the 1088
byte total bound. No transport envelope is connected to GATT v1 in P008.

Request freshness is the half-open interval `issued_at_ms <= now_ms < expires_at_ms`;
the expiration time itself is expired. Response timestamps must also be in
that interval and no later than the verifier's current time. Clock skew is not
allowed.

The Rust implementation uses `p256` 0.13.2 (Apache-2.0 OR MIT), and obtains
request randomness through `getrandom` 0.2. The Swift implementation uses
CryptoKit `P256.Signing`. Neither implementation accepts DER as a raw
signature or silently converts between the two encodings.

## Canonical encoding

All integers are big-endian. Strings are prefixed by an unsigned 16-bit byte
length and contain only ASCII bytes from `[A-Z a-z 0-9 . _ : -]`; empty strings,
invalid UTF-8, other Unicode, excess lengths, unknown fields, trailing bytes,
and alternate encodings are rejected. Each canonical message begins with:

```text
4 bytes  "OCWA"
1 byte   protocol version = 1
1 byte   message kind: 1 host request, 2 Watch response
1 byte   algorithm identifier = 1 (P-256 ECDSA with SHA-256)
```

The host request then encodes, in order:

```text
string   host enrollment ID
string   host identity
u32      Linux UID
string   target Watch enrollment ID
16 bytes session ID
32 bytes nonce
u64      issue time in milliseconds
u64      expiration time in milliseconds
string   action: "research_confirmation"
```

The Watch response then encodes, in order:

```text
32 bytes SHA-256 digest of the complete canonical host request
32 bytes original nonce
string   host enrollment ID
string   host identity
u32      Linux UID
string   target Watch enrollment ID
string   Watch identity
16 bytes original session ID
string   action: "research_confirmation"
1 byte   decision: 1 approved, 2 rejected
u64      response time in milliseconds
1 byte   consent result: 1 explicit user decision, 2 cancelled,
         3 unsupported, 4 LocalAuthentication denied
```

Only consent result `1` is accepted by a verifier. Cancellation, unsupported
policy and denied LocalAuthentication produce no signed response in the Watch
UI. The other values are reserved for strict parsing and explicit fail-closed
handling.

The exact signed input is:

```text
ASCII("OC-WATCH-AUTH-RESEARCH/1") || 0x00 || ASCII(role) || 0x00 || canonical_message
```

`role` is exactly `HOST_REQUEST` or `WATCH_RESPONSE`. The role prefix and the
message-kind byte prevent cross-direction signature reuse. SHA-256 is applied
once by the ECDSA implementation to the complete signed input.

## Version handling

Version 1 carries one selected version byte in every signed message. There is
no downgrade fallback or multi-version negotiation in this prototype: a peer
must reject every version except `1`. A future protocol version requires an
explicitly reviewed negotiation design and a distinct versioned domain; a
receiver must never retry an unsupported version as version 1.

## Verification sequence

1. The Watch obtains the expected host enrollment from a separate pinned
   record. It parses the bounded canonical request and validates version,
   algorithm, exact action, UID/identity/enrollment bindings, timestamps,
   signature, nonce and session replay state.
2. Only a successfully verified request can produce a `VerifiedHostRequest`
   token passed to the consent component. An endpoint-provided key is never a
   trust anchor.
3. The foreground user explicitly chooses approve or reject. Approval also
   requires a supported LocalAuthentication result in the watchOS UI.
4. The Watch response binds the digest of the exact request, nonce, session,
   host and Watch enrollment IDs, host identity, UID, action, explicit choice
   and response time. It is signed with the independently enrolled Watch key.
5. The Linux-side verifier checks the response using its independent Watch
   enrollment, rechecks all bindings and freshness, verifies the signature, and
   rejects replay. A rejected decision is validly signed but remains rejection.

Rust and Swift replay guards are bounded in-memory test guards with a maximum
of 4096 entries. A full guard fails closed. They are not crash-resistant or
persistent and are not suitable for a runtime deployment.

## CLI fixture contract

`opencontinuity research watch-auth --fixture <path> --json` reads one bounded
JSON fixture containing test-only host and Watch enrollments, canonical hex
messages, raw signature hex, and a fixed verification time. The command has no
Bluetooth, key-store, account-management, privileged, or Linux authentication
side effects. It emits only verification status and always reports
`authorization: never_evaluated`; it does not print message bytes, signatures,
nonces, or keys.

The shared fixture contains public keys only. Rust test code and the Swift test
target each contain deterministic fixture scalars; these are test-only and are
not included in application targets, fixture files, logs, or published
protocol documentation.
