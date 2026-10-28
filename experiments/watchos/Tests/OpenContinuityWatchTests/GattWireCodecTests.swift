import XCTest
import Foundation
import OpenContinuityProtocol

final class GattWireCodecTests: XCTestCase {
    private struct Fixture: Decodable {
        let service, version_uuid, challenge_uuid, exchange_uuid, response_uuid, control_uuid: String
        let session_id, challenge, payload: String
        let frames: [String]
        let complete, cancel, success, cancel_response, error: String
    }

    private var fixture: Fixture {
        let url = Bundle.module.url(forResource: "gatt-v1", withExtension: "json", subdirectory: "Fixtures")!
        return try! JSONDecoder().decode(Fixture.self, from: Data(contentsOf: url))
    }

    private func data(_ hex: String) -> Data {
        Data(stride(from: 0, to: hex.count, by: 2).map { i in
            let a = hex.index(hex.startIndex, offsetBy: i)
            let b = hex.index(a, offsetBy: 2)
            return UInt8(hex[a..<b], radix: 16)!
        })
    }

    func testFixtureUUIDsChallengeControlAndFrames() throws {
        let f = fixture
        XCTAssertEqual([f.service, f.version_uuid, f.challenge_uuid, f.exchange_uuid, f.response_uuid, f.control_uuid], [
            GattUUIDs.service, GattUUIDs.version, GattUUIDs.challenge,
            GattUUIDs.exchange, GattUUIDs.response, GattUUIDs.control])
        try GattWireCodec.parseVersion(Data([1]))
        let challenge = try GattWireCodec.parseChallenge(data(f.challenge))
        XCTAssertEqual(challenge.sessionID, Array(data(f.session_id)))
        XCTAssertEqual(challenge.challenge, Array(0...15).map(UInt8.init))
        let frames = try GattWireCodec.frames(payload: data(f.payload), sessionID: challenge.sessionID)
        XCTAssertEqual(frames.map { $0.map { String(format: "%02x", $0) }.joined() }, f.frames)
        let first = try GattWireCodec.validateFrame(frames[0], sessionID: challenge.sessionID)
        let second = try GattWireCodec.validateFrame(frames[1], sessionID: challenge.sessionID)
        XCTAssertEqual(first.sequence, 0); XCTAssertTrue(first.start); XCTAssertFalse(first.end); XCTAssertEqual(first.totalLength, 10)
        XCTAssertEqual(second.sequence, 1); XCTAssertFalse(second.start); XCTAssertTrue(second.end); XCTAssertEqual(second.totalLength, 0)
        XCTAssertEqual(try GattWireCodec.control(sessionID: challenge.sessionID, cancel: false), data(f.complete))
        XCTAssertEqual(try GattWireCodec.control(sessionID: challenge.sessionID, cancel: true), data(f.cancel))
    }

    func testSuccessAndCancellationDigestVectors() throws {
        let f = fixture; let id = Array(data(f.session_id)); let payload = data(f.payload)
        try GattWireCodec.verifyResponse(data(f.success), sessionID: id, payload: payload, cancelled: false)
        try GattWireCodec.verifyResponse(data(f.cancel_response), sessionID: id, payload: payload, cancelled: true)
        XCTAssertThrowsError(try GattWireCodec.verifyResponse(data(f.error), sessionID: id, payload: payload, cancelled: false))
    }

    func testRejectsTruncationWrongVersionWrongSessionStatusAndDigest() throws {
        let f = fixture; let id = Array(data(f.session_id)); let payload = data(f.payload)
        XCTAssertThrowsError(try GattWireCodec.parseChallenge(data("01")))
        XCTAssertThrowsError(try GattWireCodec.parseChallenge(Data([2] + Array(data(f.challenge).dropFirst()))))
        XCTAssertThrowsError(try GattWireCodec.parseVersion(Data([2])))
        XCTAssertThrowsError(try GattWireCodec.verifyResponse(Data(data(f.success).prefix(43)), sessionID: id, payload: payload, cancelled: false))
        var wrongVersion = data(f.success); wrongVersion[0] = 2
        XCTAssertThrowsError(try GattWireCodec.verifyResponse(wrongVersion, sessionID: id, payload: payload, cancelled: false))
        var wrongLength = data(f.success); wrongLength[10] = 9
        XCTAssertThrowsError(try GattWireCodec.verifyResponse(wrongLength, sessionID: id, payload: payload, cancelled: false))
        XCTAssertThrowsError(try GattWireCodec.verifyResponse(data(f.success), sessionID: Array("76543210".utf8), payload: payload, cancelled: false))
        var wrongStatus = data(f.success); wrongStatus[1] = 3
        XCTAssertThrowsError(try GattWireCodec.verifyResponse(wrongStatus, sessionID: id, payload: payload, cancelled: false))
        var wrongDigest = data(f.success); wrongDigest[12] ^= 0xff
        XCTAssertThrowsError(try GattWireCodec.verifyResponse(wrongDigest, sessionID: id, payload: payload, cancelled: false))
        XCTAssertThrowsError(try GattWireCodec.verifyResponse(data(f.success), sessionID: id, payload: Data("different".utf8), cancelled: false))
    }

    func testStaleGenerationTokensAreRejected() {
        var gate = GattGenerationGate()
        let old = gate.advance()
        XCTAssertTrue(gate.accepts(old))
        let current = gate.advance()
        XCTAssertFalse(gate.accepts(old))
        XCTAssertTrue(gate.accepts(current))
    }

    func testFragmentBoundariesAndMalformedFrames() throws {
        let id = Array(data(fixture.session_id))
        XCTAssertThrowsError(try GattWireCodec.frames(payload: Data(), sessionID: id))
        XCTAssertThrowsError(try GattWireCodec.frames(payload: Data(repeating: 1, count: 1025), sessionID: id))
        XCTAssertThrowsError(try GattWireCodec.frames(payload: Data([1]), sessionID: [1]))
        let full = try GattWireCodec.frames(payload: Data(repeating: 0x61, count: 1024), sessionID: id)
        XCTAssertEqual(full.count, 205)
        XCTAssertEqual(try GattWireCodec.validateFrame(full.last!, sessionID: id).sequence, 204)
        var badFlags = full[0]; badFlags[12] = 4
        XCTAssertThrowsError(try GattWireCodec.validateFrame(badFlags, sessionID: id))
        var badVersion = full[0]; badVersion[2] = 2
        XCTAssertThrowsError(try GattWireCodec.validateFrame(badVersion, sessionID: id))
        var wrongSession = full[0]; wrongSession[3] ^= 1
        XCTAssertThrowsError(try GattWireCodec.validateFrame(wrongSession, sessionID: id))
        XCTAssertThrowsError(try GattWireCodec.validateFrame(Data(full[0].prefix(15)), sessionID: id))
    }
}
