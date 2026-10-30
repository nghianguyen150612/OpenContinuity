import SwiftUI
import UniformTypeIdentifiers
import OpenContinuityProtocol

struct AuthenticatedResearchView: View {
    @ObservedObject var bluetooth: WatchBluetoothManager
    @State private var selectingHostPin = false
    @State private var hostFingerprintConfirmed = false
    @State private var watchFingerprintConfirmed = false

    var body: some View {
        ScrollView {
            VStack(alignment: .leading, spacing: 10) {
                Text("Authenticated Research")
                    .font(.headline)
                Text("Explicit test-only BLE research. A signed result never unlocks Linux and does not prove genuine Watch identity, proximity, or wearer presence.")
                    .font(.caption2)
                    .fixedSize(horizontal: false, vertical: true)

                Button("Create ephemeral Watch key") {
                    hostFingerprintConfirmed = false
                    watchFingerprintConfirmed = false
                    bluetooth.createTemporaryResearchKey()
                }
                .disabled(bluetooth.researchKeyCreated || !bluetooth.canEditResearchSetup)

                Text(bluetooth.researchKeyCreated
                     ? "A temporary P-256 private key exists in memory only."
                     : "No Watch research key is active.")
                    .font(.caption2)

                Button("Import Linux host public pin") { selectingHostPin = true }
                    .disabled(!bluetooth.researchKeyCreated || !bluetooth.canEditResearchSetup)

                if bluetooth.researchHostPinImported {
                    Group {
                        Text("Host: \(bluetooth.researchHostIdentity)")
                        Text("Linux UID: \(bluetooth.researchLinuxUID)")
                        Text("Host SHA-256")
                        Text(p009DisplayFingerprint(bluetooth.researchHostFingerprint))
                            .font(.system(.caption2, design: .monospaced))
                            .textSelection(.enabled)
                        Toggle("I compared the host fingerprint over a separate channel", isOn: $hostFingerprintConfirmed)
                            .font(.caption2)
                    }
                    .font(.caption2)
                }

                if bluetooth.researchEnrollmentReady {
                    Text("Watch enrollment")
                        .font(.caption.bold())
                    Text(bluetooth.researchEnrollmentMetadata)
                        .font(.caption2)
                        .fixedSize(horizontal: false, vertical: true)
                    Text("Watch SHA-256")
                        .font(.caption2)
                    Text(p009DisplayFingerprint(bluetooth.researchWatchFingerprint))
                        .font(.system(.caption2, design: .monospaced))
                        .textSelection(.enabled)
                    ShareLink(item: bluetooth.researchEnrollmentExport) {
                        Label("Share public enrollment with Linux operator", systemImage: "square.and.arrow.up")
                    }
                    .font(.caption2)
                    Toggle("Linux operator confirmed this Watch fingerprint is pinned", isOn: $watchFingerprintConfirmed)
                        .font(.caption2)
                    Button("Start Authenticated Research") {
                        bluetooth.startAuthenticatedResearch(
                            hostFingerprintConfirmed: hostFingerprintConfirmed,
                            watchFingerprintConfirmed: watchFingerprintConfirmed
                        )
                    }
                    .disabled(!hostFingerprintConfirmed || !watchFingerprintConfirmed || bluetooth.bluetoothState != "Bluetooth On" || !bluetooth.canStartScan || !bluetooth.canEditResearchSetup)
                    .tint(.orange)
                }

                ForEach(bluetooth.peripherals) { item in
                    Button("Connect to \(item.name)") {
                        bluetooth.connect(item)
                    }
                    .font(.caption2)
                    .disabled(bluetooth.state != .scanning)
                }

                if bluetooth.state == .scanning || bluetooth.state == .connecting || bluetooth.state == .discovering || bluetooth.state == .readingAuthStatus || bluetooth.state == .sendingAuthControl || bluetooth.state == .readingAuthRequest || bluetooth.state == .verifyingAuthRequest || bluetooth.state == .awaitingResearchConsent || bluetooth.state == .sendingAuthResponse {
                    Button("Cancel Authenticated Research") {
                        bluetooth.clearAuthenticatedResearch()
                        hostFingerprintConfirmed = false
                        watchFingerprintConfirmed = false
                    }
                    .tint(.red)
                }

                Text("Bluetooth: \(bluetooth.bluetoothState)")
                    .font(.caption2)
                Text("Stage: \(bluetooth.state.title)")
                    .font(.caption2.bold())
                Text(bluetooth.researchSetupMessage)
                    .font(.caption2)
                    .fixedSize(horizontal: false, vertical: true)
                Text(bluetooth.detail)
                    .font(.caption2)
                    .fixedSize(horizontal: false, vertical: true)
                Text("Every credential is test_only. Authorization remains never_evaluated.")
                    .font(.caption2)
                    .foregroundStyle(.secondary)
            }
            .padding()
        }
        .navigationTitle("Research")
        .fileImporter(
            isPresented: $selectingHostPin,
            allowedContentTypes: [.json],
            allowsMultipleSelection: false
        ) { result in
            do {
                guard let url = try result.get().first else { return }
                let hasScopedAccess = url.startAccessingSecurityScopedResource()
                defer {
                    if hasScopedAccess { url.stopAccessingSecurityScopedResource() }
                }
                let data = try Data(contentsOf: url)
                bluetooth.importResearchHostPin(data)
                hostFingerprintConfirmed = false
                watchFingerprintConfirmed = false
            } catch {
                bluetooth.reportResearchSetupError("Could not import host public pin: \(error.localizedDescription)")
            }
        }
        .onChange(of: bluetooth.researchSetupGeneration) { _, _ in
            hostFingerprintConfirmed = false
            watchFingerprintConfirmed = false
        }
    }
}
