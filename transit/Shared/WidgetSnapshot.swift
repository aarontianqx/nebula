import Foundation

public enum WidgetPeriod: String, Codable, CaseIterable, Sendable {
    case hour
    case today
    case sevenDays = "seven_days"

    public var displayName: String {
        switch self {
        case .hour: "最近一小时"
        case .today: "今日"
        case .sevenDays: "最近七天"
        }
    }
}

public enum WidgetProxyState: String, Codable, Sendable {
    case stopped
    case starting
    case running
    case degraded
}

public enum WidgetPrimaryMetric: String, Codable, CaseIterable, Identifiable, Sendable {
    case total
    case input
    case output
    case cachedInput = "cached_input"
    case reasoning

    public var id: String { rawValue }

    public var displayName: String {
        switch self {
        case .total: "总 Token"
        case .input: "输入 Token"
        case .output: "输出 Token"
        case .cachedInput: "缓存 Token"
        case .reasoning: "推理 Token"
        }
    }

    public func value(in summary: WidgetUsageSummary) -> Int64 {
        switch self {
        case .total: summary.totalTokens
        case .input: summary.inputTokens
        case .output: summary.outputTokens
        case .cachedInput: summary.cachedInputTokens
        case .reasoning: summary.reasoningTokens
        }
    }
}

public struct WidgetModelSummary: Codable, Identifiable, Equatable, Sendable {
    public let id: String
    public let model: String
    public let totalTokens: Int64

    public init(id: String, model: String, totalTokens: Int64) {
        self.id = id
        self.model = model
        self.totalTokens = totalTokens
    }
}

public struct WidgetUsageSummary: Codable, Equatable, Sendable {
    public let totalTokens: Int64
    public let inputTokens: Int64
    public let outputTokens: Int64
    public let cachedInputTokens: Int64
    public let reasoningTokens: Int64
    public let estimatedCost: String?
    public let currency: String?
    public let topModels: [WidgetModelSummary]

    public init(
        totalTokens: Int64,
        inputTokens: Int64,
        outputTokens: Int64,
        cachedInputTokens: Int64,
        reasoningTokens: Int64,
        estimatedCost: String?,
        currency: String?,
        topModels: [WidgetModelSummary]
    ) {
        self.totalTokens = totalTokens
        self.inputTokens = inputTokens
        self.outputTokens = outputTokens
        self.cachedInputTokens = cachedInputTokens
        self.reasoningTokens = reasoningTokens
        self.estimatedCost = estimatedCost
        self.currency = currency
        self.topModels = topModels
    }
}

public struct WidgetSnapshot: Codable, Equatable, Sendable {
    public let generatedAt: Date
    public let proxyState: WidgetProxyState
    public let eventsComplete: Bool
    public let periods: [WidgetPeriod: WidgetUsageSummary]

    public init(
        generatedAt: Date = Date(),
        proxyState: WidgetProxyState,
        eventsComplete: Bool,
        periods: [WidgetPeriod: WidgetUsageSummary]
    ) {
        self.generatedAt = generatedAt
        self.proxyState = proxyState
        self.eventsComplete = eventsComplete
        self.periods = periods
    }

    enum CodingKeys: String, CodingKey {
        case generatedAt
        case proxyState
        case eventsComplete
        case periods
    }

    public init(from decoder: any Decoder) throws {
        let container = try decoder.container(keyedBy: CodingKeys.self)
        generatedAt = try container.decode(Date.self, forKey: .generatedAt)
        proxyState = try container.decode(WidgetProxyState.self, forKey: .proxyState)
        eventsComplete = try container.decode(Bool.self, forKey: .eventsComplete)
        let values = try container.decode([String: WidgetUsageSummary].self, forKey: .periods)
        periods = Dictionary(uniqueKeysWithValues: values.compactMap { key, value in
            WidgetPeriod(rawValue: key).map { ($0, value) }
        })
    }

