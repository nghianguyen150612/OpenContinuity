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

/// Test-only credentials are injected by an in-memory research harness. The
/// application has no persistent enrollment or key provisioning path.
struct P009ResearchCredentials {
    let hostEnrollment: HostEnrollment
    let watchEnrollment: WatchEnrollment
    let signingKey: P256.Signing.PrivateKey
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
    @Published var consentPresentation: P009ConsentPresentation?

    private var central: CBCentralManager!
    private var lifecycle = GattLifecycle()
    private var pendingCharacteristic: CBCharacteristic?
    private var expectedService: CBService?
    private var connectionDelegate: ConnectionDelegate?
    var canStartScan: Bool { lifecycle.canScan && state != .scanning }
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
    private var authTransferID: Data?
    private var authOwner: String?
    private var authFrameCount = 0
    private var authAssembler: WatchTransferAssembler?
    private var authReadCursor: WatchRequestReadCursor?
    private var authResponseFrames: [Data] = []
    private var authResponseFrameIndex = 0
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

    /// Starts the P009 scan only when a caller injects independently enrolled
    /// test records and the matching temporary signing key in memory.
    func startAuthenticatedResearch(credentials: P009ResearchCredentials) {
        guard central.state == .poweredOn, canStartScan else {
            detail = "P009 can start only from an idle foreground Bluetooth state."
            return
        }
        guard credentials.hostEnrollment.testOnly,
              credentials.watchEnrollment.testOnly,
              credentials.watchEnrollment.boundHostEnrollmentID == credentials.hostEnrollment.enrollmentID,
              credentials.watchEnrollment.boundHostIdentity == credentials.hostEnrollment.hostIdentity,
              credentials.watchEnrollment.boundLinuxUID == credentials.hostEnrollment.linuxUID,
              credentials.signingKey.publicKey.x963Representation == credentials.watchEnrollment.publicKeySEC1 else {
            detail = "P009 requires matching test-only pins and an injected temporary Watch key."
            return
        }
        authCredentials = credentials
        beginScan(authenticated: true)
    }

