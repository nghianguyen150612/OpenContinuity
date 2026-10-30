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
    public enum Stage: String, Codable {
        case discovery, connection, services, writeLimit = "write_limit"
        case requestFrames = "request_frames", requestDelivery = "request_delivery"
        case getReadAck = "get_read_ack", writeAck = "write_ack"
        case hostSignature = "host_signature", consent, localAuthentication = "local_authentication"
        case responseDelivery = "response_delivery", cleanup, failure
        case challengeRead = "challenge_read", resultRead = "result_read", requestRead = "request_read"
    }
    public enum Outcome: String, Codable { case pass = "PASS", fail = "FAIL", blocked = "BLOCKED", notRun = "NOT RUN" }
    public struct Sample: Codable {
        public let stage: Stage
        public let timestampUnixMs: UInt64
        public let elapsedMs: UInt64
        public let outcome: Outcome
        public let expected: Int?
        public let actual: Int?
    }
    public struct Aggregate: Codable {
        public var count: UInt64 = 0
        public var totalMs: UInt64 = 0
        public var maximumMs: UInt64 = 0
    }
    public private(set) var aggregates: [String: Aggregate] = [:]
    public private(set) var samples: [Sample] = []
    private var starts: [Stage: UInt64] = [:]
    public mutating func begin(_ stage: Stage, now: UInt64 = DispatchTime.now().uptimeNanoseconds / 1_000_000) {
        guard enabled else { return }
        starts[stage] = now
    }
    /// Numeric metadata only. Limits are observed CoreBluetooth value limits, not negotiated ATT MTU.
    public mutating func measure(_ stage: Stage, outcome: Outcome = .pass,
                                 expected: Int? = nil, actual: Int? = nil,
                                 now: UInt64 = DispatchTime.now().uptimeNanoseconds / 1_000_000,
                                 wall: UInt64 = UInt64(max(0, Date().timeIntervalSince1970 * 1_000))) {
        guard enabled else { return }
        let start = starts.removeValue(forKey: stage) ?? now
        let elapsed = min(now >= start ? now - start : 0, 3_600_000)
        var aggregate = aggregates[stage.rawValue] ?? Aggregate()
        if aggregate.count < 65535 {
            aggregate.count += 1
            aggregate.totalMs += elapsed
            aggregate.maximumMs = max(aggregate.maximumMs, elapsed)
            aggregates[stage.rawValue] = aggregate
        }
        if samples.count == 64 { samples.removeFirst() }
        samples.append(Sample(stage: stage, timestampUnixMs: wall, elapsedMs: elapsed,
                              outcome: outcome, expected: expected.map { min(max($0, 0), 65535) },
                              actual: actual.map { min(max($0, 0), 65535) }))
    }
    /// A trace is supporting data, never a physical qualification verdict.
    public enum Source: String, Encodable { case physical, simulated, hostPreflight = "host_preflight" }
    public func exportJSON(source: Source = .simulated) -> String {
        struct Trace: Encodable {
            let schemaVersion = 1
            let kind = "diagnostic_trace"
            let authorization = "never_evaluated"
            let source: Source
            let aggregates: [String: Aggregate]
            let samples: [Sample]
        }
        let encoder = JSONEncoder()
        encoder.outputFormatting = [.sortedKeys]
        guard let data = try? encoder.encode(Trace(source: source, aggregates: aggregates, samples: samples)) else { return "" }
        return String(data: data, encoding: .utf8) ?? ""
    }
    public static let authorization = "authorization: never_evaluated"
    public private(set) var enabled = false
    public private(set) var events: [String] = []
    public init() {}
    public mutating func setEnabled(_ enabled: Bool) {
        self.enabled = enabled
        aggregates.removeAll()
        samples.removeAll()
        starts.removeAll()
        events.removeAll() // opting out discards the bounded in-memory trace
    }
    public mutating func record(_ event: Event) {
        guard enabled else { return }
        if events.count == 64 { events.removeFirst() }
        events.append(event.text)
    }
}
