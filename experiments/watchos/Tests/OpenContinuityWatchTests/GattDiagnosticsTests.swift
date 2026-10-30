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
    func testTraceBoundAndMetadataClamping() {
        var trace = GattDiagnostics()
        trace.setEnabled(true)
        for index in 0..<100 { trace.record(.acknowledged(index)) }
        XCTAssertEqual(trace.events.count, 64)
        XCTAssertEqual(trace.events.first, "frame_ack=36")
        trace.record(.responseLength(Int.max))
        XCTAssertEqual(trace.events.last, "response_length expected=44_or_11 actual=65535")
        XCTAssertEqual(GattDiagnostics.authorization, "authorization: never_evaluated")
    }
}
