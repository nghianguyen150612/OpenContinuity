# P008 foreground Watch consent research

The P008 consent component is a separate SwiftUI view for an already verified
host request. A parent app can present it only after a user starts a test and
`WatchAuthProtocol.verifyHostRequest` succeeds against an independently pinned
host record. It is not connected to the P006/P007 synthetic GATT v1 message
flow; a future transport must deliver and verify the P008 request before
presenting the view.

## LocalAuthentication policy research

Apple's public [`LAPolicy.deviceOwnerAuthentication`](https://developer.apple.com/documentation/localauthentication/lapolicy/deviceownerauthentication)
documentation describes user authentication with biometry, Apple Watch or the
device passcode, and lists availability on watchOS starting at 3.0. The public
[`LAContext.canEvaluatePolicy`](<https://developer.apple.com/documentation/localauthentication/lacontext/canevaluatepolicy(_:error:)>)
and [`LAContext.evaluatePolicy`](<https://developer.apple.com/documentation/localauthentication/lacontext/evaluatepolicy(_:localizedreason:reply:)>)
APIs are also listed for watchOS. The app uses the broad
`.deviceOwnerAuthentication` policy and checks `canEvaluatePolicy` at runtime.
It does not assume that the policy is supported or that a specific biometric
is available on every Watch. The biometrics-only policy is not used.

## Consent behavior

- The screen displays **Research confirmation — does not unlock Linux.** and
  shows the verified host identity, UID, exact action and expiration.
- The view has no automatic or background approval. It requires an explicit
  button tap for either decision.
- Approval invokes `LAContext` only after the explicit approve tap, and signs
  only after LocalAuthentication succeeds and the request remains unexpired.
- A manual rejection is signed as `rejected` after its explicit button tap.
- Cancellation, unsupported policy, denial, failure, or expiration returns a
  local error and produces no response signature.
- The Watch signing key is provided in memory by a test harness and must match
  the independently registered Watch public key. The view does not store it.

An app-reported LocalAuthentication result is local evidence asserted by the
app. It is not remotely attestable proof of the genuine wearer, an authentic
Apple Watch, secure distance, or uncompromised software.

The LocalAuthentication implementation is source-only until the hosted
watchOS SDK build completes. Its behavior on Watch SE 2, Secure Enclave signing,
physical Watch installation, and biometric/passcode prompts are **NOT RUN**.
`SecureEnclave.P256.Signing.PrivateKey` is not used. Hardware-backed key
custody on Watch SE 2 remains **UNVERIFIED**.
