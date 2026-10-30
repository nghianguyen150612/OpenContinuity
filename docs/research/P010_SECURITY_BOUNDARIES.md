# P010 security boundaries

`authorization: never_evaluated`

P007 authenticates no Watch: SHA-256 checks only synthetic transcript integrity.
P009 authenticates configured temporary research keys, not Apple identity, wearer
presence, secure proximity or Linux authority. A transport ACK is not cryptographic
verification; even a valid signed approval never authorizes Linux.

Both public protocols, independent fingerprint pinning, UID/Watch bindings, replay
checks and P008 absolute expiry <=60 seconds remain unchanged. No background
approval, production enrollment, persistent Watch key store, Secure Enclave claim,
RSSI authorization, pairing-derived trust, Apple-private protocol, PAM, polkit,
login manager or session unlock is added. P011 is outside scope.

Diagnostics are off by default, accept fixed categories and bounded numbers, and
retain at most 64 recent events/samples with fixed-stage bounded aggregates.
Watch opt-out discards the trace; host timing is emitted only with explicit
OC_P010_DIAGNOSTICS=1 on an already authorized bounded server. No keys, envelopes,
challenges, addresses, peripheral IDs, account data or credentials are accepted by
the timing API. Existing server error output is not automatically safe to publish;
retain only allowlisted classifications after review.

Evidence manifests reject unknown fields, unknown domains/stages, missing proof
references, incomplete successful reads and inappropriate outcomes. Trace events
are supporting observations, not acceptance records. Simulation cannot satisfy
`--require-physical` when honestly declared. JSON provenance is an operator
assertion, not anti-tamper attestation: manual review of actual proof is required.
A malicious operator can lie in JSON. No new evidence signing key or claim of
signed qualification provenance is introduced; only P009 messages are signed.
Public version/model metadata and local proof files still require privacy review.

Host trace source `host_preflight` describes local host measurements, not radio
proof. Watch simulator/package fixtures default to `simulated`; the actual device
app labels its supporting trace `physical`. Neither trace format is accepted as a
qualification manifest. Tests/CI only demonstrate software behavior. Missing
physical prerequisites leave the PR Draft and the hardware decision NO-GO.
