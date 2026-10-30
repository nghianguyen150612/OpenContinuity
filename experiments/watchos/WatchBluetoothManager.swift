import Foundation
import CoreBluetooth
import Combine
import CryptoKit
import OpenContinuityProtocol

struct ExperimentPeripheral: Identifiable {
    let peripheral: CBPeripheral
    let name: String
    var id: UUID { peripheral.identifier }
}

/// Active test-only credentials are assembled by the explicit enrollment UI.
/// The application has no persistent enrollment or production key store.
struct P009ResearchCredentials {
    let hostEnrollment: HostEnrollment
    let watchEnrollment: WatchEnrollment
    let signingKey: P256.Signing.PrivateKey
    let expiresAtUnixMilliseconds: UInt64
    let expiresAtMonotonicMilliseconds: UInt64
}

struct P009ConsentPresentation: Identifiable {
    let id = UUID()
    let verifiedRequest: VerifiedHostRequest
    let watchEnrollment: WatchEnrollment
    let signingKey: P256.Signing.PrivateKey
}

@MainActor
final class WatchBluetoothManager: NSObject, ObservableObject {
    @Published private(set) var bluetoothState = "Starting Bluetooth"
    @Published private(set) var state: GattResearchState = .idle
    @Published private(set) var peripherals: [ExperimentPeripheral] = []
    @Published private(set) var selectedName = "None"
    @Published private(set) var protocolVersion = "—"
    @Published private(set) var detail = "Synthetic transport only. No authentication or unlock."
    @Published private(set) var researchSetupGeneration = UUID()
    @Published private(set) var researchKeyCreated = false
    @Published private(set) var researchHostPinImported = false
    @Published private(set) var researchEnrollmentReady = false
    @Published private(set) var researchHostFingerprint = ""
    @Published private(set) var researchWatchFingerprint = ""
    @Published private(set) var researchHostIdentity = ""
    @Published private(set) var researchLinuxUID: UInt32 = 0
    @Published private(set) var researchEnrollmentExport = ""
    @Published private(set) var researchEnrollmentMetadata = ""
    @Published private(set) var researchSetupMessage = "No authenticated research credentials are stored."
    @Published var consentPresentation: P009ConsentPresentation?

    @Published private(set) var diagnostics = GattDiagnostics()
    var diagnosticsEnabled: Bool { diagnostics.enabled }
    var diagnosticLines: [String] { [GattDiagnostics.authorization] + diagnostics.events }
    func setDiagnosticsEnabled(_ enabled: Bool) { diagnostics.setEnabled(enabled) }

    private var central: CBCentralManager!
    private var lifecycle = GattLifecycle()
    private var pendingCharacteristic: CBCharacteristic?
    private var expectedService: CBService?
    private var connectionDelegate: ConnectionDelegate?
    var canStartScan: Bool { lifecycle.canScan && state != .scanning }
    var canEditResearchSetup: Bool {
        state == .idle || state == .completedResearchDecision || state == .cancelled || state == .disconnected || isFailedState
    }
    private var peripheral: CBPeripheral?
    private var characteristics: [CBUUID: CBCharacteristic] = [:]
    private var generationGate = GattGenerationGate()
    private var generation: UInt64 { generationGate.current }
    private var operationToken: UInt64 = 0
    private var watchdog: Task<Void, Never>?
    private var expectedRead: CBUUID?
    private var frames: [Data] = []
    private var frameIndex = 0
    private var testPayload = Data("OpenContinuity synthetic test".utf8)
    private var sessionID: [UInt8]?
    private var cancelRequested = false
    private var scanGeneration: UInt64?
    private var authCredentials: P009ResearchCredentials?
    private var temporaryResearchSigningKey: P256.Signing.PrivateKey?
    private var importedHostEnrollment: HostEnrollment?
    private var importedHostRecord: P009HostEnrollmentRecord?
    private var importedHostExpiry: UInt64?
    private var importedHostExpiryMonotonicMilliseconds: UInt64?
    private var watchEnrollmentForExport: WatchEnrollment?
    private var watchRecordForExport: P009WatchEnrollmentRecord?
    private var authTransferID: Data?
    private var authOwner: String?
    private var authFrameCount = 0
    private var authAssembler: WatchTransferAssembler?
    private var authReadCursor: WatchRequestReadCursor?
    private var authResponseFrames: [Data] = []
    private var authResponseFrameIndex = 0
    private var authRequestCompleteSentAtMonotonicMilliseconds: UInt64?
    private var authConsentDeadline: WatchAuthMonotonicDeadline?
    private var authResponseDeadline: WatchAuthMonotonicDeadline?
    private var authResponseDecision: String?
    private var authSignedExpiryMonotonicMilliseconds: UInt64?
    private var authVerifiedRequest: VerifiedHostRequest?
    private var authReplayGuard = WatchAuthReplayGuard()
    private var authPhase: AuthPhase?

    private enum AuthPhase {
        case requestGet(Int)
        case requestAck(Int)
        case requestComplete
        case responseFrame(Int)
        case responseComplete
        case cancel
    }

    override init() {
        super.init()
        central = CBCentralManager(delegate: self, queue: .main)
    }

    func startScan() {
        beginScan(authenticated: false)
    }

    func createTemporaryResearchKey() {
        guard authCredentials == nil, canEditResearchSetup else {
            researchSetupMessage = "End the current transport session before creating a new test key."
            return
        }
        temporaryResearchSigningKey = nil
        watchEnrollmentForExport = nil
        watchRecordForExport = nil
        researchWatchFingerprint = ""
        researchEnrollmentExport = ""
        researchEnrollmentReady = false
        researchSetupGeneration = UUID()
        temporaryResearchSigningKey = P256.Signing.PrivateKey()
        researchKeyCreated = true
        rebuildWatchEnrollmentIfReady()
        researchSetupMessage = "Ephemeral P-256 test key created in memory. Its private key is not exported or saved."
    }

