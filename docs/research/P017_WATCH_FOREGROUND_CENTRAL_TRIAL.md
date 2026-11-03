# P017 Apple Watch SE 2 foreground central trial

Current outcome: physical W0–W6 NOT RUN/BLOCKED. No Watch, paired installation
path, Xcode signing team or real Linux adapter is available here. Hosted unsigned
compilation is software evidence only. P010 Watch physical acceptance and native
Apple Auto Unlock remain NO-GO. `authorization: never_evaluated`.

## Existing compatibility matrix

Topology: **Linux BlueZ GATT peripheral/server ↔ Apple Watch CoreBluetooth
central/client**. Apple documents CBPeripheralManager service advertising as
unavailable on watchOS. Never use a Watch-as-advertising-peripheral plan.

| Capability | Linux implementation | Watch implementation | Software / physical boundary |
| --- | --- | --- | --- |
| Temporary advertisement | `gatt.rs` registers owned GATT app and LE advertisement | `WatchBluetoothManager` filters an explicit foreground scan | Registration and RF need separate hardware evidence |
| P006 synthetic service | Existing `watch-gatt serve`, five characteristics | `GattIdentifiers`, `GattWireCodec`, acknowledged writes | Existing wire format; real long read remains unqualified |
| Service/characteristic discovery | Existing public experimental UUIDs | `discoverServices`, exact characteristic/property validation | No second protocol or private Apple UUID |
| P009 research transport | Existing `watch-auth-gatt serve` and P008 signatures | `WatchAuthTransport`, GET/read/ACK and sequential response writes | Separate existing P009 service, not a change to P006 |
| Manual test enrollment | New 0600 test host key; host pin export, enrollment check, two fingerprint flags | Ephemeral CryptoKit key, host pin import, two explicit confirmations | Public pins independently compared; no production enrollment |
| Foreground consent | Verified response only; no unlock | Verified request → ResearchConsentView → explicit action and LocalAuthentication | Hardware LA outcome is independent of signature correctness |
| Cancel/timeout/cleanup | Bounded runtime, signal cleanup and radio guard | Scan/connection/phase deadlines, cancel/disconnect, generation checks, background cleanup | Physical callback/resource evidence still NOT RUN |
| Installation | Not applicable | XcodeGen project, watchOS 10+, Bluetooth usage description, signing disabled by default | CI artifact cannot be installed as a signed device app |

Source review found the required foreground, cancel, generation, enrollment and
wire behavior already implemented. P017 reuses these sources unchanged. Existing
Swift/CryptoKit/interop and unsigned build jobs remain mandatory regressions.
See [Watch README](../../experiments/watchos/README.md),
[P006 contract](P006_WATCH_CLIENT_CONTRACT.md),
[P009 transport](P009_AUTH_TRANSPORT_DESIGN.md) and
[temporary enrollment](P009A_TEMPORARY_ENROLLMENT.md).

## W0: signing and actual installation

Use an owned Watch SE 2, its Apple-supported paired iPhone installation path and
a Mac/Xcode/watchOS SDK compatible with that Watch. Record model/OS/tool versions
privately without serials, account or signing-team IDs. Generate the project:

```sh
xcodegen generate --spec experiments/watchos/project.yml
xcodebuild -project experiments/watchos/OpenContinuityWatch.xcodeproj \
  -scheme OpenContinuityWatch -configuration Debug \
  -destination 'generic/platform=watchOS' CODE_SIGNING_ALLOWED=NO build
```

This command tests unsigned compilation only. To install, open the generated
project in Xcode, configure the operator's development team and provisioning and
override the generated `CODE_SIGNING_ALLOWED=NO` to enable signing for the real
paired Watch destination. Use the supported Xcode device workflow described in
[P007B installation](P007B_APPLE_SIGNED_INSTALL.md). Record actual installed launch
separately. Missing signing/device support blocks W0 and dependent Watch stages.
Do not alter repository signing defaults or publish credentials.

## W1–W3: explicit P006 trial

1. Finish all discovery trials and ensure their session has exited. The adapter
   guard excludes discovery and GATT on the same adapter. Do not bypass it.
