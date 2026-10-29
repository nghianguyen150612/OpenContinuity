import Foundation

/// P009 transport. This is separate from GattWireCodec and P007 GATT v1.
public enum WatchAuthTransport {
    public static let protocolName = "OC-WATCH-AUTH-TRANSPORT/1"
    public static let version: UInt8 = 1
    public static let service = "9a7f0001-6f4e-4c4f-9b4f-4f70656e436f"
    public static let requestData = "9a7f0002-6f4e-4c4f-9b4f-4f70656e436f"
    public static let requestControl = "9a7f0003-6f4e-4c4f-9b4f-4f70656e436f"
    public static let responseData = "9a7f0004-6f4e-4c4f-9b4f-4f70656e436f"
    public static let responseControl = "9a7f0005-6f4e-4c4f-9b4f-4f70656e436f"
    public static let status = "9a7f0006-6f4e-4c4f-9b4f-4f70656e436f"
    public static let maximumCanonicalBytes = 1_024
    public static let signatureBytes = 64
    public static let maximumEnvelopeBytes = maximumCanonicalBytes + signatureBytes
    public static let frameHeaderBytes = 15
    public static let minimumATTMTU = 23
    public static let maximumATTMTU = 517
    public static let maximumFrameCount = 218
    public static let transferTimeoutMilliseconds: UInt64 = 30_000
    fileprivate static let magic: [UInt8] = [0x4f, 0x41]
    fileprivate static let startFlag: UInt8 = 0x01
    fileprivate static let endFlag: UInt8 = 0x02
}

public enum WatchTransportDirection: Equatable {
    case hostToWatch
    case watchToHost
}

public enum WatchTransportError: Error, Equatable {
    case invalidATTMTU, frameTooLarge, malformed, versionMismatch, directionMismatch
    case sessionMismatch, ownerMismatch, duplicateOrOutOfOrder, startRequired
    case messageInProgress, messageTooLarge, invalidEnvelope, lengthMismatch
    case transferExpired, incompleteTransfer, alreadyCompleted, duplicateCompletion
    case cancelled, invalidChunkCommand
}

public struct WatchSignedEnvelope: Equatable {
    public let direction: WatchTransportDirection
    public let canonical: Data
    public let signatureRaw: Data

    public init(direction: WatchTransportDirection, canonical: Data, signatureRaw: Data) throws {
        guard !canonical.isEmpty, canonical.count <= WatchAuthTransport.maximumCanonicalBytes else {
            throw WatchTransportError.messageTooLarge
        }
        guard signatureRaw.count == WatchAuthTransport.signatureBytes else {
            throw WatchTransportError.invalidEnvelope
        }
        do {
            switch direction {
            case .hostToWatch: _ = try WatchAuthRequest.parseCanonical(canonical)
            case .watchToHost: _ = try WatchAuthResponse.parseCanonical(canonical)
            }
        } catch {
            throw WatchTransportError.invalidEnvelope
        }
        self.direction = direction
        self.canonical = canonical
        self.signatureRaw = signatureRaw
    }

    public init(direction: WatchTransportDirection, bytes: Data) throws {
        guard bytes.count > WatchAuthTransport.signatureBytes,
              bytes.count <= WatchAuthTransport.maximumEnvelopeBytes else {
            throw WatchTransportError.invalidEnvelope
        }
        let split = bytes.count - WatchAuthTransport.signatureBytes
        try self.init(direction: direction, canonical: Data(bytes.prefix(split)), signatureRaw: Data(bytes.suffix(WatchAuthTransport.signatureBytes)))
    }

    public var bytes: Data {
        canonical + signatureRaw
    }
}