    func importResearchHostPin(_ data: Data) {
        guard authCredentials == nil, canEditResearchSetup else {
            researchSetupMessage = "End the current authenticated research session before importing a new host pin."
            return
        }
        guard data.count <= 4_096 else {
            researchSetupMessage = "Host public pin exceeds the 4 KiB import limit. Nothing was activated."
            clearImportedHostPin()
            return
        }
        do {
            let document = try JSONDecoder().decode(P009HostPinDocument.self, from: data)
            let now = currentMilliseconds
            let host = try document.validate(nowMilliseconds: now)
            let fingerprint = p009Fingerprint(host.publicKeySEC1)
            let expiresAtMonotonic = currentMonotonicMilliseconds + (document.expiresAtUnixMs - now)
            importedHostEnrollment = host
            importedHostRecord = document.hostEnrollment
            importedHostExpiry = document.expiresAtUnixMs
            importedHostExpiryMonotonicMilliseconds = expiresAtMonotonic
            researchHostFingerprint = fingerprint
            researchHostIdentity = host.hostIdentity
            researchLinuxUID = host.linuxUID
            researchHostPinImported = true
            watchEnrollmentForExport = nil
            watchRecordForExport = nil
            researchWatchFingerprint = ""
            researchEnrollmentExport = ""
            researchEnrollmentReady = false
            researchSetupGeneration = UUID()
            rebuildWatchEnrollmentIfReady()
            researchSetupMessage = "Host test pin imported. Compare its fingerprint through a separate channel before activation."
        } catch P009EnrollmentError.expired {
            researchSetupMessage = "Host test pin is expired or exceeds its one-hour validity window. Export a fresh host pin."
            clearImportedHostPin()
        } catch P009EnrollmentError.mismatchedFingerprint {
            researchSetupMessage = "Host public key fingerprint does not match its imported record. Nothing was activated."
            clearImportedHostPin()
        } catch {
            researchSetupMessage = "Host pin is incomplete, unsupported, or not valid test-only P-256 data."
            clearImportedHostPin()
        }
    }

    func startAuthenticatedResearch(
        hostFingerprintConfirmed: Bool,
        watchFingerprintConfirmed: Bool
    ) {
        guard central.state == .poweredOn, canStartScan else {
            detail = "Authenticated research can start only from an idle foreground Bluetooth state."
            return
        }
        guard hostFingerprintConfirmed, watchFingerprintConfirmed,
              let credentials = makeResearchCredentials() else {
            researchSetupMessage = "Activation requires complete, unexpired test pins and explicit fingerprint confirmations for both keys."
            return
        }
        authCredentials = credentials
        temporaryResearchSigningKey = nil
        researchKeyCreated = false
        researchEnrollmentReady = false
        authReplayGuard = WatchAuthReplayGuard()
        researchSetupMessage = "Authenticated research started with test-only enrollment. No Linux authorization is available."
        beginScan(authenticated: true)
    }

    func prepareLinuxEnrollmentExport() {
        rebuildWatchEnrollmentIfReady()
        guard researchEnrollmentReady else {
            researchSetupMessage = "Create an ephemeral Watch key and import a complete, unexpired host pin first."
            return
        }
        researchSetupMessage = "Share this public enrollment JSON with the Linux operator. Compare the Watch fingerprint out of band before activating."
    }

    func clearAuthenticatedResearch() {
        disconnect()
        clearTemporaryResearchEnrollment(message: "Authenticated research state and ephemeral private key cleared.")
    }

    func reportResearchSetupError(_ message: String) {
        researchSetupMessage = message
    }

    private func rebuildWatchEnrollmentIfReady() {
        guard let key = temporaryResearchSigningKey,
              let host = importedHostEnrollment,
              let hostRecord = importedHostRecord,
              let expiresAt = importedHostExpiry,
              let monotonicExpiry = importedHostExpiryMonotonicMilliseconds,
              expiresAt > currentMilliseconds,
              currentMonotonicMilliseconds < monotonicExpiry else {
            researchEnrollmentReady = false
            return
        }
        if watchEnrollmentForExport != nil, watchRecordForExport != nil { return }
        let suffix = UUID().uuidString.lowercased().replacingOccurrences(of: "-", with: "")
        let enrollmentID = "test-watch-\(suffix)"
        let watchIdentity = "watch-research-\(suffix)"
        let publicKey = Data(key.publicKey.x963Representation)
        let record = P009WatchEnrollmentRecord(
            enrollmentID: enrollmentID,
            watchIdentity: watchIdentity,
            boundHostEnrollmentID: host.enrollmentID,
            boundHostIdentity: host.hostIdentity,
            boundLinuxUID: host.linuxUID,
            publicKeySEC1Hex: p009Hex(publicKey),
            keyAlgorithm: "p256_ecdsa_sha256",
            keyVersion: 1,
            state: "active",
            testOnly: true
        )
        let model = WatchEnrollment(
            enrollmentID: enrollmentID,
            watchIdentity: watchIdentity,
            boundHostEnrollmentID: host.enrollmentID,
            boundHostIdentity: host.hostIdentity,
            boundLinuxUID: host.linuxUID,
            publicKeySEC1: publicKey,
            algorithm: .p256EcdsaSha256,
            keyVersion: 1,
            state: .active,
            testOnly: true
        )
        do {
            let document = P009LinuxEnrollmentDocument(
                expiresAtUnixMs: expiresAt,
                hostEnrollment: hostRecord,
                watchEnrollment: record
            )
            let encoder = JSONEncoder()
            encoder.outputFormatting = [.prettyPrinted, .sortedKeys]
            let data = try encoder.encode(document)
            guard let text = String(data: data, encoding: .utf8) else {
                throw P009EnrollmentError.incompleteOrUnsupported
            }
            watchEnrollmentForExport = model
            watchRecordForExport = record
            researchWatchFingerprint = p009Fingerprint(publicKey)
            researchEnrollmentMetadata = "\(watchIdentity) · host \(host.hostIdentity) · Linux UID \(host.linuxUID) · expires \(expiresAt)"
            researchEnrollmentExport = text
            researchEnrollmentReady = true
            researchSetupGeneration = UUID()
        } catch {
            watchEnrollmentForExport = nil
            watchRecordForExport = nil
            researchEnrollmentExport = ""
            researchEnrollmentReady = false
            researchSetupMessage = "Could not encode the test-only public enrollment record."
        }
    }

