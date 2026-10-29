import CryptoKit
import Foundation
import XCTest
@testable import OpenContinuityProtocol

final class WatchAuthTransportTests: XCTestCase {
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

    private struct P008Fixture: Decodable {
        let schema_version: UInt8
        let now_ms: UInt64
        let host_enrollment: EnrollmentFixture
        let watch_enrollment: EnrollmentFixture
        let request: SignedFixture
    }

    private struct SignedFixture: Decodable {
        let canonical_hex: String
        let signature_raw_hex: String
    }

    private struct RustTransportFixture: Decodable {
        let schema_version: UInt8
        let transfer_id_hex: String
        let request_envelope_hex: String
        let request_frames_hex: [String]
    }

    private struct SwiftTransportFixture: Encodable {
        let schema_version: UInt8
        let transfer_id_hex: String
        let response_envelope_hex: String
        let response_frames_hex: [String]
    }

    private var p008Fixture: P008Fixture {
        let url = Bundle.module.url(forResource: "watch-auth-request-v1", withExtension: "json", subdirectory: "Fixtures")!
        return try! JSONDecoder().decode(P008Fixture.self, from: Data(contentsOf: url))
    }

    private func bytes(_ hex: String) throws -> Data {
        guard hex.count.isMultiple(of: 2), hex == hex.lowercased() else { throw WatchAuthError.nonCanonical }
        var result = Data()
        var index = hex.startIndex
        while index < hex.endIndex {
            let end = hex.index(index, offsetBy: 2)
            guard let byte = UInt8(hex[index..<end], radix: 16) else { throw WatchAuthError.nonCanonical }
            result.append(byte)
            index = end
        }
        return result
    }

    private func hex(_ data: Data) -> String {
        data.map { String(format: "%02x", $0) }.joined()
    }

    private var hostEnrollment: HostEnrollment {
        let value = p008Fixture.host_enrollment
        return HostEnrollment(
            enrollmentID: value.enrollment_id,
            hostIdentity: value.host_identity!,
            linuxUID: value.linux_uid!,
            publicKeySEC1: try! bytes(value.public_key_sec1_hex),
            algorithm: WatchKeyAlgorithm(rawValue: value.key_algorithm)!,
            keyVersion: value.key_version,
            state: WatchEnrollmentState(rawValue: value.state)!,
            testOnly: value.test_only
        )
    }

    private var watchEnrollment: WatchEnrollment {
        let value = p008Fixture.watch_enrollment
        return WatchEnrollment(
            enrollmentID: value.enrollment_id,
            watchIdentity: value.watch_identity!,
            boundHostEnrollmentID: value.bound_host_enrollment_id!,
            boundHostIdentity: value.bound_host_identity!,
            boundLinuxUID: value.bound_linux_uid!,
            publicKeySEC1: try! bytes(value.public_key_sec1_hex),
            algorithm: WatchKeyAlgorithm(rawValue: value.key_algorithm)!,
            keyVersion: value.key_version,
            state: WatchEnrollmentState(rawValue: value.state)!,
            testOnly: value.test_only
        )
    }

    private func watchTestKey() throws -> P256.Signing.PrivateKey {
        // Test-only scalar 2 matches the independent P008 fixture's Watch pin.
        try P256.Signing.PrivateKey(rawRepresentation: Data(repeating: 0x02, count: 32))
    }

    private func verifyHost(_ envelope: WatchSignedEnvelope) throws -> VerifiedHostRequest {
        var replay = WatchAuthReplayGuard()
        return try WatchAuthProtocol.verifyHostTransportEnvelope(
            bytes: envelope.bytes,
            enrollment: hostEnrollment,
            expectedWatchEnrollmentID: watchEnrollment.enrollmentID,
            nowMilliseconds: p008Fixture.now_ms,
            replayGuard: &replay
        )
    }

