import Foundation
import CryptoKit

public let watchAuthProtocolName = "OC-WATCH-AUTH-RESEARCH/1"
public let watchAuthAction = "research_confirmation"
public let watchAuthMaximumMessageLength = 1024
public let watchAuthMaximumIdentifierLength = 128
public let watchAuthMaximumLifetimeMilliseconds: UInt64 = 60_000
private let maximumReplayEntries = 4096

private let requestDomain = Data("OC-WATCH-AUTH-RESEARCH/1\0HOST_REQUEST\0".utf8)
private let responseDomain = Data("OC-WATCH-AUTH-RESEARCH/1\0WATCH_RESPONSE\0".utf8)
private let requestKind: UInt8 = 1
private let responseKind: UInt8 = 2
private let algorithmID: UInt8 = 1

public enum WatchAuthError: Error, Equatable {
    case malformed
    case oversized
    case invalidUTF8
    case invalidIdentifier
    case nonCanonical
    case unsupportedVersion
    case unsupportedAlgorithm
    case wrongMessageType
    case invalidPublicKey
    case invalidSignature
    case unknownEnrollment
    case enrollmentRevoked
    case enrollmentInactive
    case nonTestEnrollment
    case bindingMismatch
    case wrongAction
    case invalidTimestamp
    case notYetValid
    case expired
    case lifetimeTooLong
    case replay
    case replayStoreFull
    case invalidDecision
    case consentNotCompleted
}

public enum WatchEnrollmentState: String, Codable {
    case active
    case suspended
    case revoked
}

public enum WatchKeyAlgorithm: String, Codable {
    case p256EcdsaSha256 = "p256_ecdsa_sha256"
    case ed25519
    case p384EcdsaSha384
}

public struct HostEnrollment {
    public let enrollmentID: String
    public let hostIdentity: String
    public let linuxUID: UInt32
    public let publicKeySEC1: Data
    public let algorithm: WatchKeyAlgorithm
    public let keyVersion: UInt8
    public let state: WatchEnrollmentState
    public let testOnly: Bool

    public init(
        enrollmentID: String,
        hostIdentity: String,
        linuxUID: UInt32,
        publicKeySEC1: Data,
        algorithm: WatchKeyAlgorithm = .p256EcdsaSha256,
        keyVersion: UInt8 = 1,
        state: WatchEnrollmentState = .active,
        testOnly: Bool
    ) {
        self.enrollmentID = enrollmentID
        self.hostIdentity = hostIdentity
        self.linuxUID = linuxUID
        self.publicKeySEC1 = publicKeySEC1
        self.algorithm = algorithm
        self.keyVersion = keyVersion
        self.state = state
        self.testOnly = testOnly
    }
}

public struct WatchEnrollment {
    public let enrollmentID: String
    public let watchIdentity: String
    public let boundHostEnrollmentID: String
    public let boundHostIdentity: String
    public let boundLinuxUID: UInt32
    public let publicKeySEC1: Data
    public let algorithm: WatchKeyAlgorithm
    public let keyVersion: UInt8
    public let state: WatchEnrollmentState
    public let testOnly: Bool

    public init(
        enrollmentID: String,
        watchIdentity: String,
        boundHostEnrollmentID: String,
        boundHostIdentity: String,
        boundLinuxUID: UInt32,
        publicKeySEC1: Data,
        algorithm: WatchKeyAlgorithm = .p256EcdsaSha256,
        keyVersion: UInt8 = 1,
        state: WatchEnrollmentState = .active,
        testOnly: Bool
    ) {
        self.enrollmentID = enrollmentID
        self.watchIdentity = watchIdentity
        self.boundHostEnrollmentID = boundHostEnrollmentID
        self.boundHostIdentity = boundHostIdentity
        self.boundLinuxUID = boundLinuxUID
        self.publicKeySEC1 = publicKeySEC1
        self.algorithm = algorithm
        self.keyVersion = keyVersion
        self.state = state
        self.testOnly = testOnly
    }
}