public enum WatchAuthTransportCodec {
    public static func frames(
        envelope: WatchSignedEnvelope,
        transferID: Data,
        mtu: Int = WatchAuthTransport.minimumATTMTU
    ) throws -> [Data] {
        guard transferID.count == 8 else { throw WatchTransportError.sessionMismatch }
        let payloadLimit = try framePayloadLimit(mtu: mtu)
        let bytes = Array(envelope.bytes)
        let count = (bytes.count + payloadLimit - 1) / payloadLimit
        guard count <= WatchAuthTransport.maximumFrameCount, count <= 256 else {
            throw WatchTransportError.messageTooLarge
        }
        return (0..<count).map { index in
            let start = index == 0
            let end = index + 1 == count
            let low = index * payloadLimit
            let high = min(low + payloadLimit, bytes.count)
            var frame = WatchAuthTransport.magic
            frame.append(WatchAuthTransport.version)
            frame.append(contentsOf: transferID)
            frame.append(UInt8(index))
            frame.append((start ? WatchAuthTransport.startFlag : 0) | (end ? WatchAuthTransport.endFlag : 0))
            let totalLength = start ? bytes.count : 0
            frame.append(UInt8(totalLength & 0xff))
            frame.append(UInt8((totalLength >> 8) & 0xff))
            frame.append(contentsOf: bytes[low..<high])
            return Data(frame)
        }
    }

    public static func requestCommand(transferID: Data, command: WatchRequestCommand) throws -> Data {
        guard transferID.count == 8 else { throw WatchTransportError.sessionMismatch }
        let (opcode, index): (UInt8, UInt8)
        switch command {
        case .get(let value): (opcode, index) = (1, value)
        case .acknowledge(let value): (opcode, index) = (2, value)
        case .complete: (opcode, index) = (3, 0)
        case .cancel: (opcode, index) = (4, 0)
        }
        return Data(WatchAuthTransport.magic + [WatchAuthTransport.version, opcode] + Array(transferID) + [index])
    }

    public static func parseRequestCommand(_ data: Data) throws -> (Data, WatchRequestCommand) {
        let bytes = Array(data)
        guard bytes.count == 13, Array(bytes[0..<2]) == WatchAuthTransport.magic,
              bytes[2] == WatchAuthTransport.version else {
            throw WatchTransportError.invalidChunkCommand
        }
        let transferID = Data(bytes[4..<12])
        switch (bytes[3], bytes[12]) {
        case (1, let index): return (transferID, .get(index))
        case (2, let index): return (transferID, .acknowledge(index))
        case (3, 0): return (transferID, .complete)
        case (4, 0): return (transferID, .cancel)
        default: throw WatchTransportError.invalidChunkCommand
        }
    }

    fileprivate static func framePayloadLimit(mtu: Int) throws -> Int {
        guard (WatchAuthTransport.minimumATTMTU...WatchAuthTransport.maximumATTMTU).contains(mtu) else {
            throw WatchTransportError.invalidATTMTU
        }
        let valueLimit = mtu - 3
        guard valueLimit > WatchAuthTransport.frameHeaderBytes else {
            throw WatchTransportError.frameTooLarge
        }
        return valueLimit - WatchAuthTransport.frameHeaderBytes
    }
}

public enum WatchRequestCommand: Equatable {
    case get(UInt8)
    case acknowledge(UInt8)
    case complete
    case cancel
}

/// Sequential frame receiver bound to a single BLE connection identity.
public struct WatchTransferAssembler {
    private let transferID: Data
    private let direction: WatchTransportDirection
    private let owner: String
    private var expectedSequence = 0
    private var totalLength: Int?
    private var bytes = [UInt8]()
    private let createdAtMilliseconds: UInt64
    private var byteComplete = false
    private var committed = false
    private var cancelled = false

    public init(transferID: Data, direction: WatchTransportDirection, owner: String, createdAtMilliseconds: UInt64) throws {
        guard transferID.count == 8, !owner.isEmpty else { throw WatchTransportError.sessionMismatch }
        self.transferID = transferID
        self.direction = direction
        self.owner = owner
        self.createdAtMilliseconds = createdAtMilliseconds
    }

