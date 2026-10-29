# P008 — Foreground Watch Authenticated Approval Protocol Prototype (No Unlock)

**Status:** STARTED — scope/branch only; implementation, Swift/Rust crypto parity, key custody and physical Watch verification NOT RUN.
**Branch:** `feat/p008-watch-authentication-prototype`
**Base:** `main` at `1cf956bd7356df2e7e2800da5657c01d12f7b256` (P007/P007A merged).
**Parallel unresolved work:** P007B PR #8, `research/p007b-watch-bluez-physical-interop` at `8cf11d093d26d5e0c3d20d63b7bc33f47ca03f2f`, remains DRAFT and unmerged because real BlueZ/Watch interoperability has NOT RUN. P008 MUST NOT depend on its unmerged source or claim physical BLE support.

## Provenance
The original P001–P060 milestone definitions were never recovered. This P008 is an explicitly **provisional** Watch-track research implementation following merged P007, not the verified original P008. Outstanding Foundation/Qt6/QML Settings and production platform Bluetooth adapters remain unfinished.

## Objective
Introduce a separate, rigorously tested **research-only, foreground, explicit-consent, mutually authenticated message protocol** between a Linux host and Watch using independently pinned keys, versioned domain-separated transcripts and replay protection. It must demonstrate genuine cryptographic signatures and verification on deterministic fixtures and hosted Apple tooling where feasible, not merely SHA-256 acknowledgments. It must not authorize any Linux session, connect to PAM/polkit/login managers, claim real Watch identity, or automatically operate over BLE.

**Most important boundary:** P005–P007's v1 GATT session ID/challenge and SHA-256 transport result are not authenticated. Do not replace, overload or silently reinterpret them. The P003/P004 Ed25519 deterministic mock and enrollment records are test-only; reuse lessons, not the signing seeds, trust claims or incompatible wire encoding. P008 is a separate, opt-in protocol research layer until there is independently validated physical transport and security review.

## Domain model and algorithm contract

- Design an explicit versioned protocol, e.g. `OC-WATCH-AUTH-RESEARCH/1`, with purpose/domain separation distinguishing host request vs Watch response and research confirmation vs future privileged actions.
- Choose one cross-platform compatible asymmetric algorithm for the new prototype (prefer P-256 ECDSA + SHA-256, with CryptoKit on Apple and a vetted Rust p256 ECDSA implementation). Document required curve, SEC1 uncompressed 65-byte public keys, fixed-length 64-byte IEEE-P1363/raw signature representation (or a precisely specified alternate supported by both SDKs), SHA-256 input rules and parsing. Do not equate a SHA-256 digest with a signature or a MAC.
- Messages should be strictly bounded, canonically serialized with unambiguous length prefixes or a canonical encoding; reject duplicates, unknown critical fields, invalid UTF-8, oversized lengths, unsupported protocol/algorithm versions and noncanonical encodings. No key material from an untrusted message becomes an enrollment trust anchor.
- Host challenge: independent host key identity, bound Linux account identifier, specific action `research_confirmation` (never `unlock`), fresh 32-byte CSPRNG nonce, session ID, issued-at/expiry and optional Watch enrollment ID. The host must sign the transcript; Watch verifies it against a **separately provisioned pinned host public key before showing a consent prompt**.
- Watch response: signed canonical transcript binding the exact host request digest/nonce, host/account, watch enrollment ID, session, action, explicit approved/rejected decision, response time and authenticated-only-as-research local consent outcome. Host verifies with an **independently registered Watch public key**, with strict expiration/replay and revoked/inactive binding checks.
- A Watch UI tap or LocalAuthentication result is *local evidence asserted by the app*, not remotely attestable proof of a live wearer, secure distance or unmodified Watch software. The protocol must report these limits explicitly.
- Never generate/accept production trust from endpoint-supplied public keys or automatic BLE pairing. An explicit out-of-band trust ceremony and secure storage are separate future gates; for P008 allow in-memory test-only enrollment with clear fixture/CLI labeling. Secure Enclave/Keychain hardware availability on SE 2 remains UNVERIFIED.

## Workstream A — Rust portable signed research contracts
1. Inspect `crates/opencontinuity-core/src/watch_endpoint.rs` and existing P003/P004 invariants; implement new isolated `watch_auth_research` or equivalent, not a breaking replacement.
2. Build typed host identity, Watch identity, explicit independent enrollment/trust record with lifecycle/algorithm/version, signed request and signed response types, strict canonical transcript encoder and bounded verifier.
3. Require separately enrolled host key verification on Watch side and separately enrolled Watch key verification on host side. Fail closed for unknown, inactive, revoked, mismatched IDs/algorithms, replay, expiry, wrong action, key substitution and downgrade.
4. Replay guard must be explicit; document persistence gap and fail closed on storage loss for any runtime integration. In-memory guard is test-only; production replay-resistant store is NOT IMPLEMENTED.
5. Use OS CSPRNG for live synthetic nonces when feasible, deterministic fixed data only in fixtures; test-only keys never used in product UI or public unlock logic.
6. Expose a read-only/fixture-based CLI research command if useful: `opencontinuity research watch-auth --fixture <path> --json`, with no Bluetooth, system auth, key-store or privileged side effects and report `authorization: never_evaluated`. Never expose private keys or original transcript bytes in logs.

