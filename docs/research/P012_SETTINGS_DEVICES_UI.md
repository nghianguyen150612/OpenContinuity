# P012 Settings Devices UI

The Devices page exposes one explicit **Refresh known devices** action. It starts only `opencontinuity devices list --json`; no startup request or recurring timer exists. The page labels the source **BlueZ known devices — no Bluetooth scan performed.**

The page distinguishes an unrefreshed/loading snapshot, successful empty snapshot, known devices, unavailable adapter/BlueZ/system bus, denied access, timeout, malformed data, missing CLI and stale last-valid rows. It shows adapter availability, the number of known Device1 objects, the last refresh time, and BlueZ's reported Connected, Paired and Trusted values. Missing values render as unknown. Trusted is explicitly scoped to local BlueZ state.

Names use plain QML text after strict schema validation and Rust sanitization. No actions for connect, pair, trust, forget, AirDrop, AirPods configuration, or Watch unlock are present. Apple capability pages continue to use DoctorReport v1 and remain unsupported. The QML smoke test loads the actual window, navigates to Devices, refreshes a fixture snapshot and inspects model and labels.