public struct WatchAuthRequest: Equatable {
    public let hostEnrollmentID: String
    public let hostIdentity: String
    public let linuxUID: UInt32
    public let targetWatchEnrollmentID: String
    public let sessionID: Data
    public let nonce: Data
    public let issuedAtMilliseconds: UInt64
    public let expiresAtMilliseconds: UInt64
    public let action: String

    public func canonicalBytes() throws -> Data {
        try encodeRequest(self)
    }

    public static func parseCanonical(_ bytes: Data) throws -> WatchAuthRequest {
        guard bytes.count <= watchAuthMaximumMessageLength else { throw WatchAuthError.oversized }
        var reader = CanonicalReader(bytes)
        try reader.readHeader(expectedKind: requestKind)
        let request = WatchAuthRequest(
            hostEnrollmentID: try reader.readIdentifier(),
            hostIdentity: try reader.readIdentifier(),
            linuxUID: try reader.readUInt32(),
            targetWatchEnrollmentID: try reader.readIdentifier(),
            sessionID: Data(try reader.readBytes(count: 16)),
            nonce: Data(try reader.readBytes(count: 32)),
            issuedAtMilliseconds: try reader.readUInt64(),
            expiresAtMilliseconds: try reader.readUInt64(),
            action: try reader.readIdentifier()
        )
        try reader.finish()
        guard try encodeRequest(request) == bytes else { throw WatchAuthError.nonCanonical }
        return request
    }
}

public enum WatchAuthDecision: UInt8, Equatable {
    case approved = 1
    case rejected = 2
}

public enum WatchConsentResult: UInt8, Equatable {
    case explicitUserDecision = 1
    case cancelled = 2
    case unsupported = 3
    case localAuthenticationDenied = 4
}

public struct WatchAuthResponse: Equatable {
    public let requestDigest: Data
    public let nonce: Data
    public let hostEnrollmentID: String
    public let hostIdentity: String
    public let linuxUID: UInt32
    public let targetWatchEnrollmentID: String
    public let watchIdentity: String
    public let sessionID: Data
    public let action: String
    public let decision: WatchAuthDecision
    public let responseAtMilliseconds: UInt64
    public let consentResult: WatchConsentResult

    public func canonicalBytes() throws -> Data {
        try encodeResponse(self)
    }

    public static func parseCanonical(_ bytes: Data) throws -> WatchAuthResponse {
        guard bytes.count <= watchAuthMaximumMessageLength else { throw WatchAuthError.oversized }
        var reader = CanonicalReader(bytes)
        try reader.readHeader(expectedKind: responseKind)
        let requestDigest = Data(try reader.readBytes(count: 32))
        let nonce = Data(try reader.readBytes(count: 32))
        let hostEnrollmentID = try reader.readIdentifier()
        let hostIdentity = try reader.readIdentifier()
        let linuxUID = try reader.readUInt32()
        let targetWatchEnrollmentID = try reader.readIdentifier()
        let watchIdentity = try reader.readIdentifier()
        let sessionID = Data(try reader.readBytes(count: 16))
        let action = try reader.readIdentifier()
        guard let decision = WatchAuthDecision(rawValue: try reader.readUInt8()) else {
            throw WatchAuthError.invalidDecision
        }
        let responseAtMilliseconds = try reader.readUInt64()
        guard let consentResult = WatchConsentResult(rawValue: try reader.readUInt8()) else {
            throw WatchAuthError.consentNotCompleted
        }
        try reader.finish()
        let response = WatchAuthResponse(
            requestDigest: requestDigest,
            nonce: nonce,
            hostEnrollmentID: hostEnrollmentID,
            hostIdentity: hostIdentity,
            linuxUID: linuxUID,
            targetWatchEnrollmentID: targetWatchEnrollmentID,
            watchIdentity: watchIdentity,
            sessionID: sessionID,
            action: action,
            decision: decision,
            responseAtMilliseconds: responseAtMilliseconds,
            consentResult: consentResult
        )
        guard try encodeResponse(response) == bytes else { throw WatchAuthError.nonCanonical }
        return response
    }
}