    private func makeResearchCredentials() -> P009ResearchCredentials? {
        guard researchEnrollmentReady,
              let host = importedHostEnrollment,
              let watch = watchEnrollmentForExport,
              let signingKey = temporaryResearchSigningKey,
              let expiresAt = importedHostExpiry,
              let monotonicExpiry = importedHostExpiryMonotonicMilliseconds,
              expiresAt > currentMilliseconds,
              currentMonotonicMilliseconds < monotonicExpiry,
              signingKey.publicKey.x963Representation == watch.publicKeySEC1 else { return nil }
        return P009ResearchCredentials(
            hostEnrollment: host,
            watchEnrollment: watch,
            signingKey: signingKey,
            expiresAtUnixMilliseconds: expiresAt,
            expiresAtMonotonicMilliseconds: monotonicExpiry
        )
    }

    private var isFailedState: Bool {
        if case .failed = state { return true }
        return false
    }

    private func clearImportedHostPin() {
        importedHostEnrollment = nil
        importedHostRecord = nil
        importedHostExpiry = nil
        importedHostExpiryMonotonicMilliseconds = nil
        watchEnrollmentForExport = nil
        watchRecordForExport = nil
        researchHostPinImported = false
        researchEnrollmentReady = false
        researchHostFingerprint = ""
        researchHostIdentity = ""
        researchLinuxUID = 0
        researchWatchFingerprint = ""
        researchEnrollmentMetadata = ""
        researchEnrollmentExport = ""
        researchSetupGeneration = UUID()
    }

    private func clearTemporaryResearchEnrollment(message: String) {
        temporaryResearchSigningKey = nil
        authCredentials = nil
        clearImportedHostPin()
        researchKeyCreated = false
        researchSetupMessage = message
    }

    private func beginScan(authenticated: Bool) {
        guard central.state == .poweredOn, canStartScan else { return }
        if !authenticated { authCredentials = nil }
        invalidateSession(keepPeripheral: false, preserveAuthCredentials: authenticated)
        peripherals.removeAll()
        diagnostics.record(.scanStarted)
        state = .scanning
        generationGate.advance()
        let token = generation
        scanGeneration = token
        let service = authenticated
            ? CBUUID(string: WatchAuthTransport.service)
            : GattIdentifiers.service
        central.scanForPeripherals(withServices: [service], options: [CBCentralManagerScanOptionAllowDuplicatesKey: false])
        arm(seconds: WatchAuthTransport.bleClientScanTimeoutSeconds, token: token, message: "Scan timed out") { [weak self] in
            guard let self, self.scanGeneration == token else { return }
            self.diagnostics.record(.scanStopped)
            self.central.stopScan()
            self.scanGeneration = nil
            if self.authCredentials != nil { self.clearProtocolState() }
            self.state = .idle
            self.detail = "No experimental peripheral selected before scan timeout."
        }
    }

    func stopScan() {
        guard state == .scanning else { return }
        generationGate.advance()
        scanGeneration = nil
        watchdog?.cancel()
        diagnostics.record(.scanStopped)
        central.stopScan()
        if authCredentials != nil { clearProtocolState() }
        state = .idle
    }

    func connect(_ item: ExperimentPeripheral) {
        guard central.state == .poweredOn, state == .scanning, lifecycle.connect(item.id) else { return }
        watchdog?.cancel()
        scanGeneration = nil
        diagnostics.record(.scanStopped)
        central.stopScan()
        generationGate.advance()
        let token = generation
        peripheral = item.peripheral
        let delegate = ConnectionDelegate(owner: self, generation: lifecycle.generation)
        connectionDelegate = delegate
        peripheral?.delegate = delegate
        selectedName = item.name
        state = .connecting
        _ = lifecycle.begin(.connect)
        diagnostics.record(.connecting)
        central.connect(item.peripheral)
        arm(seconds: WatchAuthTransport.bleConnectTimeoutSeconds, token: token, message: "Connection timed out") { [weak self] in
            guard let self, self.generation == token else { return }
            self.fail("Connection timed out")
        }
    }

    func disconnect() {
        diagnostics.record(.scanStopped)
        central.stopScan()
        scanGeneration = nil
        guard !lifecycle.retiring else { return }
        generationGate.advance()
        clearProtocolState()
        lifecycle.terminate()
        guard let peripheral else { state = .disconnected; return }
        // Retain until the terminal delegate event. No new scan may orphan it.
        state = .disconnecting
        central.cancelPeripheralConnection(peripheral)
    }

    func runTest() {
        guard state == .ready, let id = sessionID, let peripheral,
              let exchange = characteristics[GattIdentifiers.exchange],
              peripheral.maximumWriteValueLength(for: .withResponse) >= 20 else {
            fail("Not ready or CoreBluetooth write limit is below 20 bytes"); return
        }
        do { frames = try GattWireCodec.frames(payload: testPayload, sessionID: id) }
        catch { fail("Could not encode test frames: \(error.localizedDescription)"); return }
        diagnostics.record(.frames(frames.count))
        frameIndex = 0
        cancelRequested = false
        state = .sendingFrames
        writeNextFrame(exchange)
    }

    func cancelTest() {
        guard state == .sendingFrames,
              characteristics[GattIdentifiers.control] != nil else { return }
        diagnostics.record(.cancelled)
        cancelRequested = true
        // A write-with-response must finish before control can be written.
        detail = "Cancellation queued after current acknowledged frame."
    }

    func applicationDidSuspend() {
        central.stopScan()
        scanGeneration = nil
        generationGate.advance()
        clearProtocolState()
        lifecycle.terminate()
        if let peripheral { central.cancelPeripheralConnection(peripheral) }
        clearTemporaryResearchEnrollment(message: "App left the foreground; test enrollment and ephemeral private key were cleared.")
        state = .cancelled
        detail = "Foreground session ended; session data discarded. Reconnect for a fresh challenge."
    }

    private func beginOperation(_ operation: GattLifecycle.Operation, characteristic: CBCharacteristic? = nil) -> Bool {
        guard lifecycle.begin(operation) != nil else { fail("Overlapping BLE operation"); return false }
        pendingCharacteristic = characteristic
        return true
    }

    fileprivate func acceptsDelegate(_ epoch: UInt64, peripheral: CBPeripheral) -> Bool {
        peripheral === self.peripheral && lifecycle.generation == epoch && !lifecycle.retiring
    }

    private func consume(_ operation: GattLifecycle.Operation, peripheral: CBPeripheral, characteristic: CBCharacteristic? = nil) -> Bool {
        guard peripheral === self.peripheral, let ticket = lifecycle.pending,
              ticket.operation == operation else { return false }
        if let characteristic, characteristic !== pendingCharacteristic { return false }
        guard lifecycle.complete(ticket, peripheral: peripheral.identifier) else { return false }
        pendingCharacteristic = nil
        return true
    }

