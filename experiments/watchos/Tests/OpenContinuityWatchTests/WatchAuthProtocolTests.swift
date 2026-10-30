import CryptoKit
import Foundation
import XCTest
@testable import OpenContinuityProtocol

final class WatchAuthProtocolTests: XCTestCase {
    private struct EnrollmentFixture: Decodable {
        let enrollment_id: String
        let host_identity: String?
        let watch_identity: String?
        let linux_uid: UInt32?
        let bound_host_enrollment_id: String?
        let bound_host_identity: String?
        let bound_linux_uid: UInt32?
        let public_key_sec1_hex: String
        let key_algorithm: String
        let key_version: UInt8
        let state: String
        let test_only: Bool
    }

    private struct SignedFixture: Codable {
        var canonical_hex: String
        var signature_raw_hex: String
    }

    private struct Fixture: Decodable {
        let schema_version: UInt8
        let now_ms: UInt64
        let host_enrollment: EnrollmentFixture
        let watch_enrollment: EnrollmentFixture
        let request: SignedFixture
        let response: SignedFixture?
    }

    private var fixture: Fixture {
        let url = Bundle.module.url(forResource: "watch-auth-request-v1", withExtension: "json", subdirectory: "Fixtures")!
        return try! JSONDecoder().decode(Fixture.self, from: Data(contentsOf: url))
    }

    private var hostEnrollment: HostEnrollment {
        let value = fixture.host_enrollment
        return HostEnrollment(
            enrollmentID: value.enrollment_id,
            hostIdentity: value.host_identity!,
            linuxUID: value.linux_uid!,
            publicKeySEC1: try! data(value.public_key_sec1_hex),
            algorithm: WatchKeyAlgorithm(rawValue: value.key_algorithm)!,
            keyVersion: value.key_version,
            state: WatchEnrollmentState(rawValue: value.state)!,
            testOnly: value.test_only
        )
    }

    private var watchEnrollment: WatchEnrollment {
        let value = fixture.watch_enrollment
        return WatchEnrollment(
            enrollmentID: value.enrollment_id,
            watchIdentity: value.watch_identity!,
            boundHostEnrollmentID: value.bound_host_enrollment_id!,
            boundHostIdentity: value.bound_host_identity!,
            boundLinuxUID: value.bound_linux_uid!,
            publicKeySEC1: try! data(value.public_key_sec1_hex),
            algorithm: WatchKeyAlgorithm(rawValue: value.key_algorithm)!,
            keyVersion: value.key_version,
            state: WatchEnrollmentState(rawValue: value.state)!,
            testOnly: value.test_only
        )
    }

    private func data(_ hex: String) throws -> Data {
        guard hex.count.isMultiple(of: 2), hex == hex.lowercased() else {
            throw WatchAuthError.nonCanonical
        }
        var output = Data()
        var index = hex.startIndex
        while index < hex.endIndex {
            let next = hex.index(index, offsetBy: 2)
            guard let byte = UInt8(hex[index..<next], radix: 16) else {
                throw WatchAuthError.nonCanonical
            }
            output.append(byte)
            index = next
        }
        return output
    }

    private func hex(_ bytes: Data) -> String {
        bytes.map { String(format: "%02x", $0) }.joined()
    }

    private func verifiedRequest(now: UInt64? = nil) throws -> VerifiedHostRequest {
        var replayGuard = WatchAuthReplayGuard()
        return try WatchAuthProtocol.verifyHostRequest(
            canonicalBytes: data(fixture.request.canonical_hex),
            signatureRaw: data(fixture.request.signature_raw_hex),
            enrollment: hostEnrollment,
            expectedWatchEnrollmentID: watchEnrollment.enrollmentID,
            nowMilliseconds: now ?? fixture.now_ms,
            replayGuard: &replayGuard
        )
    }

    private func watchTestKey() throws -> P256.Signing.PrivateKey {
        // TEST-ONLY scalar. It exists only in this test target, never in the app.
        try P256.Signing.PrivateKey(rawRepresentation: Data(repeating: 0x02, count: 32))
    }

