import SwiftUI

struct ContentView: View {
    @ObservedObject var bluetooth: WatchBluetoothManager

    var body: some View {
        ScrollView {
            VStack(alignment: .leading, spacing: 8) {
                Text("OpenContinuity").font(.headline)
                Text("Experimental BLE transport").font(.caption2).foregroundStyle(.secondary)
                Label(bluetooth.bluetoothState, systemImage: "dot.radiowaves.left.and.right")
                    .font(.caption)
                Text("Stage: \(bluetooth.state.title)").font(.caption).bold()
                Text("Peripheral: \(bluetooth.selectedName)").font(.caption2)
                Text("Protocol: \(bluetooth.protocolVersion)").font(.caption2)

                if bluetooth.state == .scanning {
                    Button("Stop Scan") { bluetooth.stopScan() }
                } else {
                    Button("Start Scan") { bluetooth.startScan() }
                        .disabled(bluetooth.bluetoothState != "Bluetooth On" || !bluetooth.canStartScan)
                }

                ForEach(bluetooth.peripherals) { item in
                    Button("Connect: \(item.name)") { bluetooth.connect(item) }
                        .font(.caption2)
                }

                if bluetooth.state == .ready {
                    Button("Run Synthetic Test") { bluetooth.runTest() }
                }
                if bluetooth.state == .sendingFrames {
                    Button("Cancel Test") { bluetooth.cancelTest() }
                }
                if bluetooth.selectedName != "None" {
                    Button("Disconnect") { bluetooth.disconnect() }
                        .tint(.red)
                }

                Toggle("Diagnostics", isOn: Binding(get: { bluetooth.diagnosticsEnabled }, set: { bluetooth.setDiagnosticsEnabled($0) }))
                if bluetooth.diagnosticsEnabled {
                    ForEach(Array(bluetooth.diagnosticLines.enumerated()), id: \.offset) { _, line in
                        Text(line).font(.caption2)
                    }
                }
                Text(bluetooth.detail).font(.caption2).fixedSize(horizontal: false, vertical: true)
                Text("A successful exchange only tests synthetic transport. It does not authenticate or unlock.")
                    .font(.caption2).foregroundStyle(.secondary)
            }
            .padding()
        }
    }
}