    public mutating func push(
        owner: String,
        direction: WatchTransportDirection,
        frame: Data,
        mtu: Int = WatchAuthTransport.minimumATTMTU,
        nowMilliseconds: UInt64
    ) throws {
        guard owner == self.owner else { throw WatchTransportError.ownerMismatch }
        guard direction == self.direction else { throw WatchTransportError.directionMismatch }
        try checkLive(nowMilliseconds: nowMilliseconds)
        let valueLimit = mtu - 3
        guard (WatchAuthTransport.minimumATTMTU...WatchAuthTransport.maximumATTMTU).contains(mtu) else {
            throw WatchTransportError.invalidATTMTU
        }
        guard frame.count <= valueLimit else { throw WatchTransportError.frameTooLarge }
        let b = Array(frame)
        guard b.count > WatchAuthTransport.frameHeaderBytes,
              Array(b[0..<2]) == WatchAuthTransport.magic else { throw WatchTransportError.malformed }
        guard b[2] == WatchAuthTransport.version else { throw WatchTransportError.versionMismatch }
        guard Data(b[3..<11]) == transferID else { throw WatchTransportError.sessionMismatch }
        guard !byteComplete, !committed else { throw WatchTransportError.alreadyCompleted }
        let sequence = Int(b[11])
        let flags = b[12]
        let start = flags & WatchAuthTransport.startFlag != 0
        let end = flags & WatchAuthTransport.endFlag != 0
        let totalField = Int(UInt16(b[13]) | (UInt16(b[14]) << 8))
        let payload = Array(b.dropFirst(WatchAuthTransport.frameHeaderBytes))
        guard flags & ~(WatchAuthTransport.startFlag | WatchAuthTransport.endFlag) == 0, !payload.isEmpty else {
            throw WatchTransportError.malformed
        }
        if start {
            guard totalLength == nil else { throw WatchTransportError.messageInProgress }
            guard sequence == 0 else { throw WatchTransportError.duplicateOrOutOfOrder }
            guard (WatchAuthTransport.signatureBytes + 1...WatchAuthTransport.maximumEnvelopeBytes).contains(totalField) else {
                throw WatchTransportError.messageTooLarge
            }
            guard payload.count <= totalField else { throw WatchTransportError.lengthMismatch }
            totalLength = totalField
        } else {
            guard totalLength != nil else { throw WatchTransportError.startRequired }
            guard totalField == 0 else { throw WatchTransportError.malformed }
        }
        guard sequence == expectedSequence else { throw WatchTransportError.duplicateOrOutOfOrder }
        guard let totalLength else { throw WatchTransportError.startRequired }
        guard bytes.count + payload.count <= totalLength else { throw WatchTransportError.lengthMismatch }
        let reachedTotal = bytes.count + payload.count == totalLength
        guard end == reachedTotal else { throw WatchTransportError.lengthMismatch }
        bytes.append(contentsOf: payload)
        expectedSequence += 1
        if reachedTotal {
            _ = try WatchSignedEnvelope(direction: direction, bytes: Data(bytes))
            byteComplete = true
        }
    }

    public mutating func complete(owner: String, transferID: Data, nowMilliseconds: UInt64) throws -> WatchSignedEnvelope {
        guard owner == self.owner else { throw WatchTransportError.ownerMismatch }
        try checkLive(nowMilliseconds: nowMilliseconds)
        guard transferID == self.transferID else { throw WatchTransportError.sessionMismatch }
        guard byteComplete else { throw WatchTransportError.incompleteTransfer }
        committed = true
        return try WatchSignedEnvelope(direction: direction, bytes: Data(bytes))
    }

    public mutating func cancel(owner: String, transferID: Data) throws {
        guard owner == self.owner else { throw WatchTransportError.ownerMismatch }
        guard transferID == self.transferID else { throw WatchTransportError.sessionMismatch }
        guard !committed else { throw WatchTransportError.alreadyCompleted }
        cancelled = true
        bytes.removeAll(keepingCapacity: false)
    }

    public var isByteComplete: Bool { byteComplete }

