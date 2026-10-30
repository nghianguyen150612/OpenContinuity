import CryptoKit
import Foundation

public let p009MaximumTestEnrollmentLifetimeMilliseconds: UInt64 = 3_600_000

public struct P009HostPinDocument: Decodable {
    public let schemaVersion: UInt8
    public let expiresAtUnixMs: UInt64
    public let hostEnrollment: P009HostEnrollmentRecord
    public let hostFingerprintSHA256: String

    enum CodingKeys: String, CodingKey {
        case schemaVersion = "schema_version"
        case expiresAtUnixMs = "expires_at_unix_ms"
        case hostEnrollment = "host_enrollment"
        case hostFingerprintSHA256 = "host_fingerprint_sha256"
    }

    public init(from decoder: Decoder) throws {
        let dynamic = try decoder.container(keyedBy: P009DynamicCodingKey.self)
        try p009RequireExactKeys(
            dynamic,
            expected: ["schema_version", "expires_at_unix_ms", "host_enrollment", "host_fingerprint_sha256"],
            decoder: decoder
        )
        let values = try decoder.container(keyedBy: CodingKeys.self)
        schemaVersion = try values.decode(UInt8.self, forKey: .schemaVersion)
        expiresAtUnixMs = try values.decode(UInt64.self, forKey: .expiresAtUnixMs)
        hostEnrollment = try values.decode(P009HostEnrollmentRecord.self, forKey: .hostEnrollment)
        hostFingerprintSHA256 = try values.decode(String.self, forKey: .hostFingerprintSHA256)
    }

    public func validate(nowMilliseconds: UInt64) throws -> HostEnrollment {
        guard schemaVersion == 1 else {
            throw P009EnrollmentError.incompleteOrUnsupported
        }
        guard expiresAtUnixMs > nowMilliseconds,
              expiresAtUnixMs - nowMilliseconds <= p009MaximumTestEnrollmentLifetimeMilliseconds else {
            throw P009EnrollmentError.expired
        }
        let host = try hostEnrollment.protocolValue()
        guard hostFingerprintSHA256 == p009Fingerprint(host.publicKeySEC1) else {
            throw P009EnrollmentError.mismatchedFingerprint
        }
        return host
    }
}

public struct P009HostEnrollmentRecord: Codable {
    public let enrollmentID: String
    public let hostIdentity: String
    public let linuxUID: UInt32
    public let publicKeySEC1Hex: String
    public let keyAlgorithm: String
    public let keyVersion: UInt8
    public let state: String
    public let testOnly: Bool

    public init(
        enrollmentID: String,
        hostIdentity: String,
        linuxUID: UInt32,
        publicKeySEC1Hex: String,
        keyAlgorithm: String,
        keyVersion: UInt8,
        state: String,
        testOnly: Bool
    ) {
        self.enrollmentID = enrollmentID
        self.hostIdentity = hostIdentity
        self.linuxUID = linuxUID
        self.publicKeySEC1Hex = publicKeySEC1Hex
        self.keyAlgorithm = keyAlgorithm
        self.keyVersion = keyVersion
        self.state = state
        self.testOnly = testOnly
    }

    enum CodingKeys: String, CodingKey {
        case enrollmentID = "enrollment_id"
        case hostIdentity = "host_identity"
        case linuxUID = "linux_uid"
        case publicKeySEC1Hex = "public_key_sec1_hex"
        case keyAlgorithm = "key_algorithm"
        case keyVersion = "key_version"
        case state
        case testOnly = "test_only"
    }

    public init(from decoder: Decoder) throws {
        let dynamic = try decoder.container(keyedBy: P009DynamicCodingKey.self)
        try p009RequireExactKeys(
            dynamic,
            expected: ["enrollment_id", "host_identity", "linux_uid", "public_key_sec1_hex", "key_algorithm", "key_version", "state", "test_only"],
            decoder: decoder
        )
        let values = try decoder.container(keyedBy: CodingKeys.self)
        enrollmentID = try values.decode(String.self, forKey: .enrollmentID)
        hostIdentity = try values.decode(String.self, forKey: .hostIdentity)
        linuxUID = try values.decode(UInt32.self, forKey: .linuxUID)
        publicKeySEC1Hex = try values.decode(String.self, forKey: .publicKeySEC1Hex)
        keyAlgorithm = try values.decode(String.self, forKey: .keyAlgorithm)
        keyVersion = try values.decode(UInt8.self, forKey: .keyVersion)
        state = try values.decode(String.self, forKey: .state)
        testOnly = try values.decode(Bool.self, forKey: .testOnly)
    }

    func protocolValue() throws -> HostEnrollment {
        guard !enrollmentID.isEmpty, !hostIdentity.isEmpty,
              p009ValidIdentifier(enrollmentID), p009ValidIdentifier(hostIdentity),
              keyAlgorithm == "p256_ecdsa_sha256", keyVersion == 1,
              state == "active", testOnly,
              let publicKey = Data(strictLowercaseHex: publicKeySEC1Hex),
              publicKey.count == 65, publicKey.first == 4,
              (try? P256.Signing.PublicKey(x963Representation: publicKey)) != nil else {
            throw P009EnrollmentError.incompleteOrUnsupported
        }
        return HostEnrollment(
            enrollmentID: enrollmentID,
            hostIdentity: hostIdentity,
            linuxUID: linuxUID,
            publicKeySEC1: publicKey,
            algorithm: .p256EcdsaSha256,
            keyVersion: keyVersion,
            state: .active,
            testOnly: true
        )
    }
}

