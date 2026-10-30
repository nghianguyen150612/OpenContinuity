# P009A temporary test enrollment

This workflow is explicitly test-only. It pins public P-256 keys using an operator-verified out-of-band comparison. A BLE address, advertisement, service UUID, pairing event or transferred JSON file is never sufficient identity evidence.

## Linux operator: create a host key and public pin

Run on the Linux test account that will operate BlueZ. Keep all files outside the repository:

```text
opencontinuity research watch-auth-gatt generate-host-key --host-key-file /secure/path/p009a-host-key --experimental
opencontinuity research watch-auth-gatt export-host-pin --host-enrollment-id lab-host-01 --host-identity linux-lab-01 --linux-uid 1000 --host-key-file /secure/path/p009a-host-key --experimental --json
```

Use the actual effective Linux UID in `--linux-uid`. The generator creates a new random P-256 key in a new mode-0600 file and refuses to overwrite an existing path. It prints only the public fingerprint. The export emits the public key, host identity, UID, active `test_only` enrollment fields, fingerprint and a one-hour expiry. Transfer this public JSON to the Watch operator. Independently communicate the host SHA-256 fingerprint through a manually verified channel, such as a supervised in-person comparison or a separately authenticated call.

## Watch operator: create the ephemeral Watch pin

1. Open **Authenticated Research** in the Watch app and choose **Create ephemeral Watch key**. CryptoKit creates a new P-256 signing key. The private key stays in app memory and is never included in export data.
2. Import the Linux host public-pin JSON. Check the displayed host identity, Linux UID, and fingerprint against the independently communicated values. Select the host-fingerprint confirmation only after they match.
3. The app creates a fresh Watch enrollment identity bound to the pinned host enrollment, host identity and Linux UID. Its public key and enrollment expire with the imported host pin, no later than one hour after export.
4. Share the displayed public enrollment JSON with the Linux operator. Confirm the Watch public-key fingerprint through the independent channel. Do not approve based only on the shared file or BLE discovery.

The Watch public enrollment has schema version 2 and contains only public keys and binding metadata. There is no private-key field. The app requires a second explicit confirmation that the Linux operator compared and pinned the Watch fingerprint before enabling **Start Authenticated Research**.

## Linux operator: verify both pins and start

Save the Watch's public enrollment JSON outside the repository. Check it against the generated host key:

```text
opencontinuity research watch-auth-gatt check-enrollment --enrollments /secure/path/p009a-enrollment.json --host-key-file /secure/path/p009a-host-key --experimental --json
```

This command rejects expired, incomplete, revoked/inactive, mismatched or incorrectly bound records, a host key that differs from the imported host pin, an invalid Watch P-256 key, and a Linux UID that differs from the effective UID. Compare both 64-character lowercase fingerprint values with the Watch operator over the separate channel. Then pass the independently checked values to `serve`:

```text
opencontinuity research watch-auth-gatt serve --adapter hci0 --duration-seconds 60 --enrollments /secure/path/p009a-enrollment.json --host-key-file /secure/path/p009a-host-key --confirm-host-sha256 <verified-host-sha256> --confirm-watch-sha256 <verified-watch-sha256> --experimental
```

Both confirmations are required and checked before D-Bus setup. `serve` signs its fresh P008 request only after BlueZ application and advertisement registration succeed. The Watch scan is explicitly user-started; the operator selects the discovered host, and the host identity still comes only from the verified P008 signature and pin.

## End and revoke

Completion, rejection, cancellation, failure, expiry, disconnect, app backgrounding and process exit terminate the current session. The Watch clears its private key and enrollment state from its active app objects. Start over by creating a new Watch key and a new Linux enrollment with new identifiers and fingerprints. Delete the Linux host key file and public enrollment/pin artifacts after the trial. Mark a record revoked or delete it to prevent future use; the CLI accepts only active records.

This workflow does not provide a production enrollment store, Watch identity attestation, Secure Enclave custody or guaranteed memory zeroization. Host files and public JSON are operator-managed test artifacts. Never reuse these keys for accounts, login, PAM, polkit or production authorization.