public struct SignedWatchResponse {
    public let response: WatchAuthResponse
    public let signatureRaw: Data
}

/// A successful host verification token. Its initializer is private so the UI
/// component can only accept a request after pinned-key signature and binding
/// checks have completed.
public struct VerifiedHostRequest {
    public let request: WatchAuthRequest
    private let canonicalRequest: Data

    fileprivate init(request: WatchAuthRequest, canonicalRequest: Data) {
        self.request = request
        self.canonicalRequest = canonicalRequest
    }

    public var digest: Data {
        Data(SHA256.hash(data: canonicalRequest))
    }
}

/// Test-only in-memory replay tracking. Runtime integrations need durable,
/// crash-resistant storage before they can claim replay protection.
public struct WatchAuthReplayGuard {
    private var nonces = Set<Data>()
    private var sessions = Set<Data>()

    public init() {}

    fileprivate mutating func remember(nonce: Data, sessionID: Data) throws {
        guard !nonces.contains(nonce), !sessions.contains(sessionID) else {
            throw WatchAuthError.replay
        }
        guard nonces.count < maximumReplayEntries, sessions.count < maximumReplayEntries else {
            throw WatchAuthError.replayStoreFull
        }
        nonces.insert(nonce)
        sessions.insert(sessionID)
    }
}

public enum WatchAuthProtocol {
    /// Parse the P009 request envelope and only return a consent-capable token
    /// after P008 canonical, pin, action, binding, time, signature and replay
    /// checks have passed. BLE advertisement identity is never used here.
    public static func verifyHostTransportEnvelope(
        bytes: Data,
        enrollment: HostEnrollment?,
        expectedWatchEnrollmentID: String,
        nowMilliseconds: UInt64,
        replayGuard: inout WatchAuthReplayGuard
    ) throws -> VerifiedHostRequest {
        let envelope = try WatchSignedEnvelope(direction: .hostToWatch, bytes: bytes)
        return try verifyHostRequest(
            canonicalBytes: envelope.canonical,
            signatureRaw: envelope.signatureRaw,
            enrollment: enrollment,
            expectedWatchEnrollmentID: expectedWatchEnrollmentID,
            nowMilliseconds: nowMilliseconds,
            replayGuard: &replayGuard
        )
    }

    /// Reassemble first, then call this with the exact P009 response envelope.
    /// The typed result retains a signed rejection as a rejection.
    public static func verifyWatchTransportEnvelope(
        bytes: Data,
        enrollment: WatchEnrollment?,
        verifiedRequest: VerifiedHostRequest,
        nowMilliseconds: UInt64,
        replayGuard: inout WatchAuthReplayGuard
    ) throws -> WatchAuthResponse {
        let envelope = try WatchSignedEnvelope(direction: .watchToHost, bytes: bytes)
        return try verifyWatchResponse(
            canonicalBytes: envelope.canonical,
            signatureRaw: envelope.signatureRaw,
            enrollment: enrollment,
            verifiedRequest: verifiedRequest,
            nowMilliseconds: nowMilliseconds,
            replayGuard: &replayGuard
        )
    }

    public static func verifyHostRequest(
        canonicalBytes: Data,
        signatureRaw: Data,
        enrollment: HostEnrollment?,
        expectedWatchEnrollmentID: String,
        nowMilliseconds: UInt64,
        replayGuard: inout WatchAuthReplayGuard
    ) throws -> VerifiedHostRequest {
        guard let enrollment else { throw WatchAuthError.unknownEnrollment }
        try validate(enrollment)
        let request = try WatchAuthRequest.parseCanonical(canonicalBytes)
        try validateTime(request, now: nowMilliseconds)
        guard request.action == watchAuthAction else { throw WatchAuthError.wrongAction }
        guard request.hostEnrollmentID == enrollment.enrollmentID,
              request.hostIdentity == enrollment.hostIdentity,
              request.linuxUID == enrollment.linuxUID,
              request.targetWatchEnrollmentID == expectedWatchEnrollmentID else {
            throw WatchAuthError.bindingMismatch
        }
        try verifySignature(
            signatureRaw,
            domain: requestDomain,
            canonical: canonicalBytes,
            publicKey: enrollment.publicKeySEC1
        )
        try replayGuard.remember(nonce: request.nonce, sessionID: request.sessionID)
        return VerifiedHostRequest(request: request, canonicalRequest: canonicalBytes)
    }