private struct P009DynamicCodingKey: CodingKey {
    let stringValue: String
    let intValue: Int?

    init?(stringValue: String) {
        self.stringValue = stringValue
        intValue = nil
    }

    init?(intValue: Int) {
        stringValue = String(intValue)
        self.intValue = intValue
    }
}

private func p009RequireExactKeys(
    _ container: KeyedDecodingContainer<P009DynamicCodingKey>,
    expected: Set<String>,
    decoder: Decoder
) throws {
    let actual = Set(container.allKeys.map(\.stringValue))
    guard actual == expected else {
        throw DecodingError.dataCorrupted(.init(
            codingPath: decoder.codingPath,
            debugDescription: "P009A test enrollment fields are incomplete or unsupported"
        ))
    }
}

private func p009ValidIdentifier(_ value: String) -> Bool {
    guard !value.isEmpty, value.utf8.count <= 128 else { return false }
    return value.utf8.allSatisfy {
        ($0 >= 48 && $0 <= 57) || ($0 >= 65 && $0 <= 90) || ($0 >= 97 && $0 <= 122)
            || $0 == 45 || $0 == 46 || $0 == 58 || $0 == 95
    }
}

public struct P009WatchEnrollmentRecord: Codable {
    public let enrollmentID: String
    public let watchIdentity: String
    public let boundHostEnrollmentID: String
    public let boundHostIdentity: String
    public let boundLinuxUID: UInt32
    public let publicKeySEC1Hex: String
    public let keyAlgorithm: String
    public let keyVersion: UInt8
    public let state: String
    public let testOnly: Bool

    public init(
        enrollmentID: String,
        watchIdentity: String,
        boundHostEnrollmentID: String,
        boundHostIdentity: String,
        boundLinuxUID: UInt32,
        publicKeySEC1Hex: String,
        keyAlgorithm: String,
        keyVersion: UInt8,
        state: String,
        testOnly: Bool
    ) {
        self.enrollmentID = enrollmentID
        self.watchIdentity = watchIdentity
        self.boundHostEnrollmentID = boundHostEnrollmentID
        self.boundHostIdentity = boundHostIdentity
        self.boundLinuxUID = boundLinuxUID
        self.publicKeySEC1Hex = publicKeySEC1Hex
        self.keyAlgorithm = keyAlgorithm
        self.keyVersion = keyVersion
        self.state = state
        self.testOnly = testOnly
    }

    enum CodingKeys: String, CodingKey {
        case enrollmentID = "enrollment_id"
        case watchIdentity = "watch_identity"
        case boundHostEnrollmentID = "bound_host_enrollment_id"
        case boundHostIdentity = "bound_host_identity"
        case boundLinuxUID = "bound_linux_uid"
        case publicKeySEC1Hex = "public_key_sec1_hex"
        case keyAlgorithm = "key_algorithm"
        case keyVersion = "key_version"
        case state
        case testOnly = "test_only"
    }
}

public struct P009LinuxEnrollmentDocument: Encodable {
    public let schemaVersion: UInt8 = 2
    public let expiresAtUnixMs: UInt64
    public let hostEnrollment: P009HostEnrollmentRecord
    public let watchEnrollment: P009WatchEnrollmentRecord

    public init(
        expiresAtUnixMs: UInt64,
        hostEnrollment: P009HostEnrollmentRecord,
        watchEnrollment: P009WatchEnrollmentRecord
    ) {
        self.expiresAtUnixMs = expiresAtUnixMs
        self.hostEnrollment = hostEnrollment
        self.watchEnrollment = watchEnrollment
    }

    enum CodingKeys: String, CodingKey {
        case schemaVersion = "schema_version"
        case expiresAtUnixMs = "expires_at_unix_ms"
        case hostEnrollment = "host_enrollment"
        case watchEnrollment = "watch_enrollment"
    }
}

public enum P009EnrollmentError: Error {
    case incompleteOrUnsupported
    case expired
    case mismatchedFingerprint
    case noEphemeralKey
}

extension Data {
    init?(strictLowercaseHex value: String) {
        guard value.count.isMultiple(of: 2),
              value.utf8.allSatisfy({ ($0 >= 48 && $0 <= 57) || ($0 >= 97 && $0 <= 102) }) else {
            return nil
        }
        var result = Data(capacity: value.count / 2)
        var index = value.startIndex
        while index < value.endIndex {
            let next = value.index(index, offsetBy: 2)
            guard let byte = UInt8(value[index..<next], radix: 16) else { return nil }
            result.append(byte)
            index = next
        }
        self = result
    }
}

public func p009Fingerprint(_ publicKey: Data) -> String {
    SHA256.hash(data: publicKey).map { String(format: "%02x", $0) }.joined()
}

public func p009DisplayFingerprint(_ fingerprint: String) -> String {
    stride(from: 0, to: fingerprint.count, by: 4)
        .map { offset in
            let start = fingerprint.index(fingerprint.startIndex, offsetBy: offset)
            let end = fingerprint.index(start, offsetBy: min(4, fingerprint.count - offset))
            return String(fingerprint[start..<end])
        }
        .joined(separator: ":")
        .uppercased()
}

public func p009Hex(_ bytes: Data) -> String {
    bytes.map { String(format: "%02x", $0) }.joined()
}
