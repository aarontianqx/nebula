import Foundation

public struct UsageEvent: Codable, Equatable, Identifiable, Sendable {
    public var id: String
    public var startedAt: Date
    public var completedAt: Date
    public var routeID: String
    public var agentID: String?
    public var protocolType: UsageProtocol
    public var method: String
    public var endpointKind: EndpointKind
    public var statusCode: Int?
    public var outcome: RequestOutcome
    public var latencyMilliseconds: Int64
    public var model: String?
    public var keyFingerprint: String?
    public var usage: NormalizedUsage
    public var usageQuality: UsageQuality
    public var usageRaw: String?
    public var requestBytes: Int64
    public var responseBytes: Int64
    public var estimatedCost: String?
    public var currency: String?
    public var pricingPolicyID: String?
    public var pricingPolicyVersion: String?
    public var errorCode: String?

    public init(
        id: String = UUID().uuidString.lowercased(),
        startedAt: Date,
        completedAt: Date,
        routeID: String,
        agentID: String?,
        protocolType: UsageProtocol,
        method: String,
        endpointKind: EndpointKind,
        statusCode: Int?,
        outcome: RequestOutcome,
        latencyMilliseconds: Int64,
        model: String?,
        keyFingerprint: String?,
        usage: NormalizedUsage,
        usageQuality: UsageQuality,
        usageRaw: String?,
        requestBytes: Int64,
        responseBytes: Int64,
        estimatedCost: String?,
        currency: String?,
        pricingPolicyID: String?,
        pricingPolicyVersion: String?,
        errorCode: String?
    ) {
        self.id = id
        self.startedAt = startedAt
        self.completedAt = completedAt
        self.routeID = routeID
        self.agentID = agentID
        self.protocolType = protocolType
        self.method = method
        self.endpointKind = endpointKind
        self.statusCode = statusCode
        self.outcome = outcome
        self.latencyMilliseconds = latencyMilliseconds
        self.model = model
        self.keyFingerprint = keyFingerprint
        self.usage = usage
        self.usageQuality = usageQuality
        self.usageRaw = usageRaw
        self.requestBytes = requestBytes
        self.responseBytes = responseBytes
        self.estimatedCost = estimatedCost
        self.currency = currency
        self.pricingPolicyID = pricingPolicyID
        self.pricingPolicyVersion = pricingPolicyVersion
        self.errorCode = errorCode
    }
}

public struct UsageSummary: Equatable, Sendable {
    public var requestCount: Int64
    public var failedRequestCount: Int64
    public var inputTokens: Int64
    public var outputTokens: Int64
    public var cachedInputTokens: Int64
    public var reasoningTokens: Int64
    public var totalTokens: Int64
    public var estimatedCost: Decimal?
    public var currency: String?
    public var averageLatencyMilliseconds: Double
    public var dataComplete: Bool

    public init(
        requestCount: Int64 = 0,
        failedRequestCount: Int64 = 0,
        inputTokens: Int64 = 0,
        outputTokens: Int64 = 0,
        cachedInputTokens: Int64 = 0,
        reasoningTokens: Int64 = 0,
        totalTokens: Int64 = 0,
        estimatedCost: Decimal? = nil,
        currency: String? = nil,
        averageLatencyMilliseconds: Double = 0,
        dataComplete: Bool = true
    ) {
        self.requestCount = requestCount
        self.failedRequestCount = failedRequestCount
        self.inputTokens = inputTokens
        self.outputTokens = outputTokens
        self.cachedInputTokens = cachedInputTokens
        self.reasoningTokens = reasoningTokens
        self.totalTokens = totalTokens
        self.estimatedCost = estimatedCost
        self.currency = currency
        self.averageLatencyMilliseconds = averageLatencyMilliseconds
        self.dataComplete = dataComplete
    }
}

public struct ModelUsageSummary: Equatable, Identifiable, Sendable {
    public var id: String { model }
    public let model: String
    public let requestCount: Int64
    public let totalTokens: Int64

    public init(model: String, requestCount: Int64, totalTokens: Int64) {
        self.model = model
        self.requestCount = requestCount
        self.totalTokens = totalTokens
    }
}

public struct HTTPStatusSummary: Equatable, Identifiable, Sendable {
    public var id: String { statusCode.map(String.init) ?? "no_response" }
    public let statusCode: Int?
    public let requestCount: Int64

    public init(statusCode: Int?, requestCount: Int64) {
        self.statusCode = statusCode
        self.requestCount = requestCount
    }

    public var displayName: String {
        statusCode.map { "HTTP \($0)" } ?? "无上游响应"
    }
}

public enum UsageBreakdownDimension: String, CaseIterable, Identifiable, Sendable {
    case agent
    case route
    case protocolType = "protocol"
    case model

    public var id: String { rawValue }

    public var displayName: String {
        switch self {
        case .agent: "Agent"
        case .route: "Route"
        case .protocolType: "协议"
        case .model: "模型"
        }
    }
}

public struct UsageBreakdownItem: Equatable, Identifiable, Sendable {
    public var id: String { label }
    public let label: String
    public let requestCount: Int64
    public let failedRequestCount: Int64
    public let totalTokens: Int64
    public let averageLatencyMilliseconds: Double

    public init(
        label: String,
        requestCount: Int64,
        failedRequestCount: Int64,
        totalTokens: Int64,
        averageLatencyMilliseconds: Double
    ) {
        self.label = label
        self.requestCount = requestCount
        self.failedRequestCount = failedRequestCount
        self.totalTokens = totalTokens
        self.averageLatencyMilliseconds = averageLatencyMilliseconds
    }
}