    /// Validate the Watch's independently enrolled signing identity before
    /// presenting consent. This does not sign or authorize a response.
    public static func validateWatchResponseEligibility(
        verifiedRequest: VerifiedHostRequest,
        enrollment: WatchEnrollment,
        signingKey: P256.Signing.PrivateKey
    ) throws {
        try validate(enrollment)
        let request = verifiedRequest.request
        guard request.targetWatchEnrollmentID == enrollment.enrollmentID,
              enrollment.boundHostEnrollmentID == request.hostEnrollmentID,
              enrollment.boundHostIdentity == request.hostIdentity,
              enrollment.boundLinuxUID == request.linuxUID,
              signingKey.publicKey.x963Representation == enrollment.publicKeySEC1 else {
            throw WatchAuthError.bindingMismatch
        }
    }

    /// Call only after a manual foreground decision. Approval callers must
    /// first complete supported LocalAuthentication; rejection is a manual
    /// decision too. Cancellation and LocalAuthentication failure return no
    /// signed response.
    public static func signWatchResponse(
        verifiedRequest: VerifiedHostRequest,
        enrollment: WatchEnrollment,
        signingKey: P256.Signing.PrivateKey,
        decision: WatchAuthDecision,
        consentResult: WatchConsentResult,
        responseAtMilliseconds: UInt64
    ) throws -> SignedWatchResponse {
        try validateWatchResponseEligibility(
            verifiedRequest: verifiedRequest,
            enrollment: enrollment,
            signingKey: signingKey
        )
        guard consentResult == .explicitUserDecision else {
            throw WatchAuthError.consentNotCompleted
        }
        let request = verifiedRequest.request
        guard responseAtMilliseconds >= request.issuedAtMilliseconds else {
            throw WatchAuthError.notYetValid
        }
        guard responseAtMilliseconds < request.expiresAtMilliseconds else {
            throw WatchAuthError.expired
        }
        let response = WatchAuthResponse(
            requestDigest: verifiedRequest.digest,
            nonce: request.nonce,
            hostEnrollmentID: request.hostEnrollmentID,
            hostIdentity: request.hostIdentity,
            linuxUID: request.linuxUID,
            targetWatchEnrollmentID: request.targetWatchEnrollmentID,
            watchIdentity: enrollment.watchIdentity,
            sessionID: request.sessionID,
            action: request.action,
            decision: decision,
            responseAtMilliseconds: responseAtMilliseconds,
            consentResult: consentResult
        )
        let canonical = try response.canonicalBytes()
        return SignedWatchResponse(
            response: response,
            signatureRaw: try signingKey.signature(for: signedTranscript(responseDomain, canonical)).rawRepresentation
        )
    }

