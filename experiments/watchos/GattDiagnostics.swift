import Foundation

/// Allowlisted metadata only: never accepts device names, identifiers or bytes.
public struct GattDiagnostics {
    public enum Event {
        case scanStarted, scanStopped, discovered, connecting, connected
        case serviceDiscovered, characteristicsDiscovered, invalidated, disconnected
        case challengeLength(Int), responseLength(Int), frames(Int), acknowledged(Int)
        case digestVerified, versionVerified, controlAcknowledged, cancelled, deadlineExpired, failed
        case staleCallbackRejected, verifiedRequestAccepted, requestValidationFailed, sessionTerminal
        case consentApproved, consentRejected, consentCancelled, consentFailed
        case signedRequestFrameCount(Int), signedResponseFrameCount(Int)
        case signedRequestFrameAcknowledged(Int), signedResponseFrameAcknowledged(Int)
        case localAuthenticationSucceeded, localAuthenticationFailed, localAuthenticationUnsupported
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
            case .deadlineExpired: return "deadline_expired"
            case .failed: return "operation_failed"
            case .staleCallbackRejected: return "stale_callback_rejected"
            case .verifiedRequestAccepted: return "verified_request_accepted"
            case .requestValidationFailed: return "request_validation_failed"
            case .sessionTerminal: return "session_terminal"
            case .consentApproved: return "consent_approved"
            case .consentRejected: return "consent_rejected"
            case .consentCancelled: return "consent_cancelled"
            case .consentFailed: return "consent_failed"
            case .signedRequestFrameCount(let n): return "signed_request_frame_count=\(bounded(n))"
            case .signedResponseFrameCount(let n): return "signed_response_frame_count=\(bounded(n))"
            case .signedRequestFrameAcknowledged(let n): return "signed_request_frame_ack=\(bounded(n))"
            case .signedResponseFrameAcknowledged(let n): return "signed_response_frame_ack=\(bounded(n))"
            case .localAuthenticationSucceeded: return "local_authentication_succeeded"
            case .localAuthenticationFailed: return "local_authentication_failed"
            case .localAuthenticationUnsupported: return "local_authentication_unsupported"
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
    public struct EventRecord: Codable {
        public let sequence: UInt8
        public let elapsedMs: UInt64
        public let event: String
    }
    private struct ExportSample: Encodable {
        let stage: Stage
        let elapsedMs: UInt64
        let outcome: Outcome
        let expected: Int?
        let actual: Int?
    }
    public private(set) var aggregates: [String: Aggregate] = [:]
    public private(set) var samples: [Sample] = []
    public private(set) var eventRecords: [EventRecord] = []
    public private(set) var incomplete = false
    private var starts: [Stage: UInt64] = [:]
    private var diagnosticStartedAtMilliseconds: UInt64 = 0
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
        } else {
            incomplete = true
        }
        if samples.count == 64 {
            samples.removeFirst()
            incomplete = true
        }
        samples.append(Sample(stage: stage, timestampUnixMs: wall, elapsedMs: elapsed,
                              outcome: outcome, expected: expected.map { min(max($0, 0), 65535) },
                              actual: actual.map { min(max($0, 0), 65535) }))
    }
    /// A trace is supporting data, never a physical qualification verdict.
    public enum Source: String, Encodable { case physical, simulated, hostPreflight = "host_preflight" }
    public func exportJSON(source: Source = .simulated) -> String {
        struct Trace: Encodable {
            let schemaVersion = 2
            let kind = "diagnostic_trace"
            let authorization = "never_evaluated"
            let trialId: String
            let source: Source
            let incomplete: Bool
            let aggregates: [String: Aggregate]
            let samples: [ExportSample]
            let events: [EventRecord]
        }
        let encoder = JSONEncoder()
        encoder.outputFormatting = [.sortedKeys]
        let terminalPresent = eventRecords.last?.event == Event.sessionTerminal.text
        let scanStartPresent = eventRecords.contains { $0.event == Event.scanStarted.text }
        let exportedSamples = samples.map {
            ExportSample(stage: $0.stage, elapsedMs: $0.elapsedMs, outcome: $0.outcome,
                         expected: $0.expected, actual: $0.actual)
        }
        guard let trialID,
              let data = try? encoder.encode(Trace(trialId: trialID, source: source,
                                                   incomplete: incomplete || !terminalPresent || !scanStartPresent,
                                                   // A capture enabled mid-session cannot show the initial foreground scan boundary.
                                                   aggregates: aggregates, samples: exportedSamples,
                                                   events: eventRecords)),
              data.count <= 16 * 1024 else { return "" }
        return String(data: data, encoding: .utf8) ?? ""
    }
    public static let authorization = "authorization: never_evaluated"
    public private(set) var enabled = false
    public private(set) var events: [String] = []
    public private(set) var trialID: String?
    public init() {}
    public mutating func setEnabled(_ enabled: Bool) {
        self.enabled = enabled
        aggregates.removeAll()
        samples.removeAll()
        starts.removeAll()
        events.removeAll() // opting out discards the bounded in-memory trace
        eventRecords.removeAll()
        incomplete = false
        trialID = enabled ? UUID().uuidString.replacingOccurrences(of: "-", with: "").lowercased() : nil
        diagnosticStartedAtMilliseconds = enabled ? DispatchTime.now().uptimeNanoseconds / 1_000_000 : 0
    }
    public mutating func record(_ event: Event) {
        guard enabled else { return }
        if case .sessionTerminal = event, eventRecords.last?.event == Event.sessionTerminal.text {
            return
        }
        if events.count == 64 {
            events.removeFirst()
            incomplete = true
        }
        events.append(event.text)
        let now = DispatchTime.now().uptimeNanoseconds / 1_000_000
        let elapsed = min(now >= diagnosticStartedAtMilliseconds ? now - diagnosticStartedAtMilliseconds : 0, 3_600_000)
        if eventRecords.count == 64 {
            eventRecords.removeFirst()
            incomplete = true
        }
        eventRecords = eventRecords.enumerated().map { index, record in
            EventRecord(sequence: UInt8(index), elapsedMs: record.elapsedMs, event: record.event)
        }
        eventRecords.append(EventRecord(sequence: UInt8(eventRecords.count), elapsedMs: elapsed, event: event.text))
    }
}