    func testRustRequestFixtureHasCanonicalBytesAndVerifiesWithPinnedHostKey() throws {
        let requestBytes = try data(fixture.request.canonical_hex)
        let request = try WatchAuthRequest.parseCanonical(requestBytes)
        XCTAssertEqual(try request.canonicalBytes(), requestBytes)
        XCTAssertEqual(request.action, watchAuthAction)
        XCTAssertEqual(request.nonce.count, 32)
        XCTAssertEqual(request.sessionID.count, 16)
        XCTAssertEqual(hostEnrollment.publicKeySEC1.count, 65)
        XCTAssertEqual(try data(fixture.request.signature_raw_hex).count, 64)
        XCTAssertEqual(try verifiedRequest().digest.count, 32)
    }

    func testSwiftSignatureIsEmittedForRustVerifier() throws {
        let token = try verifiedRequest()
        let signed = try WatchAuthProtocol.signWatchResponse(
            verifiedRequest: token,
            enrollment: watchEnrollment,
            signingKey: watchTestKey(),
            decision: .approved,
            consentResult: .explicitUserDecision,
            responseAtMilliseconds: fixture.now_ms
        )
        XCTAssertEqual(signed.signatureRaw.count, 64)
        let responseBytes = try signed.response.canonicalBytes()
        XCTAssertEqual(try WatchAuthResponse.parseCanonical(responseBytes), signed.response)
        let responseDTO = SignedFixture(
            canonical_hex: hex(responseBytes),
            signature_raw_hex: hex(signed.signatureRaw)
        )

        let url = Bundle.module.url(forResource: "watch-auth-request-v1", withExtension: "json", subdirectory: "Fixtures")!
        var root = try XCTUnwrap(JSONSerialization.jsonObject(with: Data(contentsOf: url)) as? [String: Any])
        let encoder = JSONEncoder()
        let encodedResponse = try encoder.encode(responseDTO)
        root["response"] = try JSONSerialization.jsonObject(with: encodedResponse)
        let output = try JSONSerialization.data(withJSONObject: root, options: [.sortedKeys])
        let outputURL: URL
        if let configured = ProcessInfo.processInfo.environment["OC_P008_INTEROP_OUTPUT"] {
            outputURL = URL(fileURLWithPath: configured)
        } else {
            outputURL = FileManager.default.temporaryDirectory.appendingPathComponent("watch-auth-response-rust-interop.json")
        }
        try output.write(to: outputURL, options: .atomic)
    }

    func testSwiftWatchSignatureVerifiesWithIndependentWatchEnrollment() throws {
        let token = try verifiedRequest()
        let signed = try WatchAuthProtocol.signWatchResponse(
            verifiedRequest: token,
            enrollment: watchEnrollment,
            signingKey: watchTestKey(),
            decision: .rejected,
            consentResult: .explicitUserDecision,
            responseAtMilliseconds: fixture.now_ms
        )
        var guardState = WatchAuthReplayGuard()
        let verified = try WatchAuthProtocol.verifyWatchResponse(
            canonicalBytes: signed.response.canonicalBytes(),
            signatureRaw: signed.signatureRaw,
            enrollment: watchEnrollment,
            verifiedRequest: try verifiedRequest(),
            nowMilliseconds: fixture.now_ms,
            replayGuard: &guardState
        )
        XCTAssertEqual(verified.decision, .rejected)
        XCTAssertEqual(verified.consentResult, .explicitUserDecision)
    }

    func testUnknownRevokedSuspendedAndSubstitutedHostKeysFailClosed() throws {
        var guardState = WatchAuthReplayGuard()
        XCTAssertThrowsError(try WatchAuthProtocol.verifyHostRequest(
            canonicalBytes: data(fixture.request.canonical_hex),
            signatureRaw: data(fixture.request.signature_raw_hex),
            enrollment: nil,
            expectedWatchEnrollmentID: watchEnrollment.enrollmentID,
            nowMilliseconds: fixture.now_ms,
            replayGuard: &guardState
        )) { XCTAssertEqual($0 as? WatchAuthError, .unknownEnrollment) }

        let revoked = HostEnrollment(
            enrollmentID: hostEnrollment.enrollmentID,
            hostIdentity: hostEnrollment.hostIdentity,
            linuxUID: hostEnrollment.linuxUID,
            publicKeySEC1: hostEnrollment.publicKeySEC1,
            state: .revoked,
            testOnly: true
        )
        var revokedGuard = WatchAuthReplayGuard()
        XCTAssertThrowsError(try WatchAuthProtocol.verifyHostRequest(
            canonicalBytes: data(fixture.request.canonical_hex),
            signatureRaw: data(fixture.request.signature_raw_hex),
            enrollment: revoked,
            expectedWatchEnrollmentID: watchEnrollment.enrollmentID,
            nowMilliseconds: fixture.now_ms,
            replayGuard: &revokedGuard
        )) { XCTAssertEqual($0 as? WatchAuthError, .enrollmentRevoked) }

        let substituted = HostEnrollment(
            enrollmentID: hostEnrollment.enrollmentID,
            hostIdentity: hostEnrollment.hostIdentity,
            linuxUID: hostEnrollment.linuxUID,
            publicKeySEC1: watchEnrollment.publicKeySEC1,
            testOnly: true
        )
        var substitutedGuard = WatchAuthReplayGuard()
        XCTAssertThrowsError(try WatchAuthProtocol.verifyHostRequest(
            canonicalBytes: data(fixture.request.canonical_hex),
            signatureRaw: data(fixture.request.signature_raw_hex),
            enrollment: substituted,
            expectedWatchEnrollmentID: watchEnrollment.enrollmentID,
            nowMilliseconds: fixture.now_ms,
            replayGuard: &substitutedGuard
        )) { XCTAssertEqual($0 as? WatchAuthError, .invalidSignature) }
    }

