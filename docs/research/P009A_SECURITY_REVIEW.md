# P009A security review

## Security properties implemented

- The P008 host request is created only after BlueZ has registered the dedicated GATT application and advertisement. The 30-second bounded setup phase is unsigned. Failed or expired setup emits no replacement or fallback request.
- The signed P008 request has an absolute maximum lifetime of 60 seconds and expires sooner if the test enrollment does. Test enrollment validity is capped at one hour. Linux and Watch retain monotonic deadlines for local phases and continue to check the signed wall-clock expiry. No phase timer refreshes P008 freshness.
- Request delivery, consent, and response transfer have separate bounded deadlines. The consent timer ends when the first valid response frame is accepted; the response then gets its own 15-second transfer interval, clamped to signed expiry.
- Linux test host public pins are derived from the host signing key. The Watch imports only public host data and independently confirms its SHA-256 fingerprint. Linux requires explicit command-line confirmation of both host and Watch fingerprints before D-Bus setup.
- The Watch generates an ephemeral CryptoKit P-256 signing key in memory. Only its SEC1 public key is exported. The Watch enrollment binds its identity to the host enrollment, host identity and Linux UID.
- Both CLI and Watch reject non-test, inactive/revoked, expired, malformed, unsupported, mismatched and incompletely bound enrollments. The Linux host key must be owned by the effective UID and stored in a regular mode-0600 file; generation refuses overwrite.
- `ResearchConsentView` is presented only after host signature, P008 binding, freshness, replay and Watch-key eligibility checks. Approval needs an explicit foreground choice and successful LocalAuthentication. Rejection is a separate signed decision. Failed authentication, cancellation, expiry and backgrounding do not create an approval.
- The P007 synthetic GATT v1 service and bytes remain separate and unchanged.

## Trust boundaries and limitations

- Manual fingerprint comparison is a human ceremony. The CLI flags and Watch checkboxes record the operator's explicit confirmation, but neither can prove that the operator used an independent channel.
- The JSON transfer is unauthenticated and may be copied or modified. A key is trusted only after its displayed fingerprint matches through the separately verified channel. BLE observations never establish identity.
- The Watch private key is held by CryptoKit in process memory and cleared from application state at terminal cleanup. No Secure Enclave storage, physical hardware attestation, secure memory erasure or persistence guarantee is claimed.
- The Linux host private key is a temporary test scalar file protected by mode 0600, not a production key store. The operator must delete it after the research run.
- Replay state is in memory only. Restart creates a new signed nonce/session, but replay history does not persist across process restarts.
- A valid test signature and LocalAuthentication result do not prove genuine Watch hardware, proximity, wearer presence, or authorization. Every result retains `authorization: never_evaluated`.
- Physical BlueZ/CoreBluetooth callbacks, actual fingerprint exchange, Watch background behavior and LocalAuthentication remain hardware validation items.

## Explicitly excluded

P009A adds no PAM or polkit authorization, Linux session unlock, login-manager changes, background auto-approval, automatic BLE-key trust, Apple-private STS, Apple Account credential access, production enrollment store, embedded private key, Secure Enclave claim or genuine Watch identity claim. No P010 work is included.