    public func encode(to encoder: any Encoder) throws {
        var container = encoder.container(keyedBy: CodingKeys.self)
        try container.encode(generatedAt, forKey: .generatedAt)
        try container.encode(proxyState, forKey: .proxyState)
        try container.encode(eventsComplete, forKey: .eventsComplete)
        try container.encode(
            Dictionary(uniqueKeysWithValues: periods.map { ($0.key.rawValue, $0.value) }),
            forKey: .periods
        )
    }

    public func summary(for period: WidgetPeriod) -> WidgetUsageSummary {
        periods[period] ?? WidgetUsageSummary(
            totalTokens: 0,
            inputTokens: 0,
            outputTokens: 0,
            cachedInputTokens: 0,
            reasoningTokens: 0,
            estimatedCost: nil,
            currency: nil,
            topModels: []
        )
    }

    public func isStale(at date: Date = Date(), maximumAge: TimeInterval = 300) -> Bool {
        date.timeIntervalSince(generatedAt) > maximumAge
    }
}

public enum WidgetSnapshotStore {
    public static let appGroupID = "DBGKX6VP2X.com.aarontianqx.transit"
    private static let selectedPeriodKey = "widget_selected_period"
    private static let primaryMetricKey = "widget_primary_metric"

    public static var snapshotURL: URL? {
        FileManager.default
            .containerURL(forSecurityApplicationGroupIdentifier: appGroupID)?
            .appendingPathComponent("Library/Application Support/Transit/widget_snapshot.json")
    }

    public static func load() -> WidgetSnapshot? {
        guard let url = snapshotURL else { return nil }
        return load(from: url)
    }

    public static func save(_ snapshot: WidgetSnapshot) throws {
        guard let url = snapshotURL else { throw WidgetSnapshotStoreError.containerUnavailable }
        try save(snapshot, to: url)
    }

    public static func load(from url: URL) -> WidgetSnapshot? {
        guard let data = try? Data(contentsOf: url) else { return nil }
        return try? decoder.decode(WidgetSnapshot.self, from: data)
    }

    public static func save(_ snapshot: WidgetSnapshot, to url: URL) throws {
        try FileManager.default.createDirectory(
            at: url.deletingLastPathComponent(),
            withIntermediateDirectories: true
        )
        try encoder.encode(snapshot).write(to: url, options: .atomic)
    }

    public static func loadSelectedPeriod() -> WidgetPeriod {
        guard let raw = UserDefaults(suiteName: appGroupID)?.string(forKey: selectedPeriodKey),
              let period = WidgetPeriod(rawValue: raw)
        else { return .today }
        return period
    }

    public static func saveSelectedPeriod(_ period: WidgetPeriod) {
        UserDefaults(suiteName: appGroupID)?.set(period.rawValue, forKey: selectedPeriodKey)
    }

    public static func loadPrimaryMetric() -> WidgetPrimaryMetric {
        guard let raw = UserDefaults(suiteName: appGroupID)?.string(forKey: primaryMetricKey),
              let metric = WidgetPrimaryMetric(rawValue: raw)
        else { return .total }
        return metric
    }

    public static func savePrimaryMetric(_ metric: WidgetPrimaryMetric) {
        UserDefaults(suiteName: appGroupID)?.set(metric.rawValue, forKey: primaryMetricKey)
    }

    private static let encoder: JSONEncoder = {
        let encoder = JSONEncoder()
        encoder.dateEncodingStrategy = .iso8601
        encoder.outputFormatting = [.prettyPrinted, .sortedKeys]
        return encoder
    }()

    private static let decoder: JSONDecoder = {
        let decoder = JSONDecoder()
        decoder.dateDecodingStrategy = .iso8601
        return decoder
    }()
}

public enum WidgetSnapshotStoreError: LocalizedError {
    case containerUnavailable

    public var errorDescription: String? {
        "Transit App Group container is unavailable."
    }
}
