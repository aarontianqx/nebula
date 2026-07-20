import Foundation

public enum ProxyState: String, Codable, Sendable {
    case stopped
    case starting
    case running
    case degraded
}

public enum ListenerState: String, Codable, Sendable {
    case binding
    case ready
    case failed
}

public struct ListenerStatus: Identifiable, Equatable, Sendable {
    public var id: Int { port }
    public let port: Int
    public let state: ListenerState
    public let error: String?

    public init(port: Int, state: ListenerState, error: String? = nil) {
        self.port = port
        self.state = state
        self.error = error
    }
}

public struct ProxyStatus: Equatable, Sendable {
    public let state: ProxyState
    public let listeners: [ListenerStatus]
    public let activeConnections: Int
    public let activeRequests: Int
    public let forwardedRequests: Int64
    public let parseMissing: Int64
    public let parseErrors: Int64
    public let lastError: String?

    public init(
        state: ProxyState,
        listeners: [ListenerStatus] = [],
        activeConnections: Int = 0,
        activeRequests: Int = 0,
        forwardedRequests: Int64 = 0,
        parseMissing: Int64 = 0,
        parseErrors: Int64 = 0,
        lastError: String? = nil
    ) {
        self.state = state
        self.listeners = listeners
        self.activeConnections = activeConnections
        self.activeRequests = activeRequests
        self.forwardedRequests = forwardedRequests
        self.parseMissing = parseMissing
        self.parseErrors = parseErrors
        self.lastError = lastError
    }
}

public enum ProxyServiceError: LocalizedError {
    case invalidConfiguration([ConfigurationIssue])
    case listenerBindingFailed(port: Int, underlying: Error)
    case alreadyShutdown

    public var errorDescription: String? {
        switch self {
        case .invalidConfiguration(let issues):
            issues.map(\.message).joined(separator: "\n")
        case .listenerBindingFailed(let port, let underlying):
            "Unable to bind 127.0.0.1:\(port): \(underlying.localizedDescription)"
        case .alreadyShutdown:
            "Proxy service has already shut down."
        }
    }
}
