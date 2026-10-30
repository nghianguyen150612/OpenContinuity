import Foundation

/// Allowlisted metadata only: never accepts device names, identifiers or bytes.
public struct GattDiagnostics {
    public enum Event {
        case scanStarted, scanStopped, discovered, connecting, connected
        case serviceDiscovered, characteristicsDiscovered, invalidated, disconnected
        case challengeLength(Int), responseLength(Int), frames(Int), acknowledged(Int)
        case digestVerified, versionVerified, controlAcknowledged, cancelled, failed
        fileprivate var text: String {
            func bounded(_ n: Int) -> Int { min(max(n, 0), 65535) }
            switch self {
            case .scanStarted: return "scan_started"
            case .scanStopped: return "scan_stopped"
            case .discovered: return "advertisement_discovered"
            case .connecting: return "connecting"
            case .connected: return "connected"
            case .serviceDiscovered: return "service_discovered"
            case .characteristicsDiscovered: return "five_characteristics_verified"
            case .invalidated: return "session_invalidated"
            case .disconnected: return "disconnected"
            case .challengeLength(let n): return "challenge_length expected=25 actual=\(bounded(n))"
            case .responseLength(let n): return "response_length expected=44_or_11 actual=\(bounded(n))"
            case .frames(let n): return "frame_count=\(bounded(n))"
            case .acknowledged(let n): return "frame_ack=\(bounded(n))"
            case .versionVerified: return "protocol_version_verified=1"
            case .controlAcknowledged: return "control_write_acknowledged"
            case .digestVerified: return "synthetic_digest_verified"
            case .cancelled: return "cancel_requested"
            case .failed: return "operation_failed"
            }
        }
    }
    public static let authorization = "authorization: never_evaluated"
    public private(set) var enabled = false
    public private(set) var events: [String] = []
    public init() {}
    public mutating func setEnabled(_ enabled: Bool) {
        self.enabled = enabled
        events.removeAll() // opting out discards the bounded in-memory trace
    }
    public mutating func record(_ event: Event) {
        guard enabled else { return }
        if events.count == 64 { events.removeFirst() }
        events.append(event.text)
    }
}
