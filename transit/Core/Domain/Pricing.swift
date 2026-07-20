import Foundation

public struct PricingRule: Codable, Equatable, Sendable {
    public var modelPattern: String
    public var inputPerMillion: Decimal
    public var cachedInputPerMillion: Decimal
    public var outputPerMillion: Decimal

    public init(
        modelPattern: String,
        inputPerMillion: Decimal,
        cachedInputPerMillion: Decimal = 0,
        outputPerMillion: Decimal
    ) {
        self.modelPattern = modelPattern
        self.inputPerMillion = inputPerMillion
        self.cachedInputPerMillion = cachedInputPerMillion
        self.outputPerMillion = outputPerMillion
    }

    enum CodingKeys: String, CodingKey {
        case modelPattern = "model_pattern"
        case inputPerMillion = "input_per_million"
        case cachedInputPerMillion = "cached_input_per_million"
        case outputPerMillion = "output_per_million"
    }
}
public struct PricingPolicy: Codable, Equatable, Identifiable, Sendable {
    public var id: String
    public var version: String
    public var currency: String
    public var rules: [PricingRule]

    public init(id: String, version: String, currency: String, rules: [PricingRule]) {
        self.id = id
        self.version = version
        self.currency = currency
        self.rules = rules
    }
}

public struct EstimatedCost: Equatable, Sendable {
    public let amount: Decimal
    public let currency: String
    public let policyID: String
    public let policyVersion: String

    public init(amount: Decimal, currency: String, policyID: String, policyVersion: String) {
        self.amount = amount
        self.currency = currency
        self.policyID = policyID
        self.policyVersion = policyVersion
    }
}

public enum PricingEngine {
    public static func estimate(
        usage: NormalizedUsage,
        model: String?,
        policy: PricingPolicy?
    ) -> EstimatedCost? {
        guard let model, let policy,
              let rule = policy.rules.first(where: { matches(model, pattern: $0.modelPattern) })
        else { return nil }

        let input = max(0, usage.inputTokens ?? 0)
        let cached = min(input, max(0, usage.cachedInputTokens ?? 0))
        let uncached = input - cached
        let output = max(0, usage.outputTokens ?? 0)
        let million = Decimal(1_000_000)
        let amount = Decimal(uncached) / million * rule.inputPerMillion
            + Decimal(cached) / million * rule.cachedInputPerMillion
            + Decimal(output) / million * rule.outputPerMillion
        return EstimatedCost(
            amount: amount,
            currency: policy.currency,
            policyID: policy.id,
            policyVersion: policy.version
        )
    }

    static func matches(_ value: String, pattern: String) -> Bool {
        let escaped = NSRegularExpression.escapedPattern(for: pattern)
            .replacingOccurrences(of: "\\*", with: ".*")
            .replacingOccurrences(of: "\\?", with: ".")
        guard let expression = try? NSRegularExpression(
            pattern: "^\(escaped)$",
            options: [.caseInsensitive]
        ) else { return false }
        let range = NSRange(value.startIndex..<value.endIndex, in: value)
        return expression.firstMatch(in: value, range: range) != nil
    }
}