    public static func verifyWatchResponse(
        canonicalBytes: Data,
        signatureRaw: Data,
        enrollment: WatchEnrollment?,
        verifiedRequest: VerifiedHostRequest,
        nowMilliseconds: UInt64,
        replayGuard: inout WatchAuthReplayGuard
    ) throws -> WatchAuthResponse {
        guard let enrollment else { throw WatchAuthError.unknownEnrollment }
        try validate(enrollment)
        let response = try WatchAuthResponse.parseCanonical(canonicalBytes)
        let request = verifiedRequest.request
        guard response.consentResult == .explicitUserDecision else {
            throw WatchAuthError.consentNotCompleted
        }
        guard response.action == watchAuthAction else { throw WatchAuthError.wrongAction }
        guard response.responseAtMilliseconds >= request.issuedAtMilliseconds else {
            throw WatchAuthError.notYetValid
        }
        guard response.responseAtMilliseconds < request.expiresAtMilliseconds,
              nowMilliseconds < request.expiresAtMilliseconds else {
            throw WatchAuthError.expired
        }
        guard response.responseAtMilliseconds <= nowMilliseconds else {
            throw WatchAuthError.invalidTimestamp
        }
        guard response.requestDigest == verifiedRequest.digest,
              response.nonce == request.nonce,
              response.sessionID == request.sessionID,
              response.hostEnrollmentID == request.hostEnrollmentID,
              response.hostIdentity == request.hostIdentity,
              response.linuxUID == request.linuxUID,
              response.targetWatchEnrollmentID == request.targetWatchEnrollmentID,
              response.targetWatchEnrollmentID == enrollment.enrollmentID,
              response.watchIdentity == enrollment.watchIdentity,
              response.action == request.action,
              enrollment.boundHostEnrollmentID == request.hostEnrollmentID,
              enrollment.boundHostIdentity == request.hostIdentity,
              enrollment.boundLinuxUID == request.linuxUID else {
            throw WatchAuthError.bindingMismatch
        }
        try verifySignature(
            signatureRaw,
            domain: responseDomain,
            canonical: canonicalBytes,
            publicKey: enrollment.publicKeySEC1
        )
        try replayGuard.remember(nonce: response.nonce, sessionID: response.sessionID)
        return response
    }
}

private func encodeRequest(_ request: WatchAuthRequest) throws -> Data {
    try validateIdentifier(request.hostEnrollmentID)
    try validateIdentifier(request.hostIdentity)
    try validateIdentifier(request.targetWatchEnrollmentID)
    try validateIdentifier(request.action)
    guard request.action == watchAuthAction else { throw WatchAuthError.wrongAction }
    guard request.sessionID.count == 16, request.nonce.count == 32 else {
        throw WatchAuthError.malformed
    }
    var writer = CanonicalWriter(kind: requestKind)
    try writer.appendIdentifier(request.hostEnrollmentID)
    try writer.appendIdentifier(request.hostIdentity)
    writer.appendUInt32(request.linuxUID)
    try writer.appendIdentifier(request.targetWatchEnrollmentID)
    writer.append(request.sessionID)
    writer.append(request.nonce)
    writer.appendUInt64(request.issuedAtMilliseconds)
    writer.appendUInt64(request.expiresAtMilliseconds)
    try writer.appendIdentifier(request.action)
    return try writer.finish()
}

private func encodeResponse(_ response: WatchAuthResponse) throws -> Data {
    try validateIdentifier(response.hostEnrollmentID)
    try validateIdentifier(response.hostIdentity)
    try validateIdentifier(response.targetWatchEnrollmentID)
    try validateIdentifier(response.watchIdentity)
    try validateIdentifier(response.action)
    guard response.action == watchAuthAction else { throw WatchAuthError.wrongAction }
    guard response.requestDigest.count == 32,
          response.nonce.count == 32,
          response.sessionID.count == 16 else {
        throw WatchAuthError.malformed
    }
    var writer = CanonicalWriter(kind: responseKind)
    writer.append(response.requestDigest)
    writer.append(response.nonce)
    try writer.appendIdentifier(response.hostEnrollmentID)
    try writer.appendIdentifier(response.hostIdentity)
    writer.appendUInt32(response.linuxUID)
    try writer.appendIdentifier(response.targetWatchEnrollmentID)
    try writer.appendIdentifier(response.watchIdentity)
    writer.append(response.sessionID)
    try writer.appendIdentifier(response.action)
    writer.append(response.decision.rawValue)
    writer.appendUInt64(response.responseAtMilliseconds)
    writer.append(response.consentResult.rawValue)
    return try writer.finish()
}

private struct CanonicalWriter {
    private var bytes: [UInt8] = [0x4f, 0x43, 0x57, 0x41, 1]

    init(kind: UInt8) {
        bytes.append(kind)
        bytes.append(algorithmID)
    }

