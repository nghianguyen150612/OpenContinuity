# P003 hardware validation plan

**Execution status: NOT RUN.** No Apple Watch SE 2, paired iPhone, macOS, Xcode, or Linux Bluetooth controller was attached to this environment. There is no Apple companion target, and the mock CLI is not hardware evidence. Record device versions from Settings at the time of any future run; never fill them from expectation.

## Test equipment record

Before each run, enter actual values and attach sanitized logs outside the repository:

| Item | Required observation | Recorded for P003 |
| --- | --- | --- |
| Watch | Apple Watch SE 2 case size / GPS or Cellular / model identifier / watchOS build | **NOT RUN** |
| Paired phone | iPhone model / iOS build / WatchConnectivity session state | **NOT RUN** |
| Linux host | Distribution/kernel / logged-in user/session type / host uptime | **NOT RUN** |
| Linux Bluetooth | Adapter/controller vendor, model, firmware, HCI version; `bluetoothctl --version`; BlueZ daemon version | **NOT RUN** |
| Network | Wi-Fi AP/security, host LAN address type, VLAN/client isolation, firewall route; redact SSID/IP in shared report | **NOT RUN** |
| Apple build | Mac model/macOS build/Xcode version/watchOS SDK/deployment target/signing setup | **NOT RUN** |
| Measurement | UTC start/end; monotonic challenge-to-response; sample count; timeouts; delivery path | **NOT RUN** |

Use a dedicated test Linux account and reversible test service/port. Any hardware app remains opt-in and outside production authentication. Never run a PAM, root helper, login-manager or automatic session-unlock test.

## Reproducible setup

1. Build an isolated public-API watchOS app (future work) whose only radio role is `CBCentralManager`; have a Linux test utility expose a nonprivileged mock GATT peripheral. Add only required public usage strings and WatchKit background modes. Do not use private entitlements, Apple Account credentials, extracted keys, or Auto Unlock pairing records.
2. For relay tests, build a paired iOS companion using `WCSession`; Linux endpoint remains a separate unprivileged mock verifier. Do not let phone code sign approvals or hold the Watch private key.
3. Enroll a test-only Watch public key through an explicit mock ceremony. Log only pseudonymous IDs, timing and outcomes. Do not log private keys, raw advertisements, addresses or account names.
4. Synchronize wall clocks for display only; use monotonic timers for response deadlines. Use fresh 32-byte CSPRNG challenges in any future Apple test app, not the deterministic Rust mock source.
5. Repeat each condition at least 20 times, report every timeout/failure, and do not remove failures as outliers. First test the foreground baseline, then vary exactly one condition.

## Conditions to test independently

| Scenario | Procedure | Expected safe behavior | Result |
| --- | --- | --- | --- |
| Watch app foreground | Open app; initiate one test request; tap Approve and Deny in separate runs. | Fresh signed response only after a tap and successful system policy; deny action yields no approval. | **NOT RUN** |
| Watch app background/suspended | Complete foreground connection, lower wrist, leave app, then request from Linux with app foreground/background/suspended. | An unavailable endpoint times out and fails closed; no promise of immediate wake. Record CoreBluetooth reconnect/restoration behavior only. | **NOT RUN** |
| Watch screen off | Keep supported test session only if its use matches Apple's intended session type; turn screen off and issue a request. | Observe documented session limit; after expiration, fail closed. Do not manufacture an unrelated runtime session to simulate a daemon. | **NOT RUN** |
| Watch locked | Lock Watch / do not enter passcode, then attempt policy and test request. | Wrist/passcode policy failure means no signature/approval. | **NOT RUN** |
| Watch removed from wrist | Wear, enter passcode, remove Watch, then test immediately and after auto-lock transitions. | Require policy to fail after detected removal. Record exact delay and API result; never assume immediate removal detection. | **NOT RUN** |
| iPhone nearby and app reachable | Run `WCSession.sendMessage` with active session and log reachability/error/round-trip times. | Relay forwards opaque messages; Linux accepts only Watch signature. Reachable does not mean presence. | **NOT RUN** |
| iPhone nearby but Watch extension not reachable | Suspend/terminate Watch app, then try phone-originated request. | Expect immediate send to fail or remain unavailable; no silent iPhone approval fallback. | **NOT RUN** |
| iPhone disconnected/out of range | Disable phone Bluetooth or power off phone while Watch is on known Wi-Fi (and separately no Wi-Fi). | Direct Watch networking may route differently; if transport unavailable, bounded timeout and deny. | **NOT RUN** |
| Linux host disconnected | Stop mock endpoint, disable Linux BLE adapter, and separately disconnect LAN. | Watch UI reports host unavailable; no stale approval persists or queues for later unlock. | **NOT RUN** |
| Watch → Linux direct BLE | Linux advertises test GATT peripheral; Watch foreground central scans/connects/writes a challenge and reads response. | Confirm only direct foreground role. Pairing/link does not count as auth. No RSSI threshold may accept approval. | **NOT RUN** |
| Watch → Linux direct LAN | Host serves a TLS test endpoint on private LAN; test URLSession with local IP and any supported discovery route. Repeat with iPhone proxy, Wi-Fi and Cellular independently. | Record actual route; reject wrong TLS/key identity. If local host unsupported or ambiguous, mark path blocked/unknown. | **NOT RUN** |
| Watch → iPhone → Linux | Watch initiates a signed challenge response through `WCSession`, iPhone forwards it by test BLE and separately LAN. | Phone edits/forges a message in a controlled negative run; signature verification must reject it. Delayed or queued messages must expire. | **NOT RUN** |
| Timeout/delay/failure recovery | Inject delays of 1, 5, 10, 30, 60 sec; disconnect at each transport step; restart relay. | Expired challenge denied; no retry extends expiry; user must initiate a new challenge. | **NOT RUN** |
| Replay/downgrade/malformed | Replay valid captured test response; alter host/account/session/action/version/length/signature; duplicate nonce; unknown identity. | All negative cases rejected and do not authorize. Use synthetic non-sensitive test messages only. | **NOT RUN** |

## Evidence handling

- Run only on owned or explicitly authorized devices. Isolate other people's Bluetooth devices where practical.
- Record API callbacks, state transitions, timeout measurements, app lifecycle and errors; preserve the exact OS/build versions.
- A simulator can validate UI or API compilation but not Bluetooth RF, Watch suspension, wrist detection, Secure Enclave availability, WatchConnectivity timing, or SE 2 device behavior. Label simulator results separately.
- Keep raw HCI/RF/network captures outside the repository with restrictive permissions; redact addresses, SSIDs, Apple Account data, keys and bystander device metadata before any report.
- A successful direct BLE test proves only that tested OS/build/hardware combination can exchange mock GATT messages under that condition. It does not demonstrate Apple Auto Unlock, secure proximity or safe OS unlock.

## Lab environment conclusion

At P003 implementation time: Linux environment only; `xcodebuild`, `bluetoothctl`, and `btmgmt` unavailable. Physical Watch SE 2/iPhone/BlueZ test **NOT RUN**. Apple SDK/watchOS build **BLOCKED** on macOS/Xcode access. Local Rust mock tests do not satisfy hardware acceptance.
