import SwiftUI

struct ContentView: View {
    @ObservedObject var bluetooth: WatchBluetoothManager

    var body: some View {
        NavigationStack {
            ScrollView {
                VStack(alignment: .leading, spacing: 8) {
                    Text("OpenContinuity").font(.headline)
                    Text("Experimental BLE transport").font(.caption2).foregroundStyle(.secondary)
                    NavigationLink("Authenticated Research") {
                        AuthenticatedResearchView(bluetooth: bluetooth)
                    }
                    .font(.caption)
                    Label(bluetooth.bluetoothState, systemImage: "dot.radiowaves.left.and.right")
                        .font(.caption)
                    Text("Stage: \(bluetooth.state.title)").font(.caption).bold()
                    Text("Peripheral: \(bluetooth.selectedName)").font(.caption2)
                    Text("Protocol: \(bluetooth.protocolVersion)").font(.caption2)

                    if bluetooth.state == .scanning {
                        Button("Stop Scan") { bluetooth.stopScan() }
                    } else {
                        Button("Start Synthetic Scan") { bluetooth.startScan() }
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

                    Toggle("Synthetic BLE Diagnostics", isOn: Binding(get: { bluetooth.diagnosticsEnabled }, set: { bluetooth.setDiagnosticsEnabled($0) }))
                    if bluetooth.diagnosticsEnabled {
                        ForEach(Array(bluetooth.diagnosticLines.enumerated()), id: \.offset) { _, line in
                            Text(line).font(.caption2)
                        }
                    }
                    Text(bluetooth.detail).font(.caption2).fixedSize(horizontal: false, vertical: true)
                    Text("Neither synthetic transport nor a signed research response authorizes or unlocks Linux.")
                        .font(.caption2).foregroundStyle(.secondary)
                }
                .padding()
            }
        }
        .sheet(item: $bluetooth.consentPresentation) { presentation in
            ResearchConsentView(
                verifiedRequest: presentation.verifiedRequest,
                watchEnrollment: presentation.watchEnrollment,
                researchSigningKey: presentation.signingKey,
                diagnostic: { outcome in bluetooth.recordLocalAuthentication(outcome, presentationID: presentation.id) }
            ) { result in
                bluetooth.completeAuthenticatedConsent(result, presentationID: presentation.id)
            }
        }
    }
}
