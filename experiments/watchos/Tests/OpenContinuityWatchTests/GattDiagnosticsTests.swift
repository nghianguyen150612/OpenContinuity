import XCTest
import OpenContinuityProtocol

final class GattDiagnosticsTests: XCTestCase {
    func testDisabledByDefaultAndOptOutDiscards() {
        var trace = GattDiagnostics()
        trace.record(.connected)
        XCTAssertTrue(trace.events.isEmpty)
        trace.setEnabled(true)
        trace.record(.challengeLength(25))
        XCTAssertEqual(trace.events, ["challenge_length expected=25 actual=25"])
        trace.setEnabled(false)
        XCTAssertTrue(trace.events.isEmpty)
    }
    func testSharedRustSimulatedTimingVector() throws {
        let url = Bundle.module.url(forResource: "p010-diagnostic-vector", withExtension: "json", subdirectory: "Fixtures")!
        let vector = try JSONSerialization.jsonObject(with: Data(contentsOf: url)) as! [String: Any]
        var trace = GattDiagnostics()
        trace.setEnabled(true)
        trace.begin(.cleanup, now: (vector["start_ms"] as! NSNumber).uint64Value)
        trace.measure(.cleanup, now: (vector["finish_ms"] as! NSNumber).uint64Value,
                      wall: (vector["wall_ms"] as! NSNumber).uint64Value)
        XCTAssertEqual(trace.samples[0].elapsedMs, (vector["elapsed_ms"] as! NSNumber).uint64Value)
        XCTAssertEqual(vector["source"] as? String, "simulated")
        XCTAssertEqual(vector["authorization"] as? String, "never_evaluated")
    }
    func testDeterministicTimingAndBoundedAggregates() throws {
        var trace = GattDiagnostics()
        trace.begin(.getReadAck, now: 10)
        trace.measure(.getReadAck, now: 20, wall: 100)
        XCTAssertTrue(trace.samples.isEmpty)
        trace.setEnabled(true)
        for _ in 0..<218 {
            trace.begin(.getReadAck, now: 10)
            trace.measure(.getReadAck, expected: 20, actual: 20, now: 35, wall: 100)
        }
        XCTAssertEqual(trace.samples.count, 64)
        XCTAssertEqual(trace.aggregates["get_read_ack"]?.count, 218)
        XCTAssertEqual(trace.aggregates["get_read_ack"]?.totalMs, 5450)
        XCTAssertEqual(trace.aggregates["get_read_ack"]?.maximumMs, 25)
        XCTAssertEqual(trace.samples.first?.timestampUnixMs, 100)
        let doc = try JSONSerialization.jsonObject(with: Data(trace.exportJSON().utf8)) as! [String: Any]
        XCTAssertEqual(doc["source"] as? String, "simulated")
        XCTAssertEqual(doc["authorization"] as? String, "never_evaluated")
        XCTAssertEqual(doc["kind"] as? String, "diagnostic_trace")
        XCTAssertEqual(doc["schemaVersion"] as? Int, 2)
        XCTAssertEqual((doc["trialId"] as? String)?.count, 32)
        let exportedSample = (doc["samples"] as! [[String: Any]]).first!
        XCTAssertEqual((exportedSample["elapsedMs"] as? NSNumber)?.uint64Value, 25)
        XCTAssertNil(exportedSample["timestampUnixMs"])
        XCTAssertEqual(doc["incomplete"] as? Bool, true)
        trace.setEnabled(false)
        XCTAssertTrue(trace.samples.isEmpty)
        XCTAssertTrue(trace.aggregates.isEmpty)
    }
    func testDeadlineIsAllowlistedAndClearedOnDisable() {
        var trace = GattDiagnostics()
        trace.setEnabled(true)
        trace.record(.deadlineExpired)
        XCTAssertEqual(trace.events, ["deadline_expired"])
        trace.setEnabled(false)
        XCTAssertTrue(trace.events.isEmpty)
    }
    func testTerminalIsRecordedOnceAndMarksTheCaptureComplete() throws {
        var trace = GattDiagnostics()
        trace.setEnabled(true)
        trace.record(.scanStarted)
        trace.record(.sessionTerminal)
        trace.record(.sessionTerminal)
        let doc = try JSONSerialization.jsonObject(with: Data(trace.exportJSON().utf8)) as! [String: Any]
        let events = doc["events"] as! [[String: Any]]
        XCTAssertEqual(events.map { $0["event"] as? String }, ["scan_started", "session_terminal"])
        XCTAssertEqual(doc["incomplete"] as? Bool, false)
    }
    func testStaleRequestAndDecisionCategoriesContainNoIdentifiers() throws {
        var trace = GattDiagnostics()
        trace.setEnabled(true)
        trace.record(.staleCallbackRejected)
        trace.record(.verifiedRequestAccepted)
        trace.record(.consentApproved)
        let export = trace.exportJSON()
        XCTAssertTrue(export.contains("stale_callback_rejected"))
        XCTAssertTrue(export.contains("verified_request_accepted"))
        XCTAssertTrue(export.contains("consent_approved"))
        XCTAssertFalse(export.contains("identifier"))
        let doc = try JSONSerialization.jsonObject(with: Data(export.utf8)) as! [String: Any]
        let events = doc["events"] as! [[String: Any]]
        XCTAssertEqual(events.map { ($0["sequence"] as! NSNumber).intValue }, [0, 1, 2])
        XCTAssertTrue(events.allSatisfy { ($0["elapsedMs"] as! NSNumber).uint64Value <= 3_600_000 })
        XCTAssertEqual(GattDiagnostics.authorization, "authorization: never_evaluated")
    }
    func testTraceBoundAndMetadataClamping() {
        var trace = GattDiagnostics()
        trace.setEnabled(true)
        for index in 0..<100 { trace.record(.acknowledged(index)) }
        XCTAssertEqual(trace.events.count, 64)
        XCTAssertEqual(trace.events.first, "frame_ack=36")
        let exported = try! JSONSerialization.jsonObject(with: Data(trace.exportJSON().utf8)) as! [String: Any]
        XCTAssertEqual((exported["events"] as? [[String: Any]])?.count, 64)
        trace.record(.responseLength(Int.max))
        XCTAssertEqual(trace.events.last, "response_length expected=44_or_11 actual=65535")
        XCTAssertNotNil(trace.trialID)
        let trialID = trace.trialID
        trace.setEnabled(false)
        XCTAssertNil(trace.trialID)
        trace.setEnabled(true)
        XCTAssertNotEqual(trace.trialID, trialID)
        XCTAssertEqual(GattDiagnostics.authorization, "authorization: never_evaluated")
    }
}