## Workstream B — Swift cryptographic codec and consent research UI
1. Add isolated Swift package module(s) and tests under `experiments/watchos/`; avoid breaking existing synthetic BLE app and its P007/P007A build. P-256 signatures must verify against the same Rust fixture messages and vice versa.
2. Use CryptoKit public APIs and `LocalAuthentication` for an explicitly user-initiated **research confirmation only** if SDK availability permits. Any Watch-side prompt must verify a separately pinned host signature first and clearly display `Research confirmation — no Linux unlock`.
3. Never automatically sign just because BLE connected or a frame arrived. No silent approval on timeout, app foreground transitions or missing LocalAuthentication; cancellation and denial are terminal failure outcomes.
4. Separate pure Swift protocol/canonical encoding/signature verification from platform-specific watchOS UI and key storage adapters. The cross-language conformance tests should run on hosted macOS; an unsigned watchOS SDK compile must run on CI. If no signed physical device, keep Watch-specific runtime claims NOT RUN.
5. Secure Enclave feasibility: research public `SecureEnclave.P256.Signing.PrivateKey` and watchOS LocalAuthentication policy availability, compile-gate supported SDK paths where possible, but never claim device-held secure keys or wearer verification until tested on real Watch SE 2. Do not export/enroll keys implicitly. Do not implement production persistent Keychain enrollment as a placeholder.

## Workstream C — Cross-language crypto fixtures and threat review
1. Stable versioned positive and adversarial vectors, with canonical bytes or redacted non-secret fixtures and independent Rust/Swift signatures/verifiers; confirm cross-signature verification using both languages. Signature bytes may be nondeterministic; tests should verify signatures rather than compare newly produced signatures byte-for-byte.
2. Negative tests: host-key injection/substitution, watch-key substitution, wrong host/account/session/action, reused challenge, expired/out-of-order timestamps, inactive/revoked enrollment, missing user approval, wrong signature/curve/encoding, future versions, malformed lengths, replay after disconnect, denial and downgrade, role-confusion/cross-protocol signatures.
3. Review trust-on-first-use vs authenticated out-of-band pinning, key custody, stolen/lost Watch, revocation, replay cache persistence, clock skew, concurrency, malicious peripheral, captured signatures, compromised host/Watch, relays and distance spoofing.
4. Keep auth result separate from Linux authorization; even valid signatures cannot decide whether to unlock. Authenticating a specific app key is not proof it is a genuine Apple Watch, that it is worn/unlocked, or that it is nearby.

## CI and documentation
- Preserve P001–P007 Rust and Swift suites and existing `.github/workflows/ci.yml`.
- Run Rust fmt, Clippy `-D warnings`, workspace tests/build and diff check, Swift package tests on macOS and unsigned watchOS SDK target compilation. Avoid cargo dependency/license surprises.
- Source evidence: local vs hosted; software crypto vs physical Secure Enclave/LocalAuthentication; physical BLE integration never claimed on P008 without actual trial.
- Produce `docs/research/P008_AUTH_PROTOCOL.md`, `docs/research/P008_ENROLLMENT_TRUST.md`, `docs/research/P008_WATCH_CONSENT.md`, `docs/research/P008_SECURITY_REVIEW.md`, and `docs/P008_VALIDATION.md`, plus minimum README/ROADMAP/ARCHITECTURE/THREAT_MODEL/FEATURE_MATRIX updates.
- If Apple-specific compile or source limitations arise, record exact blocker rather than fabricating results.

## Acceptance and workflow
- [ ] Real signed P-256 research request/response domain separated; Rust & Swift tests demonstrate signature verification with independent pinned keys.
- [ ] Host verified by Watch-side research verifier before consent UI, Watch key verified by Linux research verifier; no endpoint-supplied-key trust.
- [ ] Explicit manual consent only; no automatic approval, lock-screen controls or OS authorization.
- [ ] Robust parser/encoding, replay/expiry, downgrade and substitution rejection with negative tests.
- [ ] Security limitations explicit: app-key authenticity ≠ genuine Watch/wearer presence/secure proximity; key storage and BLE physical NOT VERIFIED.
- [ ] Existing P007 synthetic GATT v1, Apple SDK and CI unchanged unless narrowly required.
- [ ] Both platform software tests and actual Apple SDK compile are evaluated; physical Watch/BlueZ remains separately marked.
- [ ] PR against `main` using existing P008 branch. **No automatic merge, no P009.**

The full authoritative roadmap source remains missing. Keep P008 provisional. All auth research output uses `authorization: never_evaluated`. P007B remains unmerged in parallel, and must not be represented as a dependency satisfied.
