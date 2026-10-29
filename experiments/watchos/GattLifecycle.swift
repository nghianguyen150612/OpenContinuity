import Foundation

/// Pure ownership model. A terminal transport event must drain a connection
/// before a new scan/connection may begin, including the same peripheral ID.
public struct GattLifecycle {
    public enum Operation: Equatable { case connect, services, characteristics, read(String), write(String) }
    public struct Ticket: Equatable {
        public let generation: UInt64
        public let serial: UInt64
        public let operation: Operation
    }
    public private(set) var peripheral: UUID?
    public private(set) var retiring = false
    public private(set) var pending: Ticket?
    public private(set) var generation: UInt64 = 0
    private var serial: UInt64 = 0
    public init() {}
    public var canScan: Bool { peripheral == nil && !retiring }
    public mutating func connect(_ id: UUID) -> Bool {
        guard canScan else { return false }
        generation &+= 1; peripheral = id
        return true
    }
    public mutating func begin(_ operation: Operation) -> Ticket? {
        guard peripheral != nil, !retiring, pending == nil else { return nil }
        serial &+= 1
        let ticket = Ticket(generation: generation, serial: serial, operation: operation)
        pending = ticket
        return ticket
    }
    public mutating func complete(_ ticket: Ticket, peripheral id: UUID) -> Bool {
        guard !retiring, peripheral == id, pending == ticket, ticket.generation == generation else { return false }
        pending = nil
        return true
    }
    public mutating func terminate() {
        generation &+= 1; pending = nil; retiring = peripheral != nil
    }
    public mutating func disconnected(_ id: UUID) -> Bool {
        guard peripheral == id else { return false }
        generation &+= 1; pending = nil; peripheral = nil; retiring = false
        return true
    }
}