    mutating func append(_ value: Data) { bytes.append(contentsOf: value) }
    mutating func append(_ value: UInt8) { bytes.append(value) }
    mutating func appendUInt32(_ value: UInt32) {
        bytes.append(contentsOf: value.bigEndianBytes)
    }
    mutating func appendUInt64(_ value: UInt64) {
        bytes.append(contentsOf: value.bigEndianBytes)
    }
    mutating func appendIdentifier(_ value: String) throws {
        try validateIdentifier(value)
        let data = Data(value.utf8)
        guard data.count <= UInt16.max else { throw WatchAuthError.invalidIdentifier }
        bytes.append(contentsOf: UInt16(data.count).bigEndianBytes)
        bytes.append(contentsOf: data)
    }
    func finish() throws -> Data {
        guard bytes.count <= watchAuthMaximumMessageLength else { throw WatchAuthError.oversized }
        return Data(bytes)
    }
}

private struct CanonicalReader {
    private let bytes: [UInt8]
    private var offset = 0

    init(_ data: Data) { bytes = Array(data) }

    mutating func readHeader(expectedKind: UInt8) throws {
        guard try readBytes(count: 4) == [0x4f, 0x43, 0x57, 0x41] else {
            throw WatchAuthError.malformed
        }
        guard try readUInt8() == 1 else { throw WatchAuthError.unsupportedVersion }
        guard try readUInt8() == expectedKind else { throw WatchAuthError.wrongMessageType }
        guard try readUInt8() == algorithmID else { throw WatchAuthError.unsupportedAlgorithm }
    }

    mutating func readBytes(count: Int) throws -> [UInt8] {
        guard count >= 0, offset <= bytes.count, count <= bytes.count - offset else {
            throw WatchAuthError.malformed
        }
        defer { offset += count }
        return Array(bytes[offset..<(offset + count)])
    }

    mutating func readUInt8() throws -> UInt8 { try readBytes(count: 1)[0] }
    mutating func readUInt16() throws -> UInt16 { UInt16(bigEndianBytes: try readBytes(count: 2)) }
    mutating func readUInt32() throws -> UInt32 { UInt32(bigEndianBytes: try readBytes(count: 4)) }
    mutating func readUInt64() throws -> UInt64 { UInt64(bigEndianBytes: try readBytes(count: 8)) }

    mutating func readIdentifier() throws -> String {
        let length = Int(try readUInt16())
        guard (1...watchAuthMaximumIdentifierLength).contains(length) else {
            throw WatchAuthError.invalidIdentifier
        }
        let valueBytes = try readBytes(count: length)
        guard let value = String(bytes: valueBytes, encoding: .utf8) else {
            throw WatchAuthError.invalidUTF8
        }
        try validateIdentifier(value)
        return value
    }

    func finish() throws {
        guard offset == bytes.count else { throw WatchAuthError.nonCanonical }
    }
}

private func validateIdentifier(_ value: String) throws {
    let bytes = Array(value.utf8)
    guard !bytes.isEmpty,
          bytes.count <= watchAuthMaximumIdentifierLength,
          bytes.allSatisfy(isIdentifierByte) else {
        throw WatchAuthError.invalidIdentifier
    }
}

private func isIdentifierByte(_ byte: UInt8) -> Bool {
    (0x30...0x39).contains(byte)
        || (0x41...0x5a).contains(byte)
        || (0x61...0x7a).contains(byte)
        || [0x2e, 0x5f, 0x3a, 0x2d].contains(byte)
}

private func validate(_ enrollment: HostEnrollment) throws {
    try validateRecord(
        enrollment.enrollmentID,
        identity: enrollment.hostIdentity,
        publicKey: enrollment.publicKeySEC1,
        algorithm: enrollment.algorithm,
        keyVersion: enrollment.keyVersion,
        state: enrollment.state,
        testOnly: enrollment.testOnly
    )
}

