import Foundation

public enum UsageProtocol: String, Codable, CaseIterable, Identifiable, Sendable {
    case openAIChat = "openai_chat"
    case openAIResponses = "openai_responses"
    case anthropicMessages = "anthropic_messages"

    public var id: String { rawValue }

    public var displayName: String {
        switch self {
        case .openAIChat: "OpenAI Chat"
        case .openAIResponses: "OpenAI Responses"
        case .anthropicMessages: "Anthropic Messages"
        }
    }
}
public enum RequestOutcome: String, Codable, CaseIterable, Sendable {
    case completed
    case failed
    case cancelled
}

public enum UsageQuality: String, Codable, CaseIterable, Sendable {
    case reported
    case missing
}

public enum EndpointKind: String, Codable, Sendable {
    case chat
    case responses
    case messages
    case unknown
}

public struct NormalizedUsage: Codable, Equatable, Sendable {
    public var inputTokens: Int64?
    public var outputTokens: Int64?
    public var cachedInputTokens: Int64?
    public var reasoningTokens: Int64?
    public var totalTokens: Int64?
    public var totalTokensDerived: Bool

    public init(
        inputTokens: Int64? = nil,
        outputTokens: Int64? = nil,
        cachedInputTokens: Int64? = nil,
        reasoningTokens: Int64? = nil,
        totalTokens: Int64? = nil,
        totalTokensDerived: Bool = false
    ) {
        self.inputTokens = inputTokens
        self.outputTokens = outputTokens
        self.cachedInputTokens = cachedInputTokens
        self.reasoningTokens = reasoningTokens
        self.totalTokens = totalTokens
        self.totalTokensDerived = totalTokensDerived
    }

    public mutating func deriveTotalIfNeeded() {
        guard totalTokens == nil, inputTokens != nil || outputTokens != nil else { return }
        totalTokens = (inputTokens ?? 0) + (outputTokens ?? 0)
        totalTokensDerived = true
    }

    public var hasAnyValue: Bool {
        inputTokens != nil || outputTokens != nil || cachedInputTokens != nil
            || reasoningTokens != nil || totalTokens != nil
    }
}

public struct UsageObservation: Equatable, Sendable {
    public var usage: NormalizedUsage
    public var quality: UsageQuality
    public var model: String?
    public var endpointKind: EndpointKind
    public var rawUsageJSON: String?
    public var parserErrorCode: String?

    public init(
        usage: NormalizedUsage = NormalizedUsage(),
        quality: UsageQuality = .missing,
        model: String? = nil,
        endpointKind: EndpointKind = .unknown,
        rawUsageJSON: String? = nil,
        parserErrorCode: String? = nil
    ) {
        self.usage = usage
        self.quality = quality
        self.model = model
        self.endpointKind = endpointKind
        self.rawUsageJSON = rawUsageJSON
        self.parserErrorCode = parserErrorCode
    }
}