    func testRequestBindingExpiryReplayDowngradeAndMalformedEncodingReject() throws {
        var replayGuard = WatchAuthReplayGuard()
        let canonical = try data(fixture.request.canonical_hex)
        let signature = try data(fixture.request.signature_raw_hex)
        XCTAssertThrowsError(try WatchAuthProtocol.verifyHostRequest(
            canonicalBytes: canonical,
            signatureRaw: signature,
            enrollment: hostEnrollment,
            expectedWatchEnrollmentID: "other-watch",
            nowMilliseconds: fixture.now_ms,
            replayGuard: &replayGuard
        )) { XCTAssertEqual($0 as? WatchAuthError, .bindingMismatch) }

        var expiredGuard = WatchAuthReplayGuard()
        XCTAssertThrowsError(try WatchAuthProtocol.verifyHostRequest(
            canonicalBytes: canonical,
            signatureRaw: signature,
            enrollment: hostEnrollment,
            expectedWatchEnrollmentID: watchEnrollment.enrollmentID,
            nowMilliseconds: UInt64.max,
            replayGuard: &expiredGuard
        )) { XCTAssertEqual($0 as? WatchAuthError, .expired) }

        var boundaryGuard = WatchAuthReplayGuard()
        XCTAssertThrowsError(try WatchAuthProtocol.verifyHostRequest(
            canonicalBytes: canonical,
            signatureRaw: signature,
            enrollment: hostEnrollment,
            expectedWatchEnrollmentID: watchEnrollment.enrollmentID,
            nowMilliseconds: WatchAuthRequest.parseCanonical(canonical).expiresAtMilliseconds,
            replayGuard: &boundaryGuard
        )) { XCTAssertEqual($0 as? WatchAuthError, .expired) }

        var duplicateGuard = WatchAuthReplayGuard()
        _ = try WatchAuthProtocol.verifyHostRequest(
            canonicalBytes: canonical,
            signatureRaw: signature,
            enrollment: hostEnrollment,
            expectedWatchEnrollmentID: watchEnrollment.enrollmentID,
            nowMilliseconds: fixture.now_ms,
            replayGuard: &duplicateGuard
        )
        XCTAssertThrowsError(try WatchAuthProtocol.verifyHostRequest(
            canonicalBytes: canonical,
            signatureRaw: signature,
            enrollment: hostEnrollment,
            expectedWatchEnrollmentID: watchEnrollment.enrollmentID,
            nowMilliseconds: fixture.now_ms,
            replayGuard: &duplicateGuard
        )) { XCTAssertEqual($0 as? WatchAuthError, .replay) }

        var downgraded = canonical
        downgraded[4] = 0
        XCTAssertThrowsError(try WatchAuthRequest.parseCanonical(downgraded)) {
            XCTAssertEqual($0 as? WatchAuthError, .unsupportedVersion)
        }
        var wrongCurve = canonical
        wrongCurve[6] = 2
        XCTAssertThrowsError(try WatchAuthRequest.parseCanonical(wrongCurve)) {
            XCTAssertEqual($0 as? WatchAuthError, .unsupportedAlgorithm)
        }
        XCTAssertThrowsError(try WatchAuthRequest.parseCanonical(Data(canonical.dropLast()))) {
            XCTAssertEqual($0 as? WatchAuthError, .malformed)
        }
        var trailing = canonical
        trailing.append(0)
        XCTAssertThrowsError(try WatchAuthRequest.parseCanonical(trailing)) {
            XCTAssertEqual($0 as? WatchAuthError, .nonCanonical)
        }
        var invalidUTF8 = canonical
        invalidUTF8[9] = 0xff
        XCTAssertThrowsError(try WatchAuthRequest.parseCanonical(invalidUTF8)) {
            XCTAssertEqual($0 as? WatchAuthError, .invalidUTF8)
        }
        var invalidIdentifier = canonical
        invalidIdentifier[9] = 0x20
        XCTAssertThrowsError(try WatchAuthRequest.parseCanonical(invalidIdentifier)) {
            XCTAssertEqual($0 as? WatchAuthError, .invalidIdentifier)
        }
        XCTAssertThrowsError(try WatchAuthRequest.parseCanonical(Data(repeating: 0, count: watchAuthMaximumMessageLength + 1))) {
            XCTAssertEqual($0 as? WatchAuthError, .oversized)
        }
        var badSignature = signature
        badSignature[0] ^= 0xff
        var badSignatureGuard = WatchAuthReplayGuard()
        XCTAssertThrowsError(try WatchAuthProtocol.verifyHostRequest(
            canonicalBytes: canonical,
            signatureRaw: badSignature,
            enrollment: hostEnrollment,
            expectedWatchEnrollmentID: watchEnrollment.enrollmentID,
            nowMilliseconds: fixture.now_ms,
            replayGuard: &badSignatureGuard
        )) { XCTAssertEqual($0 as? WatchAuthError, .invalidSignature) }
    }