private func validate(_ enrollment: WatchEnrollment) throws {
    try validateRecord(
        enrollment.enrollmentID,
        identity: enrollment.watchIdentity,
        publicKey: enrollment.publicKeySEC1,
        algorithm: enrollment.algorithm,
        keyVersion: enrollment.keyVersion,
        state: enrollment.state,
        testOnly: enrollment.testOnly
    )
    try validateIdentifier(enrollment.boundHostEnrollmentID)
    try validateIdentifier(enrollment.boundHostIdentity)
}

private func validateRecord(
    _ enrollmentID: String,
    identity: String,
    publicKey: Data,
    algorithm: WatchKeyAlgorithm,
    keyVersion: UInt8,
    state: WatchEnrollmentState,
    testOnly: Bool
) throws {
    try validateIdentifier(enrollmentID)
    try validateIdentifier(identity)
    guard algorithm == .p256EcdsaSha256, keyVersion == 1 else {
        throw WatchAuthError.unsupportedAlgorithm
    }
    guard testOnly else { throw WatchAuthError.nonTestEnrollment }
    switch state {
    case .active: break
    case .suspended: throw WatchAuthError.enrollmentInactive
    case .revoked: throw WatchAuthError.enrollmentRevoked
    }
    _ = try p256PublicKey(publicKey)
}

private func p256PublicKey(_ bytes: Data) throws -> P256.Signing.PublicKey {
    guard bytes.count == 65, bytes.first == 0x04 else { throw WatchAuthError.invalidPublicKey }
    do { return try P256.Signing.PublicKey(x963Representation: bytes) }
    catch { throw WatchAuthError.invalidPublicKey }
}

private func signedTranscript(_ domain: Data, _ canonical: Data) -> Data {
    var output = domain
    output.append(canonical)
    return output
}

private func verifySignature(
    _ rawSignature: Data,
    domain: Data,
    canonical: Data,
    publicKey: Data
) throws {
    guard rawSignature.count == 64 else { throw WatchAuthError.invalidSignature }
    let key = try p256PublicKey(publicKey)
    let signature: P256.Signing.ECDSASignature
    do { signature = try P256.Signing.ECDSASignature(rawRepresentation: rawSignature) }
    catch { throw WatchAuthError.invalidSignature }
    guard key.isValidSignature(signature, for: signedTranscript(domain, canonical)) else {
        throw WatchAuthError.invalidSignature
    }
}

private func validateTime(_ request: WatchAuthRequest, now: UInt64) throws {
    guard request.expiresAtMilliseconds > request.issuedAtMilliseconds else {
        throw WatchAuthError.invalidTimestamp
    }
    guard request.expiresAtMilliseconds - request.issuedAtMilliseconds <= watchAuthMaximumLifetimeMilliseconds else {
        throw WatchAuthError.lifetimeTooLong
    }
    guard request.issuedAtMilliseconds <= now else { throw WatchAuthError.notYetValid }
    guard now < request.expiresAtMilliseconds else { throw WatchAuthError.expired }
}

private extension UInt16 {
    var bigEndianBytes: [UInt8] { [UInt8(self >> 8), UInt8(self & 0xff)] }
    init(bigEndianBytes: [UInt8]) { self = (UInt16(bigEndianBytes[0]) << 8) | UInt16(bigEndianBytes[1]) }
}

private extension UInt32 {
    var bigEndianBytes: [UInt8] {
        [UInt8(self >> 24), UInt8((self >> 16) & 0xff), UInt8((self >> 8) & 0xff), UInt8(self & 0xff)]
    }
    init(bigEndianBytes: [UInt8]) {
        self = (UInt32(bigEndianBytes[0]) << 24) | (UInt32(bigEndianBytes[1]) << 16)
            | (UInt32(bigEndianBytes[2]) << 8) | UInt32(bigEndianBytes[3])
    }
}

private extension UInt64 {
    var bigEndianBytes: [UInt8] {
        (0..<8).reversed().map { UInt8((self >> UInt64($0 * 8)) & 0xff) }
    }
    init(bigEndianBytes: [UInt8]) {
        self = bigEndianBytes.reduce(UInt64(0)) { ($0 << 8) | UInt64($1) }
    }
}