    private func checkLive(nowMilliseconds: UInt64) throws {
        if cancelled { throw WatchTransportError.cancelled }
        if committed { throw WatchTransportError.alreadyCompleted }
        if nowMilliseconds >= createdAtMilliseconds,
           nowMilliseconds - createdAtMilliseconds > WatchAuthTransport.transferTimeoutMilliseconds {
            throw WatchTransportError.transferExpired
        }
    }
}

/// Watch-side GET/read/ACK driver model. Only a completed write-with-response
/// for ACK advances the requested index; reading data never advances it.
public struct WatchRequestReadCursor {
    private let transferID: Data
    private let owner: String
    private var nextIndex: Int = 0
    private var requestedIndex: Int?
    private var readCompleted = false
    private let startedAtMilliseconds: UInt64
    private let totalFrameCount: Int
    private var completed = false
    private var cancelled = false

    public init(transferID: Data, owner: String, totalFrameCount: Int, nowMilliseconds: UInt64) throws {
        guard transferID.count == 8, !owner.isEmpty,
              (1...WatchAuthTransport.maximumFrameCount).contains(totalFrameCount) else {
            throw WatchTransportError.sessionMismatch
        }
        self.transferID = transferID
        self.owner = owner
        self.totalFrameCount = totalFrameCount
        self.startedAtMilliseconds = nowMilliseconds
    }

    public mutating func requested(owner: String, transferID: Data, index: Int, nowMilliseconds: UInt64) throws {
        try check(owner: owner, transferID: transferID, nowMilliseconds: nowMilliseconds)
        guard index == nextIndex, index < WatchAuthTransport.maximumFrameCount else {
            throw WatchTransportError.duplicateOrOutOfOrder
        }
        if let requestedIndex, requestedIndex != index { throw WatchTransportError.duplicateOrOutOfOrder }
        requestedIndex = index
    }

    public mutating func didRead(owner: String, transferID: Data, index: Int, nowMilliseconds: UInt64) throws {
        try check(owner: owner, transferID: transferID, nowMilliseconds: nowMilliseconds)
        guard requestedIndex == index, nextIndex == index else { throw WatchTransportError.duplicateOrOutOfOrder }
        readCompleted = true
    }

    public mutating func acknowledge(owner: String, transferID: Data, index: Int, nowMilliseconds: UInt64) throws {
        try check(owner: owner, transferID: transferID, nowMilliseconds: nowMilliseconds)
        guard requestedIndex == index, nextIndex == index, readCompleted else { throw WatchTransportError.duplicateOrOutOfOrder }
        nextIndex += 1
        requestedIndex = nil
        readCompleted = false
    }

    public mutating func complete(owner: String, transferID: Data, nowMilliseconds: UInt64) throws {
        try check(owner: owner, transferID: transferID, nowMilliseconds: nowMilliseconds)
        guard requestedIndex == nil, nextIndex == totalFrameCount else { throw WatchTransportError.incompleteTransfer }
        completed = true
    }

    public mutating func cancel(owner: String, transferID: Data) throws {
        guard owner == self.owner else { throw WatchTransportError.ownerMismatch }
        guard transferID == self.transferID else { throw WatchTransportError.sessionMismatch }
        guard !completed else { throw WatchTransportError.alreadyCompleted }
        cancelled = true
    }

    public var index: Int { nextIndex }
    public var sessionID: Data { transferID }

    private func check(owner: String, transferID: Data, nowMilliseconds: UInt64) throws {
        guard owner == self.owner else { throw WatchTransportError.ownerMismatch }
        guard transferID == self.transferID else { throw WatchTransportError.sessionMismatch }
        if cancelled { throw WatchTransportError.cancelled }
        if completed { throw WatchTransportError.alreadyCompleted }
        if nowMilliseconds >= startedAtMilliseconds,
           nowMilliseconds - startedAtMilliseconds > WatchAuthTransport.transferTimeoutMilliseconds {
            throw WatchTransportError.transferExpired
        }
    }
}
