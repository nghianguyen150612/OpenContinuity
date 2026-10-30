# P009 trust and consent integration

## Trust records

P009 reuses P008's independent `HostEnrollment` and `WatchEnrollment` records. The Watch trusts the host key from its separately supplied enrollment record; Linux trusts the Watch key from its independently supplied enrollment record. The transport's BLE peer address, advertised name, service discovery, pairing state, transfer ID and ACKs never create or alter an enrollment.

P009A adds a test-only user-initiated exchange. The Watch generates an ephemeral CryptoKit P-256 key in memory, imports a public host pin, and exports only public Watch key and binding metadata. Both sides require a manually verified fingerprint comparison. No production enrollment ceremony, persistent Watch key store or crash-safe replay store is added. Fixed P008 signing keys remain confined to test code and fixtures. No private signing key is embedded in the Watch target.

The Rust and Swift envelope types validate exact canonical P008 message encoding and detached signature length. `WatchAuthProtocol.verifyHostTransportEnvelope` turns a complete host request envelope into `VerifiedHostRequest` only after independent host-key signature, exact action, UID, host/Watch binding, request version, time and replay checks. The P008 verifier also rejects unknown, inactive, revoked and non-test enrollment records.

On Linux, the cross-language integration test reassembles the exact response frame bytes and verifies the detached Watch signature, request digest, nonce, session, host identity, Linux UID, target Watch enrollment, action, decision, consent result, expiration and replay state through P008. A verified signed rejection is reported as `rejected` and never promoted to approval.

## Consent boundary

`ResearchConsentView` already requires a `VerifiedHostRequest`, an explicit foreground button, and a supported `.deviceOwnerAuthentication` result before signing approval. It checks expiration again after LocalAuthentication returns. Manual rejection requires a separate explicit tap. Cancellation, unsupported policy, failed LocalAuthentication, request expiry, and view dismissal produce no signed approval. Its visible title is **Research confirmation — does not unlock Linux.**

The Watch CoreBluetooth manager reassembles the P009 request and calls `verifyHostTransportEnvelope` using the imported, independently verified host pin before it creates a consent presentation. It also validates that the selected Watch enrollment is active, bound to the verified host/account, and matches the ephemeral signing key before presenting the request. The dedicated Authenticated Research screen requires explicit confirmation of the host fingerprint and that the Linux operator pinned the Watch fingerprint. Pairing-based trust fallback does not exist. Software tests use explicitly marked test-only keys and a simulated rejection; they do not simulate LocalAuthentication. Physical signed-approval behavior remains unverified.

## Response validation

The Watch response is signed over P008's role-separated response transcript, including the exact SHA-256 digest of the verified canonical host request. Linux must accept it only for the outstanding owner and transfer ID, then verify the P008 transcript against its pinned Watch key. An ACK proves only transport progress. It is never an approval, denial, identity assertion or authorization result.

Every result must carry `authorization: never_evaluated`. No response is connected to PAM, polkit, a login manager, desktop unlock, account state, or another privilege decision.

## Temporary test enrollment

The operator exchange and cleanup steps are documented in [P009A temporary enrollment](P009A_TEMPORARY_ENROLLMENT.md). The host key is generated into a new mode-0600 Linux file; only its public pin leaves Linux. The Watch key is ephemeral and stays in Watch app memory. The Watch public pin is shared to Linux. Both exact SHA-256 fingerprints must be compared through a separate manually verified channel, and the Linux `serve` command requires both values before BlueZ registration. This is test-only and does not claim permanent identity, hardware attestation, Secure Enclave custody or guaranteed key zeroization.
