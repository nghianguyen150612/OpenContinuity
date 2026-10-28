import Foundation
import CoreBluetooth
import Combine
import OpenContinuityProtocol

struct ExperimentPeripheral: Identifiable {
    let peripheral: CBPeripheral
    let name: String
    var id: UUID { peripheral.identifier }
}

@MainActor
final class WatchBluetoothManager: NSObject, ObservableObject {
    @Published private(set) var bluetoothState = "Starting Bluetooth"
    @Published private(set) var state: GattResearchState = .idle
    @Published private(set) var peripherals: [ExperimentPeripheral] = []
    @Published private(set) var selectedName = "None"
    @Published private(set) var protocolVersion = "—"
    @Published private(set) var detail = "Synthetic transport only. No authentication or unlock."

    private var central: CBCentralManager!
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

    override init() {
        super.init()
        central = CBCentralManager(delegate: self, queue: .main)
    }

    func startScan() {
        guard central.state == .poweredOn else { fail("Bluetooth is not ready"); return }
        invalidateSession(keepPeripheral: false)
        peripherals.removeAll()
        state = .scanning
        generationGate.advance()
        let token = generation
        scanGeneration = token
        central.scanForPeripherals(withServices: [GattIdentifiers.service], options: [CBCentralManagerScanOptionAllowDuplicatesKey: false])
        arm(seconds: 15, token: token, message: "Scan timed out") { [weak self] in
            guard let self, self.scanGeneration == token else { return }
            self.central.stopScan()
            self.scanGeneration = nil
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
        state = .idle
    }

    func connect(_ item: ExperimentPeripheral) {
        guard central.state == .poweredOn, state == .scanning else { return }
        watchdog?.cancel()
        scanGeneration = nil
        central.stopScan()
        generationGate.advance()
        let token = generation
        peripheral = item.peripheral
        peripheral?.delegate = self
        selectedName = item.name
        state = .connecting
        central.connect(item.peripheral)
        arm(seconds: 15, token: token, message: "Connection timed out") { [weak self] in
            guard let self, self.generation == token else { return }
            self.central.cancelPeripheralConnection(item.peripheral)
            self.fail("Connection timed out")
        }
    }

    func disconnect() {
        central.stopScan()
        scanGeneration = nil
        guard let peripheral else { resetToIdle(); return }
        generationGate.advance()
        watchdog?.cancel()
        central.stopScan()
        central.cancelPeripheralConnection(peripheral)
        clearProtocolState()
        self.peripheral = nil
        state = .disconnected
        selectedName = "None"
        characteristics.removeAll()
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
        guard state != .idle && state != .disconnected && state != .completed && state != .cancelled else { return }
        disconnect()
        state = .cancelled
        detail = "Foreground session ended; session data discarded. Reconnect for a fresh challenge."
    }

    private func writeNextFrame(_ characteristic: CBCharacteristic) {
        guard let peripheral, generation > 0 else { fail("No active session"); return }
        if cancelRequested { sendControl(cancel: true); return }
        guard frameIndex < frames.count else { sendControl(cancel: false); return }
        let frame = frames[frameIndex]
        guard frame.count <= 20 else { fail("Frame exceeds conservative 20-byte write limit"); return }
        arm(seconds: 8, token: generation, message: "Frame write timed out") { [weak self] in self?.fail("Frame write timed out") }
        peripheral.writeValue(frame, for: characteristic, type: .withResponse)
    }

    private func sendControl(cancel: Bool) {
        guard let peripheral, let id = sessionID, let control = characteristics[GattIdentifiers.control] else { fail("Control characteristic unavailable"); return }
        let value: Data
        do { value = try GattWireCodec.control(sessionID: id, cancel: cancel) }
        catch { fail("Could not encode control command"); return }
        state = .awaitingResponse
        arm(seconds: 8, token: generation, message: "Control write timed out") { [weak self] in self?.fail("Control write timed out") }
        peripheral.writeValue(value, for: control, type: .withResponse)
    }

    private func read(_ uuid: CBUUID) {
        guard let peripheral, let characteristic = characteristics[uuid] else { fail("Required characteristic missing"); return }
        expectedRead = uuid
        state = uuid == GattIdentifiers.version ? .discovering : (uuid == GattIdentifiers.challenge ? .readingChallenge : .awaitingResponse)
        arm(seconds: 8, token: generation, message: "Characteristic read timed out") { [weak self] in self?.fail("Characteristic read timed out") }
        peripheral.readValue(for: characteristic)
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

    private func clearProtocolState() {
        watchdog?.cancel()
        watchdog = nil
        expectedRead = nil
        frames.removeAll()
        frameIndex = 0
        sessionID = nil
        cancelRequested = false
        protocolVersion = "—"
    }

    private func invalidateSession(keepPeripheral: Bool) {
        generationGate.advance()
        clearProtocolState()
        characteristics.removeAll()
        if !keepPeripheral { peripheral = nil; selectedName = "None" }
    }

    private func fail(_ reason: String) {
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
        guard state == .scanning, scanGeneration == generation, !peripherals.contains(where: { $0.id == peripheral.identifier }) else { return }
        let name = peripheral.name ?? (advertisementData[CBAdvertisementDataLocalNameKey] as? String) ?? "Unnamed Linux peripheral"
        peripherals.append(ExperimentPeripheral(peripheral: peripheral, name: name))
    }

    func centralManager(_ central: CBCentralManager, didConnect peripheral: CBPeripheral) {
        guard peripheral === self.peripheral, state == .connecting else { central.cancelPeripheralConnection(peripheral); return }
        watchdog?.cancel()
        state = .discovering
        arm(seconds: 12, token: generation, message: "Service discovery timed out") { [weak self] in self?.fail("Service discovery timed out") }
        peripheral.discoverServices([GattIdentifiers.service])
    }

    func centralManager(_ central: CBCentralManager, didFailToConnect peripheral: CBPeripheral, error: Error?) {
        guard peripheral === self.peripheral else { return }
        fail("Connection failed: \(error?.localizedDescription ?? "unknown error")")
    }

    func centralManager(_ central: CBCentralManager, didDisconnectPeripheral peripheral: CBPeripheral, error: Error?) {
        guard peripheral === self.peripheral else { return }
        generationGate.advance()
        clearProtocolState()
        self.peripheral = nil
        characteristics.removeAll()
        selectedName = "None"
        state = .disconnected
        detail = error.map { "Disconnected: \($0.localizedDescription)" } ?? "Disconnected; session discarded."
    }
}

extension WatchBluetoothManager: @preconcurrency CBPeripheralDelegate {
    func peripheral(_ peripheral: CBPeripheral, didDiscoverServices error: Error?) {
        guard peripheral === self.peripheral, state == .discovering else { return }
        guard error == nil, let service = peripheral.services?.first(where: { $0.uuid == GattIdentifiers.service }) else { fail("Experimental service missing or discovery failed"); return }
        peripheral.discoverCharacteristics([GattIdentifiers.version, GattIdentifiers.challenge, GattIdentifiers.exchange, GattIdentifiers.response, GattIdentifiers.control], for: service)
    }

    func peripheral(_ peripheral: CBPeripheral, didDiscoverCharacteristicsFor service: CBService, error: Error?) {
        guard peripheral === self.peripheral, state == .discovering, service.uuid == GattIdentifiers.service else { return }
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
        guard peripheral === self.peripheral, let expectedRead, characteristic.uuid == expectedRead else { return }
        watchdog?.cancel()
        self.expectedRead = nil
        guard error == nil, let value = characteristic.value else { fail("Characteristic read failed"); return }
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
        guard peripheral === self.peripheral else { return }
        watchdog?.cancel()
        guard error == nil else { fail("Write failed: \(error!.localizedDescription)"); return }
        if characteristic.uuid == GattIdentifiers.exchange, state == .sendingFrames {
            frameIndex += 1
            if cancelRequested { sendControl(cancel: true) }
            else { writeNextFrame(characteristic) }
        } else if characteristic.uuid == GattIdentifiers.control, state == .awaitingResponse {
            read(GattIdentifiers.response)
        } else { fail("Unexpected write completion") }
    }

    func peripheral(_ peripheral: CBPeripheral, didModifyServices invalidatedServices: [CBService]) {
        guard peripheral === self.peripheral, invalidatedServices.contains(where: { $0.uuid == GattIdentifiers.service }) else { return }
        fail("Experimental service invalidated")
    }

    private func clearProtocolStatePreservingResult() {
        watchdog?.cancel(); watchdog = nil
        sessionID = nil; frames.removeAll(); expectedRead = nil; cancelRequested = false
    }
}
