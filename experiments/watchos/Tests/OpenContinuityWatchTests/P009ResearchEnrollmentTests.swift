import CryptoKit
import Foundation
import XCTest
@testable import OpenContinuityProtocol

final class P009ResearchEnrollmentTests: XCTestCase {
    func testHostPinRequiresActiveTestOnlyP256AndMatchingFingerprint() throws {
        let key = P256.Signing.PrivateKey()
        let publicKey = Data(key.publicKey.x963Representation)
        let document = try hostPin(publicKey: publicKey, now: 1_000)

        let validated = try document.validate(nowMilliseconds: 1_000)

        XCTAssertEqual(validated.hostIdentity, "lab-host-01")
        XCTAssertEqual(validated.linuxUID, 1000)
        XCTAssertEqual(validated.publicKeySEC1, publicKey)
        XCTAssertTrue(validated.testOnly)
    }

    func testHostPinRejectsExpiredOrOverlongValidity() throws {
        let key = P256.Signing.PrivateKey()
        let publicKey = Data(key.publicKey.x963Representation)
        let expired = try hostPin(publicKey: publicKey, now: 1_000, expires: 999)
        let tooLong = try hostPin(publicKey: publicKey, now: 1_000, expires: 3_601_001)

        XCTAssertThrowsError(try expired.validate(nowMilliseconds: 1_000)) { error in
            guard case P009EnrollmentError.expired = error else {
                return XCTFail("expected expired host pin, got \(error)")
            }
        }
        XCTAssertThrowsError(try tooLong.validate(nowMilliseconds: 1_000)) { error in
            guard case P009EnrollmentError.expired = error else {
                return XCTFail("expected overlong host pin, got \(error)")
            }
        }
    }

    func testHostPinRejectsFingerprintAndEnrollmentTampering() throws {
        let key = P256.Signing.PrivateKey()
        let publicKey = Data(key.publicKey.x963Representation)
        let mismatched = try hostPin(publicKey: publicKey, now: 1_000, fingerprint: String(repeating: "0", count: 64))
        XCTAssertThrowsError(try mismatched.validate(nowMilliseconds: 1_000)) { error in
            guard case P009EnrollmentError.mismatchedFingerprint = error else {
                return XCTFail("expected fingerprint mismatch, got \(error)")
            }
        }

        let revokedRecord = P009HostEnrollmentRecord(
            enrollmentID: "host-test-01",
            hostIdentity: "lab-host-01",
            linuxUID: 1000,
            publicKeySEC1Hex: p009Hex(publicKey),
            keyAlgorithm: "p256_ecdsa_sha256",
            keyVersion: 1,
            state: "revoked",
            testOnly: true
        )
        let revokedData = try encodedPin(record: revokedRecord, fingerprint: p009Fingerprint(publicKey), expires: 3_000_000)
        let revoked = try JSONDecoder().decode(P009HostPinDocument.self, from: revokedData)
        XCTAssertThrowsError(try revoked.validate(nowMilliseconds: 1_000))
    }

    func testHostPinRejectsUnknownOrIncompleteFields() throws {
        let key = P256.Signing.PrivateKey()
        let publicKey = Data(key.publicKey.x963Representation)
        let valid = try hostPin(publicKey: publicKey, now: 1_000)
        let validData = try encodedPin(
            record: valid.hostEnrollment,
            fingerprint: valid.hostFingerprintSHA256,
            expires: valid.expiresAtUnixMs
        )
        var root = try XCTUnwrap(JSONSerialization.jsonObject(with: validData) as? [String: Any])
        root["unexpected"] = "ignored by default Codable decoders"
        let unknownTopLevel = try JSONSerialization.data(withJSONObject: root)
        XCTAssertThrowsError(try JSONDecoder().decode(P009HostPinDocument.self, from: unknownTopLevel))

        root.removeValue(forKey: "unexpected")
        var host = try XCTUnwrap(root["host_enrollment"] as? [String: Any])
        host.removeValue(forKey: "test_only")
        root["host_enrollment"] = host
        let incompleteHost = try JSONSerialization.data(withJSONObject: root)
        XCTAssertThrowsError(try JSONDecoder().decode(P009HostPinDocument.self, from: incompleteHost))
    }

