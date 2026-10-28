import CoreBluetooth
import OpenContinuityProtocol

enum GattIdentifiers {
    static let service = CBUUID(string: GattUUIDs.service)
    static let version = CBUUID(string: GattUUIDs.version)
    static let challenge = CBUUID(string: GattUUIDs.challenge)
    static let exchange = CBUUID(string: GattUUIDs.exchange)
    static let response = CBUUID(string: GattUUIDs.response)
    static let control = CBUUID(string: GattUUIDs.control)
}