2. Run read-only capability/preflight commands from the existing
   [P007B runbook](P007B_REAL_BLE_RUNBOOK.md). Privately inspect selected actual
   hciN, firmware, Powered, LE peripheral support, GattManager1,
   LEAdvertisingManager1 and free advertisement capacity. Introspection does not
   establish registration permission. Missing adapter/policy/slots is BLOCKED;
   do not power, pair, trust, restart services or patch policy.
3. Obtain explicit consent from Linux and Watch operators for the temporary
   advertisement, connection and synthetic payload. Start the bounded existing
   server (substitute the actual selected hciN):
   `opencontinuity research watch-gatt serve --adapter hci0 --duration-seconds 120 --experimental --json`.
   Keep terminal logs private. Record app registration, advertisement registration
   and independent receiver RF observations separately as W1.
4. Keep the installed Watch app foreground. Verify Bluetooth readiness and no
   pre-action scan. Enable bounded metadata-only diagnostics explicitly if
   desired. Press Scan, select the controlled Linux host and connect (W2).
   Scan is bounded to 15 seconds, connection to 15 seconds, discovery to 12
   seconds; operation callbacks to 8 seconds. Do not infer background reliability.
5. Verify exact existing service and five characteristics, version 1, complete
   25-byte challenge, sequential writes of at most 20 bytes including the
   15-byte header, acknowledgements, Complete and complete 44-byte success with
   matching SHA-256 (W3). An 11-byte error or truncated long read is failure,
   not success. Record lengths/counts/verdict only in public summaries.
6. Repeat cancel, disconnect mid-transfer, reconnect with fresh challenge, scan
   timeout, app backgrounding, server expiry and SIGINT/SIGTERM as separate W6
   trials. Verify terminal connection callbacks, owned advertisement/application
   removal and independent RF cessation. Never assume process exit proves it.

## W4–W5: signed test-only P009 trial

Stop the P006 server and verify its cleanup first. Follow the exact commands and
public-pin schema in [P009A temporary enrollment](P009A_TEMPORARY_ENROLLMENT.md):

```text
opencontinuity research watch-auth-gatt generate-host-key --host-key-file /private/test/host-key --experimental
opencontinuity research watch-auth-gatt export-host-pin --host-enrollment-id lab-host-01 --host-identity linux-lab-01 --linux-uid <actual-effective-uid> --host-key-file /private/test/host-key --experimental --json
opencontinuity research watch-auth-gatt check-enrollment --enrollments /private/test/watch-public.json --host-key-file /private/test/host-key --experimental --json
opencontinuity research watch-auth-gatt serve --adapter <actual-hciN> --duration-seconds 60 --enrollments /private/test/watch-public.json --host-key-file /private/test/host-key --confirm-host-sha256 <verified-64-lowercase-hex> --confirm-watch-sha256 <verified-64-lowercase-hex> --experimental
```

Between export and check-enrollment, create the ephemeral Watch key in
Authenticated Research, import the host public pin, independently compare the
host fingerprint/identity/effective UID, export only the Watch public enrollment,
and independently compare its fingerprint on Linux. Confirm both comparisons
explicitly on the Watch before Start Authenticated Research. A transferred JSON
file or BLE advertisement is not an independent verification channel. Keep keys,
public bindings and fingerprints outside the repository/public evidence.

Perform a signed rejection first (W4), then a separate fresh enrollment/trial
for explicit signed approval (W5). Observe request GET/read/ACK, verified pinned
host request before consent, explicit foreground decision, actual successful
LocalAuthentication for approval, sequential acknowledged response writes and
Linux signature/verdict verification. A response-Complete callback alone does
not prove the Linux verdict. Request delivery is bounded to 20 seconds, consent
to 25 seconds, response transfer to 15 seconds and inactivity to 8 seconds, all
clamped by signed expiry. Unsupported/denied LA blocks/fails approval; it cannot
be relabeled from CI's simulated rejection.

Exercise wrong pin, expired/revoked binding, cancellation, missing ACK, disconnect,
suspension, timeout and stale reconnect callbacks as separate negative records.
After each terminal path verify cleanup and fresh setup; delete the test host key
and public enrollment artifacts after the trial. There is no production identity,
secure proximity, Apple Account, native Auto Unlock, PAM/polkit or Linux login
permission implied by any P006/P009 result. The Watch app does not advertise.
