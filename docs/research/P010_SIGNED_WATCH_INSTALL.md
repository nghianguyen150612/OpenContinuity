# P010 development installation on Watch SE 2

`authorization: never_evaluated`

Follow [P007B installation](P007B_APPLE_SIGNED_INSTALL.md) and
[P009A temporary enrollment](P009A_TEMPORARY_ENROLLMENT.md). No physical installation
was executed in this cloud workspace.

Use macOS supported by your Xcode release and Xcode with device support for the
Watch's installed watchOS. The project requires watchOS >=10.0, Swift tools >=5.9
and a watchOS SDK; prior baseline qualified Xcode 16.4/watchOS 11.5. Current CI uses
macos-15; its inventory records the selected actual Xcode/SDK. This is not a promise
that every newer Watch OS works with that SDK. Record versions on the installation
Mac. Use XcodeGen (baseline 2.46.0) to generate:

```sh
xcodegen generate --spec experiments/watchos/project.yml
xcodebuild -project experiments/watchos/OpenContinuityWatch.xcodeproj -list
xcodebuild -project experiments/watchos/OpenContinuityWatch.xcodeproj -scheme OpenContinuityWatch -showdestinations
```

Keep destination output private (device identifiers). In Xcode, select scheme
OpenContinuityWatch and the paired physical Watch destination. Prepare/trust the
paired iPhone and Watch in Devices and Simulators; enable Developer Mode when
requested. Choose your development team, a unique bundle ID and valid development
provisioning in Signing & Capabilities. Override CODE_SIGNING_ALLOWED=NO to YES
locally; the generated default intentionally permits unsigned CI. Do not commit
team IDs, certificates, profiles or Apple credentials. Use Xcode Run to build,
install and launch. A CLI `build` alone does not install.

Missing target: regenerate from repository root, inspect `-list`, use the generated
project and scheme, and ensure the watchOS SDK is installed. Missing destination:
check pairing, trust, Developer Mode, device preparation and installed OS support.
Signing failure: check the local YES override, unique bundle ID, selected authorized
team and provisioning eligibility/expiry. Classify build, sign, install and launch
separately; never fix signing by publishing credentials.

Keep the app foreground. Accept its Bluetooth permission prompt; if denied, use
normal device privacy settings manually. Start Scan and Authenticated Research are
explicit actions. Linux pairing is not required. Review verified host request
before choosing reject or approve. Approval needs supported successful local
watchOS authentication; report unsupported policy as BLOCKED.

Apple CI uploads `OpenContinuityWatch-UNSIGNED-development-local-signing-required`
with a seven-day retention. The ZIP contains an unsigned Debug watchOS app for
inspection. It requires appropriate local signing/provisioning; regenerate and
build from source with Xcode for reproducible device installation. No CI artifact
establishes installation or hardware compatibility.

After testing, Disconnect, Clear Authenticated Research, disable diagnostics, stop
the bounded server and remove the Watch app if no longer needed. Delete Linux
mode-0600 temporary host key and public enrollment/pin files outside the repo;
revocation/deletion blocks subsequent enrollment use. Do not reuse research keys.
Clearing app references does not claim guaranteed zeroization or Secure Enclave
custody. Retain only reviewed sanitized evidence.