    func testSwiftCodecRoundTripsBothDirectionsAndRejectsMalformedFrames() throws {
        let fixture = p008Fixture
        let requestCanonical = try bytes(fixture.request.canonical_hex)
        let requestSignature = try bytes(fixture.request.signature_raw_hex)
        let request = try WatchSignedEnvelope(direction: .hostToWatch, canonical: requestCanonical, signatureRaw: requestSignature)
        let transferID = Data(repeating: 0xa5, count: 8)
        let frames = try WatchAuthTransportCodec.frames(envelope: request, transferID: transferID)
        XCTAssertEqual(frames.first?.count, 20)

        var assembler = try WatchTransferAssembler(transferID: transferID, direction: .hostToWatch, owner: "watch-client", createdAtMilliseconds: fixture.now_ms)
        for (index, frame) in frames.enumerated() {
            XCTAssertEqual(Array(frame)[11], UInt8(index))
            try assembler.push(owner: "watch-client", direction: .hostToWatch, frame: frame, nowMilliseconds: fixture.now_ms)
        }
        let recovered = try assembler.complete(owner: "watch-client", transferID: transferID, nowMilliseconds: fixture.now_ms)
        XCTAssertEqual(recovered, request)
        let verified = try verifyHost(recovered)
        XCTAssertNoThrow(try WatchAuthProtocol.validateWatchResponseEligibility(
            verifiedRequest: verified,
            enrollment: watchEnrollment,
            signingKey: watchTestKey()
        ))
        let wrongWatchKey = try P256.Signing.PrivateKey(rawRepresentation: Data(repeating: 0x01, count: 32))
        XCTAssertThrowsError(try WatchAuthProtocol.validateWatchResponseEligibility(
            verifiedRequest: verified,
            enrollment: watchEnrollment,
            signingKey: wrongWatchKey
        )) {
            XCTAssertEqual($0 as? WatchAuthError, .bindingMismatch)
        }

        let signedResponse = try WatchAuthProtocol.signWatchResponse(
            verifiedRequest: verified,
            enrollment: watchEnrollment,
            signingKey: watchTestKey(),
            decision: .rejected,
            consentResult: .explicitUserDecision,
            responseAtMilliseconds: fixture.now_ms
        )
        let response = try WatchSignedEnvelope(
            direction: .watchToHost,
            canonical: signedResponse.response.canonicalBytes(),
            signatureRaw: signedResponse.signatureRaw
        )
        let responseFrames = try WatchAuthTransportCodec.frames(envelope: response, transferID: transferID)
        var responseAssembler = try WatchTransferAssembler(transferID: transferID, direction: .watchToHost, owner: "linux-client", createdAtMilliseconds: fixture.now_ms)
        for frame in responseFrames {
            try responseAssembler.push(owner: "linux-client", direction: .watchToHost, frame: frame, nowMilliseconds: fixture.now_ms)
        }
        let recoveredResponse = try responseAssembler.complete(owner: "linux-client", transferID: transferID, nowMilliseconds: fixture.now_ms)
        XCTAssertEqual(recoveredResponse, response)
        var responseReplay = WatchAuthReplayGuard()
        let verifiedResponse = try WatchAuthProtocol.verifyWatchTransportEnvelope(
            bytes: recoveredResponse.bytes,
            enrollment: watchEnrollment,
            verifiedRequest: verified,
            nowMilliseconds: fixture.now_ms,
            replayGuard: &responseReplay
        )
        XCTAssertEqual(verifiedResponse.decision, .rejected)

        var corrupt = frames[0]
        corrupt[2] = 2
        var badVersion = try WatchTransferAssembler(transferID: transferID, direction: .hostToWatch, owner: "watch-client", createdAtMilliseconds: fixture.now_ms)
        XCTAssertThrowsError(try badVersion.push(owner: "watch-client", direction: .hostToWatch, frame: corrupt, nowMilliseconds: fixture.now_ms)) {
            XCTAssertEqual($0 as? WatchTransportError, .versionMismatch)
        }
        var wrongOwner = try WatchTransferAssembler(transferID: transferID, direction: .hostToWatch, owner: "watch-client", createdAtMilliseconds: fixture.now_ms)
        XCTAssertThrowsError(try wrongOwner.push(owner: "other-client", direction: .hostToWatch, frame: frames[0], nowMilliseconds: fixture.now_ms)) {
            XCTAssertEqual($0 as? WatchTransportError, .ownerMismatch)
        }
        var wrongSession = try WatchTransferAssembler(transferID: Data(repeating: 0xb6, count: 8), direction: .hostToWatch, owner: "watch-client", createdAtMilliseconds: fixture.now_ms)
        XCTAssertThrowsError(try wrongSession.push(owner: "watch-client", direction: .hostToWatch, frame: frames[0], nowMilliseconds: fixture.now_ms)) {
            XCTAssertEqual($0 as? WatchTransportError, .sessionMismatch)
        }
        XCTAssertThrowsError(try WatchSignedEnvelope(direction: .hostToWatch, bytes: Data(request.bytes.dropLast()))) {
            XCTAssertEqual($0 as? WatchTransportError, .invalidEnvelope)
        }
    }