    private func beginScan(authenticated: Bool) {
        guard central.state == .poweredOn, canStartScan else { return }
        if !authenticated { authCredentials = nil }
        invalidateSession(keepPeripheral: false, preserveAuthCredentials: authenticated)
        peripherals.removeAll()
        state = .scanning
        generationGate.advance()
        let token = generation
        scanGeneration = token
        let service = authenticated
            ? CBUUID(string: WatchAuthTransport.service)
            : GattIdentifiers.service
        central.scanForPeripherals(withServices: [service], options: [CBCentralManagerScanOptionAllowDuplicatesKey: false])
        arm(seconds: 15, token: token, message: "Scan timed out") { [weak self] in
            guard let self, self.scanGeneration == token else { return }
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
        central.stopScan()
        if authCredentials != nil { clearProtocolState() }
        state = .idle
    }

    func connect(_ item: ExperimentPeripheral) {
        guard central.state == .poweredOn, state == .scanning, lifecycle.connect(item.id) else { return }
        watchdog?.cancel()
        scanGeneration = nil
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
        central.connect(item.peripheral)
        arm(seconds: 15, token: token, message: "Connection timed out") { [weak self] in
            guard let self, self.generation == token else { return }
            self.fail("Connection timed out")
        }
    }

    func disconnect() {
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
        frameIndex = 0
        cancelRequested = false
        state = .sendingFrames
        writeNextFrame(exchange)
    }

    func cancelTest() {
        guard state == .sendingFrames,
              characteristics[GattIdentifiers.control] != nil else { return }
        cancelRequested = true
        // A write-with-response must finish before control can be written.
        detail = "Cancellation queued after current acknowledged frame."
    }

    func applicationDidSuspend() {
        disconnect()
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
        arm(seconds: 8, token: generation, message: "Frame write timed out") { [weak self] in self?.fail("Frame write timed out") }
        guard beginOperation(.write(characteristic.uuid.uuidString), characteristic: characteristic) else { return }
        peripheral.writeValue(frame, for: characteristic, type: .withResponse)
    }

    private func sendControl(cancel: Bool) {
        guard let peripheral, let id = sessionID, let control = characteristics[GattIdentifiers.control] else { fail("Control characteristic unavailable"); return }
        let value: Data
        do { value = try GattWireCodec.control(sessionID: id, cancel: cancel) }
        catch { fail("Could not encode control command"); return }
        state = .awaitingResponse
        arm(seconds: 8, token: generation, message: "Control write timed out") { [weak self] in self?.fail("Control write timed out") }
        guard beginOperation(.write(control.uuid.uuidString), characteristic: control) else { return }
        peripheral.writeValue(value, for: control, type: .withResponse)
    }

    private func read(_ uuid: CBUUID) {
        guard let peripheral, let characteristic = characteristics[uuid] else { fail("Required characteristic missing"); return }
        expectedRead = uuid
        state = uuid == GattIdentifiers.version ? .discovering : (uuid == GattIdentifiers.challenge ? .readingChallenge : .awaitingResponse)
        arm(seconds: 8, token: generation, message: "Characteristic read timed out") { [weak self] in self?.fail("Characteristic read timed out") }
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
        arm(seconds: 8, token: generation, message: "P009 characteristic read timed out") { [weak self] in
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
                    createdAtMilliseconds: currentMilliseconds
                )
                authReadCursor = try WatchRequestReadCursor(
                    transferID: transferID,
                    owner: owner,
                    totalFrameCount: count,
                    nowMilliseconds: currentMilliseconds
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
                nowMilliseconds: currentMilliseconds
            )
            try cursor.didRead(owner: owner, transferID: transferID, index: index, nowMilliseconds: currentMilliseconds)
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
        try cursor.requested(owner: owner, transferID: transferID, index: index, nowMilliseconds: currentMilliseconds)
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
        arm(seconds: 8, token: generation, message: "P009 write-with-response timed out") { [weak self] in
            self?.fail("P009 write-with-response timed out")
        }
        guard beginOperation(.write(characteristic.uuid.uuidString), characteristic: characteristic) else { return }
        peripheral.writeValue(value, for: characteristic, type: .withResponse)
    }

    private func finishAuthRequest() throws {
        guard let transferID = authTransferID, let owner = authOwner,
              var assembler = authAssembler, let credentials = authCredentials else {
            throw WatchTransportError.incompleteTransfer
        }
        let envelope = try assembler.complete(owner: owner, transferID: transferID, nowMilliseconds: currentMilliseconds)
        state = .verifyingAuthRequest
        let verified = try WatchAuthProtocol.verifyHostTransportEnvelope(
            bytes: envelope.bytes,
            enrollment: credentials.hostEnrollment,
            expectedWatchEnrollmentID: credentials.watchEnrollment.enrollmentID,
            nowMilliseconds: currentMilliseconds,
            replayGuard: &authReplayGuard
        )
        try WatchAuthProtocol.validateWatchResponseEligibility(
            verifiedRequest: verified,
            enrollment: credentials.watchEnrollment,
            signingKey: credentials.signingKey
        )
        authAssembler = assembler
        authVerifiedRequest = verified
        consentPresentation = P009ConsentPresentation(
            verifiedRequest: verified,
            watchEnrollment: credentials.watchEnrollment,
            signingKey: credentials.signingKey
        )
        state = .awaitingResearchConsent
        detail = "P009 request signature, enrollment, action, UID, session, freshness, and replay checks passed. Awaiting an explicit choice."
        arm(seconds: 30, token: generation, message: "P009 research session timed out") { [weak self] in
            self?.fail("P009 research session timed out before a signed response")
        }
    }

    func completeAuthenticatedConsent(
        _ result: Result<SignedWatchResponse, ResearchConsentFailure>,
        presentationID: UUID
    ) {
        guard consentPresentation?.id == presentationID,
              state == .awaitingResearchConsent else { return }
        watchdog?.cancel()
        consentPresentation = nil
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
                let envelope = try WatchSignedEnvelope(
                    direction: .watchToHost,
                    canonical: signed.response.canonicalBytes(),
                    signatureRaw: signed.signatureRaw
                )
                authResponseFrames = try WatchAuthTransportCodec.frames(envelope: envelope, transferID: transferID, mtu: 23)
                authResponseFrameIndex = 0
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
        authVerifiedRequest = nil
        authPhase = nil
        consentPresentation = nil
        if !preserveAuthCredentials { authCredentials = nil }
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
        peripherals.append(ExperimentPeripheral(peripheral: peripheral, name: name))
    }

    func centralManager(_ central: CBCentralManager, didConnect peripheral: CBPeripheral) {
        guard central === self.central, state == .connecting, consume(.connect, peripheral: peripheral) else { return }
        watchdog?.cancel()
        state = .discovering
        arm(seconds: 12, token: generation, message: "Service discovery timed out") { [weak self] in self?.fail("Service discovery timed out") }
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
                protocolVersion = "1"
                read(GattIdentifiers.challenge)
            } else if characteristic.uuid == GattIdentifiers.challenge {
                let challenge = try GattWireCodec.parseChallenge(value)
                sessionID = challenge.sessionID
                state = .ready
                detail = "Fresh synthetic challenge read. Long-read completeness must be hardware verified."
            } else if characteristic.uuid == GattIdentifiers.response {
                guard let id = sessionID else { throw GattWireCodec.WireError.session }
                let cancelled = cancelRequested
                try GattWireCodec.verifyResponse(value, sessionID: id, payload: testPayload, cancelled: cancelled)
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
                    try cursor.acknowledge(owner: owner, transferID: transferID, index: index, nowMilliseconds: currentMilliseconds)
                    authReadCursor = cursor
                    if index + 1 < authFrameCount {
                        try requestAuthFrame(index + 1)
                    } else {
                        try cursor.complete(owner: owner, transferID: transferID, nowMilliseconds: currentMilliseconds)
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
                    clearProtocolState()
                    state = .completedResearchDecision
                    detail = "Signed research response sent with write-with-response. Linux verification and authorization were not inferred by the Watch."
                case .cancel:
                    clearProtocolState()
                    state = .cancelled
                    detail = "P009 research session cancelled. No signed approval was sent."
                }
            } catch {
                fail("P009 transport operation failed: \(error.localizedDescription)")
            }
            return
        }
        if characteristic.uuid == GattIdentifiers.exchange, state == .sendingFrames {
            frameIndex += 1
            let token = generation
            Task { @MainActor [weak self] in
                await Task.yield()
                guard let self, self.generation == token, self.state == .sendingFrames else { return }
                self.writeNextFrame(characteristic)
            }
        } else if characteristic.uuid == GattIdentifiers.control, state == .awaitingResponse {
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