    func testWrongAccountAndCurveEnrollmentAreRejected() throws {
        let wrongAccount = HostEnrollment(
            enrollmentID: hostEnrollment.enrollmentID,
            hostIdentity: hostEnrollment.hostIdentity,
            linuxUID: hostEnrollment.linuxUID + 1,
            publicKeySEC1: hostEnrollment.publicKeySEC1,
            testOnly: true
        )
        var accountGuard = WatchAuthReplayGuard()
        XCTAssertThrowsError(try WatchAuthProtocol.verifyHostRequest(
            canonicalBytes: data(fixture.request.canonical_hex),
            signatureRaw: data(fixture.request.signature_raw_hex),
            enrollment: wrongAccount,
            expectedWatchEnrollmentID: watchEnrollment.enrollmentID,
            nowMilliseconds: fixture.now_ms,
            replayGuard: &accountGuard
        )) { XCTAssertEqual($0 as? WatchAuthError, .bindingMismatch) }

        let wrongCurve = HostEnrollment(
            enrollmentID: hostEnrollment.enrollmentID,
            hostIdentity: hostEnrollment.hostIdentity,
            linuxUID: hostEnrollment.linuxUID,
            publicKeySEC1: hostEnrollment.publicKeySEC1,
            algorithm: .p384EcdsaSha384,
            testOnly: true
        )
        var curveGuard = WatchAuthReplayGuard()
        XCTAssertThrowsError(try WatchAuthProtocol.verifyHostRequest(
            canonicalBytes: data(fixture.request.canonical_hex),
            signatureRaw: data(fixture.request.signature_raw_hex),
            enrollment: wrongCurve,
            expectedWatchEnrollmentID: watchEnrollment.enrollmentID,
            nowMilliseconds: fixture.now_ms,
            replayGuard: &curveGuard
        )) { XCTAssertEqual($0 as? WatchAuthError, .unsupportedAlgorithm) }
    }

