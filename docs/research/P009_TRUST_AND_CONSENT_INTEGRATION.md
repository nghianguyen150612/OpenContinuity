# P009 trust and consent integration

## Trust records

P009 reuses P008's independent `HostEnrollment` and `WatchEnrollment` records. The Watch trusts the host key from its separately supplied enrollment record; Linux trusts the Watch key from its independently supplied enrollment record. The transport's BLE peer address, advertised name, service discovery, pairing state, transfer ID and ACKs never create or alter an enrollment.

The repository still has test-only in-memory enrollment records. There is no production enrollment ceremony, safe provisioning UI, persistent key store or crash-safe replay store. Fixed P008 test signing keys are confined to test code and fixtures. No private signing key is added to the shipped Watch target.

The Rust and Swift envelope types validate exact canonical P008 message encoding and detached signature length. `WatchAuthProtocol.verifyHostTransportEnvelope` turns a complete host request envelope into `VerifiedHostRequest` only after independent host-key signature, exact action, UID, host/Watch binding, request version, time and replay checks. The P008 verifier also rejects unknown, inactive, revoked and non-test enrollment records.

On Linux, the cross-language integration test reassembles the exact response frame bytes and verifies the detached Watch signature, request digest, nonce, session, host identity, Linux UID, target Watch enrollment, action, decision, consent result, expiration and replay state through P008. A verified signed rejection is reported as `rejected` and never promoted to approval.

## Consent boundary

`ResearchConsentView` already requires a `VerifiedHostRequest`, an explicit foreground button, and a supported `.deviceOwnerAuthentication` result before signing approval. It checks expiration again after LocalAuthentication returns. Manual rejection requires a separate explicit tap. Cancellation, unsupported policy, failed LocalAuthentication, request expiry, and view dismissal produce no signed approval. Its visible title is **Research confirmation — does not unlock Linux.**

The Watch CoreBluetooth manager now reassembles the P009 request and calls `verifyHostTransportEnvelope` using an injected, independent host enrollment before it creates a consent presentation. It also validates that the selected Watch enrollment is active, bound to the verified host/account, and matches the injected signing key before presenting the request. `ContentView` presents `ResearchConsentView` only for that verified and response-eligible session. The manager accepts only matching test-only enrollment records and an in-memory Watch signing key that matches the Watch pin. No key-loading or pairing-based trust fallback exists, and the current app has no approved provisioning UI or caller to supply those values. Therefore the authenticated scan and physical signed-approval flow remain **BLOCKED** in the shipped app. Software tests use explicitly marked test-only keys and a simulated rejection; they do not simulate LocalAuthentication.

## Response validation

The Watch response is signed over P008's role-separated response transcript, including the exact SHA-256 digest of the verified canonical host request. Linux must accept it only for the outstanding owner and transfer ID, then verify the P008 transcript against its pinned Watch key. An ACK proves only transport progress. It is never an approval, denial, identity assertion or authorization result.

Every result must carry `authorization: never_evaluated`. No response is connected to PAM, polkit, a login manager, desktop unlock, account state, or another privilege decision.

## Prospective physical research provisioning

Before a device trial, an operator must establish a temporary research enrollment out of band:

1. Generate separate host and Watch P-256 key pairs on the test devices; do not use a BLE advertisement or pairing result as a source of trust.
2. Exchange only the public keys and enrollment/binding fields through an authenticated, independently verified channel. Confirm the host UID and target Watch enrollment on both sides.
3. Keep signing private keys in process memory for the test. Do not write test secrets into the repository, a fixture, command-line arguments, logs, or the Watch application bundle. The current Linux CLI reads the host key from a separate mode-0600 file before loading it into memory; this convenience path is not a reviewed physical key ceremony, and that temporary file must be deleted immediately after any controlled software trial.
4. Ensure both devices label the enrollment `test_only`, have a bounded request lifetime, and start with empty in-memory replay guards.
5. Revoke/delete the temporary material after the test and record that replay protection does not survive process restart.

No such provisioning flow currently exists. The CLI and Watch manager expose bounded test-only injection points for research harnesses, but no current workflow safely distributes independent pins and temporary private keys between them. Until that setup is separately reviewed and implemented, no live signed approval should be attempted.
