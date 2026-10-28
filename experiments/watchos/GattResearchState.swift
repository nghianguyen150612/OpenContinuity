import Foundation

enum GattResearchState: Equatable {
    case idle, scanning, connecting, discovering, readingChallenge
    case ready, sendingFrames, awaitingResponse, completed, cancelled
    case disconnected, failed(String)

    var title: String {
        switch self {
        case .idle: "Idle"
        case .scanning: "Scanning"
        case .connecting: "Connecting"
        case .discovering: "Discovering GATT"
        case .readingChallenge: "Reading synthetic challenge"
        case .ready: "Ready for test"
        case .sendingFrames: "Sending synthetic frames"
        case .awaitingResponse: "Awaiting synthetic result"
        case .completed: "Transport test completed"
        case .cancelled: "Test cancelled"
        case .disconnected: "Disconnected"
        case .failed(let reason): "Failed: \(reason)"
        }
    }
}