    private func writeNextFrame(_ characteristic: CBCharacteristic) {
        guard let peripheral, generation > 0 else { fail("No active session"); return }
        if cancelRequested { sendControl(cancel: true); return }
        guard frameIndex < frames.count else { sendControl(cancel: false); return }
        let frame = frames[frameIndex]
        guard frame.count <= 20 else { fail("Frame exceeds conservative 20-byte write limit"); return }
        arm(seconds: WatchAuthTransport.bleCallbackTimeoutSeconds, token: generation, message: "Frame write timed out") { [weak self] in self?.fail("Frame write timed out") }
        guard beginOperation(.write(characteristic.uuid.uuidString), characteristic: characteristic) else { return }
        peripheral.writeValue(frame, for: characteristic, type: .withResponse)
    }

    private func sendControl(cancel: Bool) {
        guard let peripheral, let id = sessionID, let control = characteristics[GattIdentifiers.control] else { fail("Control characteristic unavailable"); return }
        let value: Data
        do { value = try GattWireCodec.control(sessionID: id, cancel: cancel) }
        catch { fail("Could not encode control command"); return }
        state = .awaitingResponse
        arm(seconds: WatchAuthTransport.bleCallbackTimeoutSeconds, token: generation, message: "Control write timed out") { [weak self] in self?.fail("Control write timed out") }
        guard beginOperation(.write(control.uuid.uuidString), characteristic: control) else { return }
        peripheral.writeValue(value, for: control, type: .withResponse)
    }

    private func read(_ uuid: CBUUID) {
        guard let peripheral, let characteristic = characteristics[uuid] else { fail("Required characteristic missing"); return }
        expectedRead = uuid
        state = uuid == GattIdentifiers.version ? .discovering : (uuid == GattIdentifiers.challenge ? .readingChallenge : .awaitingResponse)
        arm(seconds: WatchAuthTransport.bleCallbackTimeoutSeconds, token: generation, message: "Characteristic read timed out") { [weak self] in self?.fail("Characteristic read timed out") }
        guard beginOperation(.read(uuid.uuidString), characteristic: characteristic) else { return }
        peripheral.readValue(for: characteristic)
    }

    private func readAuth(_ uuid: CBUUID) {
        guard let peripheral, let characteristic = characteristics[uuid] else {
            fail("Required P009 characteristic missing")
            return
        }
        expectedRead = uuid
        state = uuid == CBUUID(string: WatchAuthTransport.status) ? .readingAuthStatus : .readingAuthRequest
        arm(seconds: WatchAuthTransport.bleCallbackTimeoutSeconds, token: generation, message: "P009 characteristic read timed out") { [weak self] in
            self?.fail("P009 characteristic read timed out")
        }
        guard beginOperation(.read(uuid.uuidString), characteristic: characteristic) else { return }
        peripheral.readValue(for: characteristic)
    }

    private func processAuthRead(_ uuid: CBUUID, value: Data) {
        do {
            if uuid == CBUUID(string: WatchAuthTransport.status) {
                guard value.count == 13, Array(value.prefix(3)) == [0x4f, 0x41, WatchAuthTransport.version] else {
                    throw WatchTransportError.malformed
                }
                let transferID = Data(value[3..<11])
                let envelopeLength = Int(UInt16(value[11]) | (UInt16(value[12]) << 8))
                guard (WatchAuthTransport.signatureBytes + 1...WatchAuthTransport.maximumEnvelopeBytes).contains(envelopeLength) else {
                    throw WatchTransportError.messageTooLarge
                }
                let owner = peripheral?.identifier.uuidString ?? ""
                guard !owner.isEmpty else { throw WatchTransportError.ownerMismatch }
                let count = (envelopeLength + 4) / 5
                authTransferID = transferID
                authOwner = owner
                authFrameCount = count
                authAssembler = try WatchTransferAssembler(
                    transferID: transferID,
                    direction: .hostToWatch,
                    owner: owner,
                    createdAtMilliseconds: currentMonotonicMilliseconds
                )
                authReadCursor = try WatchRequestReadCursor(
                    transferID: transferID,
                    owner: owner,
                    totalFrameCount: count,
                    nowMilliseconds: currentMonotonicMilliseconds
                )
                try requestAuthFrame(0)
                return
            }

            guard uuid == CBUUID(string: WatchAuthTransport.requestData),
                  let transferID = authTransferID,
                  let owner = authOwner,
                  var assembler = authAssembler,
                  var cursor = authReadCursor else {
                throw WatchTransportError.malformed
            }
            let index = cursor.index
            try assembler.push(
                owner: owner,
                direction: .hostToWatch,
                frame: value,
                mtu: 23,
                nowMilliseconds: currentMonotonicMilliseconds
            )
            try cursor.didRead(owner: owner, transferID: transferID, index: index, nowMilliseconds: currentMonotonicMilliseconds)
            authAssembler = assembler
            authReadCursor = cursor
            try writeAuthCommand(.acknowledge(UInt8(index)), characteristicUUID: WatchAuthTransport.requestControl, phase: .requestAck(index))
        } catch {
            fail("P009 host request transport rejected a value: \(error.localizedDescription)")
        }
    }

    private func requestAuthFrame(_ index: Int) throws {
        guard let transferID = authTransferID, let owner = authOwner, var cursor = authReadCursor,
              index >= 0, index < authFrameCount, let byteIndex = UInt8(exactly: index) else {
            throw WatchTransportError.duplicateOrOutOfOrder
        }
        try cursor.requested(owner: owner, transferID: transferID, index: index, nowMilliseconds: currentMonotonicMilliseconds)
        authReadCursor = cursor
        try writeAuthCommand(.get(byteIndex), characteristicUUID: WatchAuthTransport.requestControl, phase: .requestGet(index))
    }

    private func writeAuthCommand(
        _ command: WatchRequestCommand,
        characteristicUUID: String,
        phase: AuthPhase
    ) throws {
        guard let transferID = authTransferID,
              let characteristic = characteristics[CBUUID(string: characteristicUUID)] else {
            throw WatchTransportError.invalidChunkCommand
        }
        let value = try WatchAuthTransportCodec.requestCommand(transferID: transferID, command: command)
        try writeAuthValue(value, characteristic: characteristic, phase: phase)
    }

