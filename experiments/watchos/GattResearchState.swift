import Foundation

enum GattResearchState: Equatable {
    case idle, scanning, connecting, discovering, readingChallenge
    case ready, sendingFrames, awaitingResponse, completed, cancelled
    case readingAuthStatus, sendingAuthControl, readingAuthRequest
    case verifyingAuthRequest, awaitingResearchConsent, sendingAuthResponse
    case completedResearchDecision
    case disconnecting, disconnected, failed(String)

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
        case .readingAuthStatus: "Reading authenticated research status"
        case .sendingAuthControl: "Sending research transport control"
        case .readingAuthRequest: "Reading signed research request"
        case .verifyingAuthRequest: "Verifying signed research request"
        case .awaitingResearchConsent: "Awaiting explicit research decision"
        case .sendingAuthResponse: "Sending signed research response"
        case .completedResearchDecision: "Research decision sent"
        case .disconnecting: "Disconnecting"
        case .disconnected: "Disconnected"
        case .failed(let reason): "Failed: \(reason)"
        }
    }
}
