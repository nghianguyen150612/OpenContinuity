# P016 Qt stream lifecycle qualification

The actual DeviceDiscoveryProvider, DeviceDiscoveryReport and
DeviceDiscoveryModel are exercised by QtTest and the real QML window. A native
fixture validates the exact fixed executable arguments before producing output.
No automatic scan or reconnect is introduced. Qt 6.4.2 remains the minimum.

Added cases send 200 observations/updates, assert all 199 model updates before
terminal completion, cancel with retained stale rows, cancel before process
start, force shutdown of a stalled child, reject a crashed child and a partial
terminal after live rows, reject incorrect terminal event counts, and deliver
retired QProcess signals during a new generation. Existing split UTF-8,
fragmentation/coalescing, escaped duplicate keys, schema/session/sequence errors,
missing terminals, oversized output, diagnostics and deadline tests remain.
Retired signals are explicitly injected before deferred deletion; the production
retire disconnect and generation fence prevent mutation of the new session.

QML smoke rows verify 200-event live updates before terminal, a visible cleanup
warning and stale rows for partial terminal, Cancel and navigation away after
queued observations, no rescan on return, and actual window close with a stalled
child. The smoke fixture uses a 200 ms cancellation grace to bound test duration;
production retains 6500 ms. A stalled fixture simulates a child that cannot
finish output or cleanup; QProcess normally drains child pipes itself.

Offscreen QML proves software interaction only. Native Wayland/Hyprland,
physical controller cleanup and RF behavior are separate NOT RUN gates.