    private func writeAuthValue(_ value: Data, characteristic: CBCharacteristic, phase: AuthPhase) throws {
        guard let peripheral, value.count <= 20,
              peripheral.maximumWriteValueLength(for: .withResponse) >= 20 else {
            throw WatchTransportError.frameTooLarge
        }
        authPhase = phase
        switch phase {
        case .responseFrame(_), .responseComplete:
            state = .sendingAuthResponse
        default:
            state = .sendingAuthControl
        }
        arm(seconds: WatchAuthTransport.bleCallbackTimeoutSeconds, token: generation, message: "P009 write-with-response timed out") { [weak self] in
            self?.fail("P009 write-with-response timed out")
        }
        guard beginOperation(.write(characteristic.uuid.uuidString), characteristic: characteristic) else { return }
        if case .requestComplete = phase {
            authRequestCompleteSentAtMonotonicMilliseconds = currentMonotonicMilliseconds
        }
        peripheral.writeValue(value, for: characteristic, type: .withResponse)
    }

    private func finishAuthRequest() throws {
        guard let transferID = authTransferID, let owner = authOwner,
              var assembler = authAssembler, let credentials = authCredentials else {
            throw WatchTransportError.incompleteTransfer
        }
        let envelope = try assembler.complete(owner: owner, transferID: transferID, nowMilliseconds: currentMonotonicMilliseconds)
        state = .verifyingAuthRequest
        let verifiedAtWallMilliseconds = currentMilliseconds
        let verifiedAtMonotonicMilliseconds = currentMonotonicMilliseconds
        let verified = try WatchAuthProtocol.verifyHostTransportEnvelope(
            bytes: envelope.bytes,
            enrollment: credentials.hostEnrollment,
            expectedWatchEnrollmentID: credentials.watchEnrollment.enrollmentID,
            nowMilliseconds: verifiedAtWallMilliseconds,
            replayGuard: &authReplayGuard
        )
        try WatchAuthProtocol.validateWatchResponseEligibility(
            verifiedRequest: verified,
            enrollment: credentials.watchEnrollment,
            signingKey: credentials.signingKey
        )
        guard verifiedAtWallMilliseconds < verified.request.expiresAtMilliseconds else {
            throw WatchAuthError.expired
        }
        let signedExpiryMonotonic = verifiedAtMonotonicMilliseconds
            + (verified.request.expiresAtMilliseconds - verifiedAtWallMilliseconds)
        authSignedExpiryMonotonicMilliseconds = signedExpiryMonotonic
        guard let consentStartedAt = authRequestCompleteSentAtMonotonicMilliseconds else {
            throw WatchTransportError.incompleteTransfer
        }
        authConsentDeadline = WatchAuthMonotonicDeadline(
            startedAtMilliseconds: consentStartedAt,
            timeoutMilliseconds: WatchAuthTransport.waitingForConsentTimeoutMilliseconds,
            hardDeadlineMilliseconds: signedExpiryMonotonic
        )
        authAssembler = assembler
        authVerifiedRequest = verified
        consentPresentation = P009ConsentPresentation(
            verifiedRequest: verified,
            watchEnrollment: credentials.watchEnrollment,
            signingKey: credentials.signingKey
        )
        guard currentMilliseconds < verified.request.expiresAtMilliseconds,
              authConsentDeadline?.isActive(atMilliseconds: currentMonotonicMilliseconds) == true else {
            throw WatchAuthError.expired
        }
        state = .awaitingResearchConsent
        detail = "P009 request signature, enrollment, action, UID, session, freshness, and replay checks passed. Awaiting an explicit choice."
        guard let consentDeadline = authConsentDeadline else {
            throw WatchTransportError.incompleteTransfer
        }
        let consentRemaining = consentDeadline.remainingMilliseconds(atMilliseconds: currentMonotonicMilliseconds)
        let consentSeconds = max(1, (consentRemaining + 999) / 1_000)
        arm(seconds: consentSeconds, token: generation, message: "P009 consent deadline elapsed") { [weak self] in
            self?.fail("Waiting-for-consent deadline elapsed or signed P008 request expired. Start again for a fresh request, nonce, and session.")
        }
    }

    func completeAuthenticatedConsent(
        _ result: Result<SignedWatchResponse, ResearchConsentFailure>,
        presentationID: UUID
    ) {
        guard consentPresentation?.id == presentationID,
              state == .awaitingResearchConsent else { return }
        guard authConsentDeadline?.isActive(atMilliseconds: currentMonotonicMilliseconds) == true,
              let signedDeadline = authSignedExpiryMonotonicMilliseconds,
              currentMonotonicMilliseconds < signedDeadline,
              currentMilliseconds < (authVerifiedRequest?.request.expiresAtMilliseconds ?? 0) else {
            fail("Waiting-for-consent deadline or signed P008 expiry elapsed. Start again for a fresh request, nonce, and session.")
            return
        }
        watchdog?.cancel()
        consentPresentation = nil
        authConsentDeadline = nil
        switch result {
        case .failure:
            do {
                try writeAuthCommand(.cancel, characteristicUUID: WatchAuthTransport.responseControl, phase: .cancel)
                detail = "Research was cancelled or consent failed. No approval was signed; P009 transport is being cancelled."
            } catch {
                fail("Could not cancel P009 transport: \(error.localizedDescription)")
            }
        case .success(let signed):
            do {
                guard let transferID = authTransferID else { throw WatchTransportError.sessionMismatch }
                let now = currentMilliseconds
                guard let credentials = authCredentials,
                      let expiresAt = authVerifiedRequest?.request.expiresAtMilliseconds,
                      now < expiresAt,
                      let monotonicExpiry = authSignedExpiryMonotonicMilliseconds,
                      currentMonotonicMilliseconds < monotonicExpiry,
                      signed.response.responseAtMilliseconds < expiresAt,
                      signed.response.responseAtMilliseconds <= now,
                      credentials.expiresAtUnixMilliseconds > now,
                      currentMonotonicMilliseconds < credentials.expiresAtMonotonicMilliseconds else {
                    throw WatchAuthError.expired
                }
                let envelope = try WatchSignedEnvelope(
                    direction: .watchToHost,
                    canonical: signed.response.canonicalBytes(),
                    signatureRaw: signed.signatureRaw
                )
                authResponseFrames = try WatchAuthTransportCodec.frames(envelope: envelope, transferID: transferID, mtu: 23)
                authResponseFrameIndex = 0
                authResponseDeadline = WatchAuthMonotonicDeadline(
                    startedAtMilliseconds: currentMonotonicMilliseconds,
                    timeoutMilliseconds: WatchAuthTransport.responseTransferTimeoutMilliseconds,
                    hardDeadlineMilliseconds: signedDeadline
                )
                authResponseDecision = signed.response.decision == .approved ? "approved" : "rejected"
                state = .sendingAuthResponse
                try sendNextAuthResponseFrame()
            } catch {
                fail("Could not frame signed P009 response: \(error.localizedDescription)")
            }
        }
    }