    func testReadCursorAdvancesOnlyAfterExplicitAcknowledgement() throws {
        let now = p008Fixture.now_ms
        let transferID = Data(repeating: 0x31, count: 8)
        var cursor = try WatchRequestReadCursor(transferID: transferID, owner: "watch-client", totalFrameCount: 2, nowMilliseconds: now)
        try cursor.requested(owner: "watch-client", transferID: transferID, index: 0, nowMilliseconds: now)
        XCTAssertEqual(cursor.index, 0)
        XCTAssertThrowsError(try cursor.requested(owner: "watch-client", transferID: transferID, index: 1, nowMilliseconds: now)) {
            XCTAssertEqual($0 as? WatchTransportError, .duplicateOrOutOfOrder)
        }
        XCTAssertThrowsError(try cursor.acknowledge(owner: "watch-client", transferID: transferID, index: 0, nowMilliseconds: now)) {
            XCTAssertEqual($0 as? WatchTransportError, .duplicateOrOutOfOrder)
        }
        try cursor.didRead(owner: "watch-client", transferID: transferID, index: 0, nowMilliseconds: now)
        try cursor.acknowledge(owner: "watch-client", transferID: transferID, index: 0, nowMilliseconds: now)
        try cursor.requested(owner: "watch-client", transferID: transferID, index: 1, nowMilliseconds: now)
        try cursor.didRead(owner: "watch-client", transferID: transferID, index: 1, nowMilliseconds: now)
        try cursor.acknowledge(owner: "watch-client", transferID: transferID, index: 1, nowMilliseconds: now)
        try cursor.complete(owner: "watch-client", transferID: transferID, nowMilliseconds: now)
        XCTAssertThrowsError(try cursor.complete(owner: "watch-client", transferID: transferID, nowMilliseconds: now))
    }