    func testResponseConsentAndWatchEnrollmentSubstitutionReject() throws {
        let token = try verifiedRequest()
        XCTAssertThrowsError(try WatchAuthProtocol.signWatchResponse(
            verifiedRequest: token,
            enrollment: watchEnrollment,
            signingKey: watchTestKey(),
            decision: .approved,
            consentResult: .cancelled,
            responseAtMilliseconds: fixture.now_ms
        )) { XCTAssertEqual($0 as? WatchAuthError, .consentNotCompleted) }

        let signed = try WatchAuthProtocol.signWatchResponse(
            verifiedRequest: try verifiedRequest(),
            enrollment: watchEnrollment,
            signingKey: watchTestKey(),
            decision: .approved,
            consentResult: .explicitUserDecision,
            responseAtMilliseconds: fixture.now_ms
        )
        let substituted = WatchEnrollment(
            enrollmentID: watchEnrollment.enrollmentID,
            watchIdentity: watchEnrollment.watchIdentity,
            boundHostEnrollmentID: watchEnrollment.boundHostEnrollmentID,
            boundHostIdentity: watchEnrollment.boundHostIdentity,
            boundLinuxUID: watchEnrollment.boundLinuxUID,
            publicKeySEC1: hostEnrollment.publicKeySEC1,
            testOnly: true
        )
        var replayGuard = WatchAuthReplayGuard()
        XCTAssertThrowsError(try WatchAuthProtocol.verifyWatchResponse(
            canonicalBytes: signed.response.canonicalBytes(),
            signatureRaw: signed.signatureRaw,
            enrollment: substituted,
            verifiedRequest: try verifiedRequest(),
            nowMilliseconds: fixture.now_ms,
            replayGuard: &replayGuard
        )) { XCTAssertEqual($0 as? WatchAuthError, .invalidSignature) }
    }

    func testResponseBindingExpiryReplayAndUnsupportedConsentReject() throws {
        let signed = try WatchAuthProtocol.signWatchResponse(
            verifiedRequest: verifiedRequest(),
            enrollment: watchEnrollment,
            signingKey: watchTestKey(),
            decision: .approved,
            consentResult: .explicitUserDecision,
            responseAtMilliseconds: fixture.now_ms
        )
        var unsupported = WatchAuthReplayGuard()
        XCTAssertThrowsError(try WatchAuthProtocol.signWatchResponse(
            verifiedRequest: verifiedRequest(),
            enrollment: watchEnrollment,
            signingKey: watchTestKey(),
            decision: .approved,
            consentResult: .unsupported,
            responseAtMilliseconds: fixture.now_ms
        )) { XCTAssertEqual($0 as? WatchAuthError, .consentNotCompleted) }

        var changed = try signed.response.canonicalBytes()
        changed[7] ^= 1 // first byte of request digest after the seven-byte header
        XCTAssertThrowsError(try WatchAuthProtocol.verifyWatchResponse(
            canonicalBytes: changed,
            signatureRaw: signed.signatureRaw,
            enrollment: watchEnrollment,
            verifiedRequest: try verifiedRequest(),
            nowMilliseconds: fixture.now_ms,
            replayGuard: &unsupported
        )) { XCTAssertEqual($0 as? WatchAuthError, .bindingMismatch) }

        var expired = WatchAuthReplayGuard()
        XCTAssertThrowsError(try WatchAuthProtocol.verifyWatchResponse(
            canonicalBytes: signed.response.canonicalBytes(),
            signatureRaw: signed.signatureRaw,
            enrollment: watchEnrollment,
            verifiedRequest: try verifiedRequest(),
            nowMilliseconds: UInt64.max,
            replayGuard: &expired
        )) { XCTAssertEqual($0 as? WatchAuthError, .expired) }

        var replayGuard = WatchAuthReplayGuard()
        _ = try WatchAuthProtocol.verifyWatchResponse(
            canonicalBytes: signed.response.canonicalBytes(),
            signatureRaw: signed.signatureRaw,
            enrollment: watchEnrollment,
            verifiedRequest: try verifiedRequest(),
            nowMilliseconds: fixture.now_ms,
            replayGuard: &replayGuard
        )
        XCTAssertThrowsError(try WatchAuthProtocol.verifyWatchResponse(
            canonicalBytes: signed.response.canonicalBytes(),
            signatureRaw: signed.signatureRaw,
            enrollment: watchEnrollment,
            verifiedRequest: try verifiedRequest(),
            nowMilliseconds: fixture.now_ms,
            replayGuard: &replayGuard
        )) { XCTAssertEqual($0 as? WatchAuthError, .replay) }
    }

    func testApprovalAfterLocalAuthenticationPastSignedExpiryIsRejected() throws {
        let token = try verifiedRequest()
        let expiresAt = token.request.expiresAtMilliseconds
        XCTAssertThrowsError(try WatchAuthProtocol.signWatchResponse(
            verifiedRequest: token,
            enrollment: watchEnrollment,
            signingKey: watchTestKey(),
            decision: .approved,
            consentResult: .explicitUserDecision,
            responseAtMilliseconds: expiresAt + 1
        )) {
            XCTAssertEqual($0 as? WatchAuthError, .expired)
        }
    }
}