    private func sendNextAuthResponseFrame() throws {
        guard let peripheral,
              let characteristic = characteristics[CBUUID(string: WatchAuthTransport.responseData)] else {
            throw WatchTransportError.incompleteTransfer
        }
        guard authResponseDeadline?.isActive(atMilliseconds: currentMonotonicMilliseconds) == true else {
            throw WatchTransportError.transferExpired
        }
        guard authResponseFrameIndex < authResponseFrames.count else {
            try writeAuthCommand(.complete, characteristicUUID: WatchAuthTransport.responseControl, phase: .responseComplete)
            return
        }
        let frame = authResponseFrames[authResponseFrameIndex]
        guard frame.count <= 20, peripheral.maximumWriteValueLength(for: .withResponse) >= 20 else {
            throw WatchTransportError.frameTooLarge
        }
        try writeAuthValue(frame, characteristic: characteristic, phase: .responseFrame(authResponseFrameIndex))
    }

    private var currentMilliseconds: UInt64 {
        UInt64(max(0, Date().timeIntervalSince1970 * 1_000))
    }

    private var currentMonotonicMilliseconds: UInt64 {
        DispatchTime.now().uptimeNanoseconds / 1_000_000
    }

    private func arm(seconds: UInt64, token: UInt64, message: String, action: @escaping @MainActor () -> Void) {
        watchdog?.cancel()
        operationToken &+= 1
        let op = operationToken
        watchdog = Task { [weak self] in
            try? await Task.sleep(for: .seconds(seconds))
            guard !Task.isCancelled, let self, self.generation == token, self.operationToken == op else { return }
            action()
        }
    }

    private func clearProtocolState(preserveAuthCredentials: Bool = false) {
        diagnostics.record(.invalidated)
        watchdog?.cancel()
        watchdog = nil
        expectedRead = nil
        expectedService = nil
        pendingCharacteristic = nil
        frames.removeAll()
        frameIndex = 0
        sessionID = nil
        cancelRequested = false
        protocolVersion = "—"
        authTransferID = nil
        authOwner = nil
        authFrameCount = 0
        authAssembler = nil
        authReadCursor = nil
        authResponseFrames.removeAll()
        authResponseFrameIndex = 0
        authRequestCompleteSentAtMonotonicMilliseconds = nil
        authConsentDeadline = nil
        authResponseDeadline = nil
        authResponseDecision = nil
        authSignedExpiryMonotonicMilliseconds = nil
        authVerifiedRequest = nil
        authPhase = nil
        consentPresentation = nil
        if !preserveAuthCredentials {
            authCredentials = nil
            clearTemporaryResearchEnrollment(message: "Research session ended; ephemeral test credentials were cleared.")
            authReplayGuard = WatchAuthReplayGuard()
        }
    }

    private func finishAuthenticatedTransport(state terminalState: GattResearchState, detail terminalDetail: String) {
        let completedPeripheral = peripheral
        clearProtocolState()
        lifecycle.terminate()
        state = terminalState
        detail = terminalDetail
        if let completedPeripheral {
            central.cancelPeripheralConnection(completedPeripheral)
        }
    }

    private func invalidateSession(keepPeripheral: Bool) {
        invalidateSession(keepPeripheral: keepPeripheral, preserveAuthCredentials: false)
    }

    private func invalidateSession(keepPeripheral: Bool, preserveAuthCredentials: Bool) {
        generationGate.advance()
        clearProtocolState(preserveAuthCredentials: preserveAuthCredentials)
        characteristics.removeAll()
        if !keepPeripheral { peripheral = nil; selectedName = "None" }
    }

    private func fail(_ reason: String) {
        diagnostics.record(.failed)
        generationGate.advance()
        lifecycle.terminate()
        watchdog?.cancel()
        clearProtocolState()
        state = .failed(reason)
        detail = reason
        if let peripheral { central.cancelPeripheralConnection(peripheral) }
    }

    private func resetToIdle() { invalidateSession(keepPeripheral: false); state = .idle }
}

extension WatchBluetoothManager: @preconcurrency CBCentralManagerDelegate {
    func centralManagerDidUpdateState(_ central: CBCentralManager) {
        guard central === self.central else { return }
        switch central.state {
        case .poweredOn: bluetoothState = "Bluetooth On"
        case .poweredOff: bluetoothState = "Bluetooth Off"; disconnect()
        case .unauthorized: bluetoothState = "Bluetooth Unauthorized"; disconnect()
        case .unsupported: bluetoothState = "Bluetooth Unsupported"; disconnect()
        case .resetting: bluetoothState = "Bluetooth Resetting"; disconnect()
        case .unknown: bluetoothState = "Bluetooth Unknown"
        @unknown default: bluetoothState = "Bluetooth Unavailable"
        }
    }

    func centralManager(_ central: CBCentralManager, didDiscover peripheral: CBPeripheral, advertisementData: [String: Any], rssi RSSI: NSNumber) {
        guard central === self.central, lifecycle.canScan, state == .scanning, scanGeneration == generation, !peripherals.contains(where: { $0.id == peripheral.identifier }) else { return }
        let name = peripheral.name ?? (advertisementData[CBAdvertisementDataLocalNameKey] as? String) ?? "Unnamed Linux peripheral"
        diagnostics.record(.discovered)
        peripherals.append(ExperimentPeripheral(peripheral: peripheral, name: name))
    }

