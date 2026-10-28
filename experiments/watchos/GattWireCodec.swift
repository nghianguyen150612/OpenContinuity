import Foundation
import CryptoKit

public enum GattUUIDs {
    public static let service = "8e6f0001-6f4e-4c4f-9b4f-4f70656e436f"
    public static let version = "8e6f0002-6f4e-4c4f-9b4f-4f70656e436f"
    public static let challenge = "8e6f0003-6f4e-4c4f-9b4f-4f70656e436f"
    public static let exchange = "8e6f0004-6f4e-4c4f-9b4f-4f70656e436f"
    public static let response = "8e6f0005-6f4e-4c4f-9b4f-4f70656e436f"
    public static let control = "8e6f0006-6f4e-4c4f-9b4f-4f70656e436f"
}

/// P006 synthetic transport only. This does not authenticate a Watch or host.
public enum GattWireCodec {
    public static let version: UInt8 = 1
    public static let maximumMessageLength = 1024
    public static let framePayloadLength = 5

    public enum WireError: Error, Equatable, LocalizedError {
        case malformed, version, session, length, status, digest, server(UInt8)

        public var errorDescription: String? {
            switch self {
            case .malformed: return "Malformed protocol value"
            case .version: return "Unsupported protocol version"
            case .session: return "Session identifier mismatch"
            case .length: return "Protocol value has an invalid length"
            case .status: return "Server returned an unexpected status"
            case .digest: return "Synthetic response digest mismatch"
            case .server(let code):
                let descriptions = [1: "Invalid ATT MTU", 2: "Frame too large", 3: "Malformed frame",
                                    4: "Unsupported server protocol version", 5: "Session mismatch",
                                    6: "Duplicate or out-of-order frame", 7: "Start frame required",
                                    8: "Message already in progress", 9: "Message exceeds size limit",
                                    10: "Message length mismatch", 11: "Partial message expired",
                                    12: "Message already complete"]
                return descriptions[code] ?? "Unknown server error code \(code)"
            }
        }
    }

    public struct Challenge {
        public let sessionID: [UInt8]
        public let challenge: [UInt8]
    }

    public struct Frame {
        public let sequence: UInt8
        public let start: Bool
        public let end: Bool
        public let totalLength: Int
        public let payload: Data
    }

    public static func parseChallenge(_ data: Data) throws -> Challenge {
        let bytes = Array(data)
        guard bytes.count == 25 else { throw WireError.length }
        guard bytes[0] == version else { throw WireError.version }
        return Challenge(sessionID: Array(bytes[1..<9]), challenge: Array(bytes[9..<25]))
    }

    public static func frames(payload: Data, sessionID: [UInt8]) throws -> [Data] {
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
            var frame: [UInt8] = [0x4f, 0x43, version]
            frame.append(contentsOf: sessionID)
            frame.append(contentsOf: [UInt8(index), flags, UInt8(length & 255), UInt8(length >> 8)])
            frame.append(contentsOf: chunk)
            return Data(frame)
        }
    }

    public static func control(sessionID: [UInt8], cancel: Bool) throws -> Data {
        guard sessionID.count == 8 else { throw WireError.session }
        return Data([0x4f, 0x43, version] + sessionID + [cancel ? 2 : 1])
    }

    public static func validateFrame(_ data: Data, sessionID: [UInt8]) throws -> Frame {
        let b = Array(data)
        guard b.count >= 16, b.count <= 20, b[0] == 0x4f, b[1] == 0x43 else { throw WireError.malformed }
        guard b[2] == version else { throw WireError.version }
        guard sessionID.count == 8, Array(b[3..<11]) == sessionID else { throw WireError.session }
        let flags = b[12]
        guard flags & 0xfc == 0 else { throw WireError.malformed }
        let start = flags & 1 != 0
        let end = flags & 2 != 0
        let total = Int(b[13]) | (Int(b[14]) << 8)
        let payload = Data(b[15...])
        guard !payload.isEmpty else { throw WireError.malformed }
        if start {
            guard b[11] == 0, (1...maximumMessageLength).contains(total) else { throw WireError.length }
        } else {
            guard total == 0 else { throw WireError.malformed }
        }
        return Frame(sequence: b[11], start: start, end: end, totalLength: total, payload: payload)
    }

    public static func verifyResponse(_ data: Data, sessionID: [UInt8], payload: Data, cancelled: Bool) throws {
        let bytes = Array(data)
        guard bytes.count == 44 || bytes.count == 11 else { throw WireError.length }
        guard bytes[0] == version else { throw WireError.version }
        guard sessionID.count == 8, Array(bytes[2..<10]) == sessionID else { throw WireError.session }
        if bytes.count == 11 {
            guard bytes[1] == 0x80 else { throw WireError.status }
            throw WireError.server(bytes[10])
        }
        // P006 status_response(0, submitted_bytes) acknowledges completion.
        guard bytes[1] == (cancelled ? 2 : 0) else { throw WireError.status }
        let expected = cancelled ? Data("cancelled".utf8) : payload
        let length = Int(bytes[10]) | (Int(bytes[11]) << 8)
        guard length == expected.count else { throw WireError.length }
        guard Array(bytes[12..<44]) == Array(SHA256.hash(data: expected)) else { throw WireError.digest }
    }

    public static func parseVersion(_ data: Data) throws {
        guard data.count == 1 else { throw WireError.length }
        guard data[ data.startIndex ] == version else { throw WireError.version }
    }
}

/// Small pure token gate used by asynchronous BLE work to reject old sessions.
public struct GattGenerationGate: Sendable {
    public private(set) var current: UInt64 = 0

    public init() {}

    @discardableResult
    public mutating func advance() -> UInt64 {
        current &+= 1
        return current
    }

    public func accepts(_ token: UInt64) -> Bool { token == current }
}
