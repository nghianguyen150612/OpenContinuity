import XCTest
import Foundation
import OpenContinuityProtocol

final class GattLifecycleTests: XCTestCase {
    func testScanDeniedDuringEveryOwnedOperation() throws {
        for operation in [GattLifecycle.Operation.connect, .services, .characteristics, .read("challenge"), .write("frame"), .read("response")] {
            var model = GattLifecycle()
            let id = UUID()
            XCTAssertTrue(model.connect(id))
            XCTAssertNotNil(model.begin(operation))
            XCTAssertFalse(model.canScan)
            XCTAssertFalse(model.connect(UUID()))
            XCTAssertNil(model.begin(.connect))
        }
    }

    func testSuspensionDrainsIdleCompletedCancelledAndFailedConnections() throws {
        // Completed, cancelled and ready have no pending operation but still own BLE.
        for pending in [nil as GattLifecycle.Operation?, .connect, .services, .read("challenge"), .write("frame"), .read("response")] {
            var model = GattLifecycle()
            let id = UUID()
            XCTAssertTrue(model.connect(id))
            if let pending { XCTAssertNotNil(model.begin(pending)) }
            model.terminate()
            XCTAssertTrue(model.retiring)
            XCTAssertFalse(model.canScan)
            XCTAssertNil(model.pending)
            XCTAssertTrue(model.disconnected(id))
            XCTAssertTrue(model.canScan)
            model.terminate() // idle/disconnected suspension is idempotent
            XCTAssertTrue(model.canScan)
        }
    }

    func testStaleReadWriteAndDuplicateCallbacksAreRejected() throws {
        for operation in [GattLifecycle.Operation.read("challenge"), .write("frame"), .read("response")] {
            var model = GattLifecycle()
            let id = UUID()
            XCTAssertTrue(model.connect(id))
            let old = try XCTUnwrap(model.begin(operation))
            model.terminate()
            XCTAssertFalse(model.complete(old, peripheral: id))
            XCTAssertTrue(model.disconnected(id))
            XCTAssertFalse(model.disconnected(id))
            XCTAssertTrue(model.connect(id)) // same peripheral, new ownership
            let new = try XCTUnwrap(model.begin(operation))
            XCTAssertFalse(model.complete(old, peripheral: id))
            XCTAssertFalse(model.complete(new, peripheral: UUID()))
            XCTAssertTrue(model.complete(new, peripheral: id))
            XCTAssertFalse(model.complete(new, peripheral: id))
        }
    }

    func testRapidActionsCancelTimeoutAndRepeatedCycles() throws {
        var model = GattLifecycle()
        let id = UUID()
        for _ in 0..<100 {
            XCTAssertTrue(model.connect(id))
            XCTAssertFalse(model.connect(id))
            let ticket = try XCTUnwrap(model.begin(.write("frame")))
            model.terminate() // cancel
            model.terminate() // timeout after cancel
            XCTAssertFalse(model.complete(ticket, peripheral: id))
            XCTAssertFalse(model.canScan)
            XCTAssertTrue(model.disconnected(id))
            XCTAssertTrue(model.canScan)
        }
    }
}