    func centralManager(_ central: CBCentralManager, didConnect peripheral: CBPeripheral) {
        guard central === self.central, state == .connecting, consume(.connect, peripheral: peripheral) else { return }
        watchdog?.cancel()
        state = .discovering
        diagnostics.record(.connected)
        arm(seconds: WatchAuthTransport.bleDiscoveryTimeoutSeconds, token: generation, message: "Service discovery timed out") { [weak self] in self?.fail("Service discovery timed out") }
        guard beginOperation(.services) else { return }
        let service = authCredentials == nil
            ? GattIdentifiers.service
            : CBUUID(string: WatchAuthTransport.service)
        peripheral.discoverServices([service])
    }

    func centralManager(_ central: CBCentralManager, didFailToConnect peripheral: CBPeripheral, error: Error?) {
        guard central === self.central, peripheral === self.peripheral,
              state == .connecting || lifecycle.retiring else { return }
        finishDisconnect(peripheral)
        state = .failed("Connection failed: \(error?.localizedDescription ?? "unknown error")")
    }

    func centralManager(_ central: CBCentralManager, didDisconnectPeripheral peripheral: CBPeripheral, error: Error?) {
        guard central === self.central else { return }
        finishDisconnect(peripheral)
    }

    private func finishDisconnect(_ peripheral: CBPeripheral) {
        guard peripheral === self.peripheral, lifecycle.disconnected(peripheral.identifier) else { return }
        generationGate.advance()
        clearProtocolState()
        diagnostics.record(.disconnected)
        peripheral.delegate = nil
        connectionDelegate = nil
        self.peripheral = nil
        characteristics.removeAll()
        peripherals.removeAll()
        selectedName = "None"
        state = .disconnected
        // A new central owns the next cycle. Late central events from the old
        // instance cannot be applied to a repeated connection to the same ID.
        central.delegate = nil
        central = CBCentralManager(delegate: self, queue: .main)
    }

}

extension WatchBluetoothManager: @preconcurrency CBPeripheralDelegate {
    func peripheral(_ peripheral: CBPeripheral, didDiscoverServices error: Error?) {
        guard state == .discovering, consume(.services, peripheral: peripheral) else { return }
        if authCredentials != nil {
            let authService = CBUUID(string: WatchAuthTransport.service)
            guard error == nil, let service = peripheral.services?.first(where: { $0.uuid == authService }) else {
                fail("P009 authenticated research service missing or discovery failed")
                return
            }
            expectedService = service
            guard beginOperation(.characteristics) else { return }
            peripheral.discoverCharacteristics([
                CBUUID(string: WatchAuthTransport.requestData),
                CBUUID(string: WatchAuthTransport.requestControl),
                CBUUID(string: WatchAuthTransport.responseData),
                CBUUID(string: WatchAuthTransport.responseControl),
                CBUUID(string: WatchAuthTransport.status)
            ], for: service)
            return
        }
        guard error == nil, let service = peripheral.services?.first(where: { $0.uuid == GattIdentifiers.service }) else { fail("Experimental service missing or discovery failed"); return }
        diagnostics.record(.serviceDiscovered)
        expectedService = service
        guard beginOperation(.characteristics) else { return }
        peripheral.discoverCharacteristics([GattIdentifiers.version, GattIdentifiers.challenge, GattIdentifiers.exchange, GattIdentifiers.response, GattIdentifiers.control], for: service)
    }

    func peripheral(_ peripheral: CBPeripheral, didDiscoverCharacteristicsFor service: CBService, error: Error?) {
        if authCredentials != nil, service.uuid == CBUUID(string: WatchAuthTransport.service) {
            guard state == .discovering, service === expectedService,
                  consume(.characteristics, peripheral: peripheral) else { return }
            guard error == nil, let found = service.characteristics else {
                fail("P009 characteristic discovery failed")
                return
            }
            for characteristic in found { characteristics[characteristic.uuid] = characteristic }
            let requestData = characteristics[CBUUID(string: WatchAuthTransport.requestData)]
            let requestControl = characteristics[CBUUID(string: WatchAuthTransport.requestControl)]
            let responseData = characteristics[CBUUID(string: WatchAuthTransport.responseData)]
            let responseControl = characteristics[CBUUID(string: WatchAuthTransport.responseControl)]
            let status = characteristics[CBUUID(string: WatchAuthTransport.status)]
            guard characteristics.count == 5,
                  requestData?.properties.contains(.read) == true,
                  requestControl?.properties.contains(.write) == true,
                  responseData?.properties.contains(.write) == true,
                  responseControl?.properties.contains(.write) == true,
                  status?.properties.contains(.read) == true else {
                fail("Required P009 characteristic or properties missing")
                return
            }
            protocolVersion = "P009/1"
            readAuth(CBUUID(string: WatchAuthTransport.status))
            return
        }
        guard state == .discovering, service === expectedService, service.uuid == GattIdentifiers.service, consume(.characteristics, peripheral: peripheral) else { return }
        guard error == nil, let found = service.characteristics else { fail("Characteristic discovery failed"); return }
        for characteristic in found { characteristics[characteristic.uuid] = characteristic }
        guard characteristics.count == 5,
              characteristics[GattIdentifiers.version]?.properties.contains(.read) == true,
              characteristics[GattIdentifiers.challenge]?.properties.contains(.read) == true,
              characteristics[GattIdentifiers.response]?.properties.contains(.read) == true,
              characteristics[GattIdentifiers.exchange]?.properties.contains(.write) == true,
              characteristics[GattIdentifiers.control]?.properties.contains(.write) == true else { fail("Required GATT characteristic or properties missing"); return }
        diagnostics.record(.characteristicsDiscovered)
        read(GattIdentifiers.version)
    }

