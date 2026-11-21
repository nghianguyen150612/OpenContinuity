# P019 RTL8852BE and iPhone 13 physical qualification

This is a manual, opt-in trial plan. The cloud environment has no authorized
RTL8852BE radio or iPhone 13; every physical step here is currently NOT RUN.
The previous observation of AP channel 153 is historical only. Re-run the
read-only doctor and inspect the current network before any trial.

## Read-only preflight

On the target Arch host, record the OpenContinuity version/commit, kernel,
current regulatory domain, PCI vendor/device ID, loaded Wi-Fi driver, interface
count, current AP association/channel, monitor-mode capability, current
`awdl0` state/link-local address, Owly version and `owly doctor` result:

```sh
opencontinuity airdrop doctor --json
iw dev
iw reg get
```

These commands are intended to read state. Review the output locally before
sharing because the doctor intentionally reports PCI ID, driver, kernel and
channel. Do not publish interface names, BSSID, SSID, hostnames, serial numbers
or raw logs. The P019 Settings status refresh runs only the sanitized local
doctor. It performs no scan or radio transition.

## Hardware-specific considerations

Owly's inspected compatibility table reports RTL8852BE Shared-mode success
for one named device on CachyOS with kernel 7.2.9. That is not this laptop's
configuration and does not establish Arch support here. Its README describes
Shared mode as dependent on an AP channel such as 6, 44 or 149; a current
channel of 153 may block that mode. Never change the router or channel
automatically. Any change requires a separately authorized operator decision.
Exclusive/Active-monitor operation can disconnect Wi-Fi. No zero-disruption
claim is made.

## Explicit physical procedure

1. Confirm the iPhone 13 owner consents, both devices are yours or authorized,
   and the receiver destination is a new or reviewed directory.
2. Install Owly independently from its verified upstream packaging and review
   its current radio/setup documentation, service, license and privilege
   requirements. OpenContinuity does not install it or `owlyd`.
3. Run read-only preflight again. If `owly doctor` reports missing capability,
   unsupported channel/driver, or unknown state, stop and mark BLOCKED.
4. If radio setup is needed, follow Owly's documented Shared/Exclusive choice
   in a separate operator-controlled session. Understand its exact effects
   before authorizing any command such as `owly radio up`; OpenContinuity never
   runs it. Do not edit NetworkManager, firewall, regulatory state or router
   settings automatically.
5. Verify manually that Owly has already produced an active `awdl0` with a
   link-local address and that `opencontinuity airdrop doctor --json` reports
   the expected local readiness. `advertisement` may remain unknown.
6. In a terminal, start a bounded receive with explicit flags and a reviewed
   destination:

   ```sh
   opencontinuity airdrop receive --backend owly \
     --destination "$HOME/Downloads/AirDrop-P019" \
     --duration-seconds 300 --radio-prepared --consent
   ```

   OpenContinuity requires the directory to exist, terminal stdin/stderr,
   `XDG_RUNTIME_DIR`, and a separate Owly approval for each incoming request.
   Never add Owly's `--accept` option.
7. On iPhone, set AirDrop visibility to Everyone for this controlled test and
   send one non-sensitive test file. Observe manually whether Linux appears,
   the exact consent shown, the transfer result, received bytes, permissions,
   duplicate handling and Wi-Fi after teardown. Do not treat a visible name or
   a UI success message alone as independent RF evidence.
8. Cancel once with Ctrl-C and verify child termination, temporary state
   cleanup and no partial publication. Separately test natural expiry and a
   normal completion. Review all output paths and ensure the Wi-Fi connection
   returns as expected. Stop if the radio or network behaves unexpectedly.
9. Record host software traces, Watch/phone observations if available,
   operator statements and independent RF captures as distinct evidence
   sources. Do not put raw addresses, BSSID/SSID, names, payloads, private keys
   or packet captures into public artifacts. Independent RF collection needs
   its own authorized setup and privacy review.

## Acceptance boundary

A physical receive is only a candidate observation if the exact host/iPhone
models, OS versions, driver/kernel, regulatory region, radio mode/channel,
start/cancel/cleanup outcomes and destination file checks are recorded and
owner-reviewed. Independent RF/protocol evidence must separately corroborate
the path before any physical interoperability acceptance. CI fixtures,
`receiver_available`, Owly status, or an operator-only JSON record never
promote this gate automatically. No result in this repository currently
establishes an iPhone 13 ↔ Arch transfer or RTL8852BE compatibility.