    func testWatchEnrollmentExportHasBindingsAndOnlyPublicKeys() throws {
        let hostKey = P256.Signing.PrivateKey()
        let watchKey = P256.Signing.PrivateKey()
        let hostPublic = Data(hostKey.publicKey.x963Representation)
        let watchPublic = Data(watchKey.publicKey.x963Representation)
        let host = P009HostEnrollmentRecord(
            enrollmentID: "host-test-01",
            hostIdentity: "lab-host-01",
            linuxUID: 1000,
            publicKeySEC1Hex: p009Hex(hostPublic),
            keyAlgorithm: "p256_ecdsa_sha256",
            keyVersion: 1,
            state: "active",
            testOnly: true
        )
        let watch = P009WatchEnrollmentRecord(
            enrollmentID: "test-watch-123",
            watchIdentity: "watch-research-123",
            boundHostEnrollmentID: host.enrollmentID,
            boundHostIdentity: host.hostIdentity,
            boundLinuxUID: host.linuxUID,
            publicKeySEC1Hex: p009Hex(watchPublic),
            keyAlgorithm: "p256_ecdsa_sha256",
            keyVersion: 1,
            state: "active",
            testOnly: true
        )
        let data = try JSONEncoder().encode(P009LinuxEnrollmentDocument(
            expiresAtUnixMs: 3_600_000,
            hostEnrollment: host,
            watchEnrollment: watch
        ))
        let json = try XCTUnwrap(JSONSerialization.jsonObject(with: data) as? [String: Any])
        let watchRecord = try XCTUnwrap(json["watch_enrollment"] as? [String: Any])

        XCTAssertEqual(json["schema_version"] as? Int, 2)
        XCTAssertEqual(watchRecord["bound_host_enrollment_id"] as? String, host.enrollmentID)
        XCTAssertEqual(watchRecord["bound_host_identity"] as? String, host.hostIdentity)
        XCTAssertEqual(watchRecord["bound_linux_uid"] as? Int, Int(host.linuxUID))
        XCTAssertEqual(watchRecord["test_only"] as? Bool, true)
        XCTAssertNil(watchRecord["private_key"])
        XCTAssertNil(watchRecord["private_key_hex"])
    }

    private func hostPin(
        publicKey: Data,
        now: UInt64,
        expires: UInt64? = nil,
        fingerprint: String? = nil
    ) throws -> P009HostPinDocument {
        let record = P009HostEnrollmentRecord(
            enrollmentID: "host-test-01",
            hostIdentity: "lab-host-01",
            linuxUID: 1000,
            publicKeySEC1Hex: p009Hex(publicKey),
            keyAlgorithm: "p256_ecdsa_sha256",
            keyVersion: 1,
            state: "active",
            testOnly: true
        )
        let data = try encodedPin(
            record: record,
            fingerprint: fingerprint ?? p009Fingerprint(publicKey),
            expires: expires ?? (now + 3_600_000)
        )
        return try JSONDecoder().decode(P009HostPinDocument.self, from: data)
    }

    private func encodedPin(
        record: P009HostEnrollmentRecord,
        fingerprint: String,
        expires: UInt64
    ) throws -> Data {
        let host = try XCTUnwrap(JSONSerialization.jsonObject(with: JSONEncoder().encode(record)) as? [String: Any])
        return try JSONSerialization.data(withJSONObject: [
            "schema_version": 1,
            "expires_at_unix_ms": expires,
            "host_enrollment": host,
            "host_fingerprint_sha256": fingerprint
        ])
    }
}