    func peripheral(_ peripheral: CBPeripheral, didUpdateValueFor characteristic: CBCharacteristic, error: Error?) {
        guard let expectedRead, characteristic.uuid == expectedRead, consume(.read(characteristic.uuid.uuidString), peripheral: peripheral, characteristic: characteristic) else { return }
        watchdog?.cancel()
        self.expectedRead = nil
        guard error == nil, let value = characteristic.value else { fail("Characteristic read failed"); return }
        if authCredentials != nil {
            processAuthRead(characteristic.uuid, value: value)
            return
        }
        do {
            if characteristic.uuid == GattIdentifiers.version {
                try GattWireCodec.parseVersion(value)
                diagnostics.record(.versionVerified)
                protocolVersion = "1"
                read(GattIdentifiers.challenge)
            } else if characteristic.uuid == GattIdentifiers.challenge {
                diagnostics.record(.challengeLength(value.count))
                let challenge = try GattWireCodec.parseChallenge(value)
                sessionID = challenge.sessionID
                state = .ready
                detail = "Fresh synthetic challenge read. Long-read completeness must be hardware verified."
            } else if characteristic.uuid == GattIdentifiers.response {
                diagnostics.record(.responseLength(value.count))
                guard let id = sessionID else { throw GattWireCodec.WireError.session }
                let cancelled = cancelRequested
                try GattWireCodec.verifyResponse(value, sessionID: id, payload: testPayload, cancelled: cancelled)
                diagnostics.record(.digestVerified)
                state = cancelled ? .cancelled : .completed
                detail = cancelled ? "Synthetic transport test cancelled and verified." : "Synthetic transport exchange verified. Not authentication."
                clearProtocolStatePreservingResult()
            } else { fail("Unexpected value update"); return }
        } catch { fail("Invalid characteristic value: \(error.localizedDescription)") }
    }

    func peripheral(_ peripheral: CBPeripheral, didWriteValueFor characteristic: CBCharacteristic, error: Error?) {
        guard consume(.write(characteristic.uuid.uuidString), peripheral: peripheral, characteristic: characteristic) else { return }
        watchdog?.cancel()
        guard error == nil else { fail("Write failed: \(error!.localizedDescription)"); return }
        if authCredentials != nil, let phase = authPhase {
            authPhase = nil
            do {
                switch phase {
                case .requestGet(_):
                    readAuth(CBUUID(string: WatchAuthTransport.requestData))
                case .requestAck(let index):
                    guard let transferID = authTransferID, let owner = authOwner, var cursor = authReadCursor else {
                        throw WatchTransportError.incompleteTransfer
                    }
                    try cursor.acknowledge(owner: owner, transferID: transferID, index: index, nowMilliseconds: currentMonotonicMilliseconds)
                    authReadCursor = cursor
                    if index + 1 < authFrameCount {
                        try requestAuthFrame(index + 1)
                    } else {
                        try cursor.complete(owner: owner, transferID: transferID, nowMilliseconds: currentMonotonicMilliseconds)
                        authReadCursor = cursor
                        try writeAuthCommand(.complete, characteristicUUID: WatchAuthTransport.requestControl, phase: .requestComplete)
                    }
                case .requestComplete:
                    try finishAuthRequest()
                case .responseFrame(let index):
                    guard index == authResponseFrameIndex else { throw WatchTransportError.duplicateOrOutOfOrder }
                    authResponseFrameIndex += 1
                    try sendNextAuthResponseFrame()
                case .responseComplete:
                    let decision = authResponseDecision ?? "unknown"
                    finishAuthenticatedTransport(
                        state: .completedResearchDecision,
                        detail: "Signed research response sent: \(decision). This is transport-only; Linux verification and authorization were not inferred by the Watch."
                    )
                case .cancel:
                    finishAuthenticatedTransport(
                        state: .cancelled,
                        detail: "P009 research session cancelled. No signed approval was sent."
                    )
                }
            } catch {
                fail("P009 transport operation failed: \(error.localizedDescription)")
            }
            return
        }
        if characteristic.uuid == GattIdentifiers.exchange, state == .sendingFrames {
            frameIndex += 1
            diagnostics.record(.acknowledged(frameIndex))
            let token = generation
            Task { @MainActor [weak self] in
                await Task.yield()
                guard let self, self.generation == token, self.state == .sendingFrames else { return }
                self.writeNextFrame(characteristic)
            }
        } else if characteristic.uuid == GattIdentifiers.control, state == .awaitingResponse {
            diagnostics.record(.controlAcknowledged)
            read(GattIdentifiers.response)
        } else { fail("Unexpected write completion") }
    }

    func peripheral(_ peripheral: CBPeripheral, didModifyServices invalidatedServices: [CBService]) {
        guard peripheral === self.peripheral,
              invalidatedServices.contains(where: {
                  $0.uuid == GattIdentifiers.service || $0.uuid == CBUUID(string: WatchAuthTransport.service)
              }) else { return }
        fail("Experimental service invalidated")
    }

    private func clearProtocolStatePreservingResult() {
        watchdog?.cancel(); watchdog = nil
        sessionID = nil; frames.removeAll(); expectedRead = nil; cancelRequested = false
    }
}

// A delegate belongs to one connection lifetime, even when CoreBluetooth returns
// the same CBPeripheral object again. Its captured epoch is never reassigned.
@MainActor
private final class ConnectionDelegate: NSObject {
    weak var owner: WatchBluetoothManager?
    let generation: UInt64
    init(owner: WatchBluetoothManager, generation: UInt64) {
        self.owner = owner
        self.generation = generation
    }
    private func active(_ peripheral: CBPeripheral) -> WatchBluetoothManager? {
        guard let owner, owner.acceptsDelegate(generation, peripheral: peripheral) else { return nil }
        return owner
    }
}

extension ConnectionDelegate: @preconcurrency CBPeripheralDelegate {
    func peripheral(_ peripheral: CBPeripheral, didDiscoverServices error: Error?) {
        active(peripheral)?.peripheral(peripheral, didDiscoverServices: error)
    }
    func peripheral(_ peripheral: CBPeripheral, didDiscoverCharacteristicsFor service: CBService, error: Error?) {
        active(peripheral)?.peripheral(peripheral, didDiscoverCharacteristicsFor: service, error: error)
    }
    func peripheral(_ peripheral: CBPeripheral, didUpdateValueFor characteristic: CBCharacteristic, error: Error?) {
        active(peripheral)?.peripheral(peripheral, didUpdateValueFor: characteristic, error: error)
    }
    func peripheral(_ peripheral: CBPeripheral, didWriteValueFor characteristic: CBCharacteristic, error: Error?) {
        active(peripheral)?.peripheral(peripheral, didWriteValueFor: characteristic, error: error)
    }
    func peripheral(_ peripheral: CBPeripheral, didModifyServices invalidatedServices: [CBService]) {
        active(peripheral)?.peripheral(peripheral, didModifyServices: invalidatedServices)
    }
}
