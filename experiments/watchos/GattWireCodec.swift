import Foundation
import CryptoKit

/// P006 synthetic transport only. This does not authenticate a Watch or host.
enum GattWireCodec {
    static let version: UInt8 = 1
    static let maximumMessageLength = 1024
    static let framePayloadLength = 5

    enum WireError: Error, Equatable {
        case malformed, version, session, length, status, digest, server(UInt8)
    }

    struct Challenge {
        let sessionID: [UInt8]
        let challenge: [UInt8]
    }

    static func parseChallenge(_ data: Data) throws -> Challenge {
        let bytes = Array(data)
        guard bytes.count == 25 else { throw WireError.length }
        guard bytes[0] == version else { throw WireError.version }
        return Challenge(sessionID: Array(bytes[1..<9]), challenge: Array(bytes[9..<25]))
    }

    static func frames(payload: Data, sessionID: [UInt8]) throws -> [Data] {
        let bytes = Array(payload)
        guard sessionID.count == 8 else { throw WireError.session }
        guard !bytes.isEmpty, bytes.count <= maximumMessageLength else { throw WireError.length }
        let count = (bytes.count + framePayloadLength - 1) / framePayloadLength
        guard count <= 256 else { throw WireError.length }
        return (0..<count).map { index in
            let start = index == 0
            let end = index == count - 1
            let length = start ? bytes.count : 0
            let flags: UInt8 = (start ? 1 : 0) | (end ? 2 : 0)
            let offset = index * framePayloadLength
            let chunk = bytes[offset..<min(offset + framePayloadLength, bytes.count)]
            return Data([0x4f, 0x43, version] + sessionID + [UInt8(index), flags, UInt8(length & 255), UInt8(length >> 8)] + chunk)
        }
    }

    static func control(sessionID: [UInt8], cancel: Bool) throws -> Data {
        guard sessionID.count == 8 else { throw WireError.session }
        return Data([0x4f, 0x43, version] + sessionID + [cancel ? 2 : 1])
    }

    static func verifyResponse(_ data: Data, sessionID: [UInt8], payload: Data, cancelled: Bool) throws {
        let bytes = Array(data)
        guard bytes.count == 44 || bytes.count == 11 else { throw WireError.length }
        guard bytes[0] == version else { throw WireError.version }
        guard sessionID.count == 8, Array(bytes[2..<10]) == sessionID else { throw WireError.session }
        if bytes.count == 11 {
            guard bytes[1] == 0x80 else { throw WireError.status }
            throw WireError.server(bytes[10])
        }
        guard bytes[1] == (cancelled ? 2 : 1) else { throw WireError.status }
        let expected = cancelled ? Data("cancelled".utf8) : payload
        let length = Int(bytes[10]) | (Int(bytes[11]) << 8)
        guard length == expected.count else { throw WireError.length }
        guard Array(bytes[12..<44]) == Array(SHA256.hash(data: expected)) else { throw WireError.digest }
    }
}