    /// CI supplies bytes emitted by the Rust framing implementation and reads
    /// this output with the Rust response verifier. Local runs may omit these
    /// environment paths while still exercising the deterministic Swift tests.
    func testRustRequestSwiftReassemblyAndSignedResponseRustReassembly() throws {
        guard let inputPath = ProcessInfo.processInfo.environment["OC_P009_TRANSPORT_INPUT"] else {
            throw XCTSkip("cross-language P009 transport fixture is produced by the macOS CI workflow")
        }
        let input = try JSONDecoder().decode(RustTransportFixture.self, from: Data(contentsOf: URL(fileURLWithPath: inputPath)))
        XCTAssertEqual(input.schema_version, 1)
        let transferID = try bytes(input.transfer_id_hex)
        let expectedEnvelope = try WatchSignedEnvelope(direction: .hostToWatch, bytes: bytes(input.request_envelope_hex))
        var assembler = try WatchTransferAssembler(transferID: transferID, direction: .hostToWatch, owner: "watch-ci", createdAtMilliseconds: p008Fixture.now_ms)
        for frameHex in input.request_frames_hex {
            try assembler.push(owner: "watch-ci", direction: .hostToWatch, frame: bytes(frameHex), nowMilliseconds: p008Fixture.now_ms)
        }
        let request = try assembler.complete(owner: "watch-ci", transferID: transferID, nowMilliseconds: p008Fixture.now_ms)
        XCTAssertEqual(request, expectedEnvelope)

        var truncated = try WatchTransferAssembler(transferID: transferID, direction: .hostToWatch, owner: "watch-ci", createdAtMilliseconds: p008Fixture.now_ms)
        for frameHex in input.request_frames_hex.dropLast() {
            try truncated.push(owner: "watch-ci", direction: .hostToWatch, frame: bytes(frameHex), nowMilliseconds: p008Fixture.now_ms)
        }
        XCTAssertThrowsError(try truncated.complete(owner: "watch-ci", transferID: transferID, nowMilliseconds: p008Fixture.now_ms)) {
            XCTAssertEqual($0 as? WatchTransportError, .incompleteTransfer)
        }

        var staleSession = try WatchTransferAssembler(transferID: Data(repeating: 0x11, count: 8), direction: .hostToWatch, owner: "watch-ci", createdAtMilliseconds: p008Fixture.now_ms)
        XCTAssertThrowsError(try staleSession.push(owner: "watch-ci", direction: .hostToWatch, frame: bytes(input.request_frames_hex[0]), nowMilliseconds: p008Fixture.now_ms)) {
            XCTAssertEqual($0 as? WatchTransportError, .sessionMismatch)
        }

        var tamperedFrames = try input.request_frames_hex.map(bytes)
        let lastFrame = tamperedFrames.count - 1
        tamperedFrames[lastFrame][tamperedFrames[lastFrame].count - 1] ^= 1
        var tamperedAssembler = try WatchTransferAssembler(transferID: transferID, direction: .hostToWatch, owner: "watch-ci", createdAtMilliseconds: p008Fixture.now_ms)
        for frame in tamperedFrames {
            try tamperedAssembler.push(owner: "watch-ci", direction: .hostToWatch, frame: frame, nowMilliseconds: p008Fixture.now_ms)
        }
        let tamperedRequest = try tamperedAssembler.complete(owner: "watch-ci", transferID: transferID, nowMilliseconds: p008Fixture.now_ms)
        var tamperedReplay = WatchAuthReplayGuard()
        XCTAssertThrowsError(try WatchAuthProtocol.verifyHostTransportEnvelope(
            bytes: tamperedRequest.bytes,
            enrollment: hostEnrollment,
            expectedWatchEnrollmentID: watchEnrollment.enrollmentID,
            nowMilliseconds: p008Fixture.now_ms,
            replayGuard: &tamperedReplay
        ))

        let wrongHostPin = HostEnrollment(
            enrollmentID: hostEnrollment.enrollmentID,
            hostIdentity: hostEnrollment.hostIdentity,
            linuxUID: hostEnrollment.linuxUID,
            publicKeySEC1: watchEnrollment.publicKeySEC1,
            algorithm: hostEnrollment.algorithm,
            keyVersion: hostEnrollment.keyVersion,
            state: hostEnrollment.state,
            testOnly: hostEnrollment.testOnly
        )
        var wrongHostReplay = WatchAuthReplayGuard()
        XCTAssertThrowsError(try WatchAuthProtocol.verifyHostTransportEnvelope(
            bytes: request.bytes,
            enrollment: wrongHostPin,
            expectedWatchEnrollmentID: watchEnrollment.enrollmentID,
            nowMilliseconds: p008Fixture.now_ms,
            replayGuard: &wrongHostReplay
        ))
        let verifiedRequest = try verifyHost(request)

        // This is a deterministic explicit choice in a software test; it does
        // not simulate or claim successful physical LocalAuthentication.
        let signed = try WatchAuthProtocol.signWatchResponse(
            verifiedRequest: verifiedRequest,
            enrollment: watchEnrollment,
            signingKey: watchTestKey(),
            decision: .rejected,
            consentResult: .explicitUserDecision,
            responseAtMilliseconds: p008Fixture.now_ms
        )
        let response = try WatchSignedEnvelope(
            direction: .watchToHost,
            canonical: signed.response.canonicalBytes(),
            signatureRaw: signed.signatureRaw
        )
        let wrongWatchPin = WatchEnrollment(
            enrollmentID: watchEnrollment.enrollmentID,
            watchIdentity: watchEnrollment.watchIdentity,
            boundHostEnrollmentID: watchEnrollment.boundHostEnrollmentID,
            boundHostIdentity: watchEnrollment.boundHostIdentity,
            boundLinuxUID: watchEnrollment.boundLinuxUID,
            publicKeySEC1: hostEnrollment.publicKeySEC1,
            algorithm: watchEnrollment.algorithm,
            keyVersion: watchEnrollment.keyVersion,
            state: watchEnrollment.state,
            testOnly: watchEnrollment.testOnly
        )
        var wrongWatchReplay = WatchAuthReplayGuard()
        XCTAssertThrowsError(try WatchAuthProtocol.verifyWatchTransportEnvelope(
            bytes: response.bytes,
            enrollment: wrongWatchPin,
            verifiedRequest: verifiedRequest,
            nowMilliseconds: p008Fixture.now_ms,
            replayGuard: &wrongWatchReplay
        ))
        let responseFrames = try WatchAuthTransportCodec.frames(envelope: response, transferID: transferID)
        let output = SwiftTransportFixture(
            schema_version: 1,
            transfer_id_hex: input.transfer_id_hex,
            response_envelope_hex: hex(response.bytes),
            response_frames_hex: responseFrames.map(hex)
        )
        guard let outputPath = ProcessInfo.processInfo.environment["OC_P009_TRANSPORT_OUTPUT"] else {
            throw XCTSkip("OC_P009_TRANSPORT_OUTPUT is required for the Rust response verification step")
        }
        let encoded = try JSONEncoder().encode(output)
        try encoded.write(to: URL(fileURLWithPath: outputPath), options: .atomic)
    }
}
