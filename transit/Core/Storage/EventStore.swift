import Foundation
import GRDB

public protocol UsageEventWriting: Sendable {
    func save(_ event: UsageEvent) throws
}

public struct UsageQuery: Sendable {
    public var since: Date
    public var until: Date?
    public var routeID: String?
    public var agentID: String?
    public var protocolType: UsageProtocol?
    public var model: String?
    public var outcome: RequestOutcome?
    public var usageQuality: UsageQuality?

    public init(
        since: Date,
        until: Date? = nil,
        routeID: String? = nil,
        agentID: String? = nil,
        protocolType: UsageProtocol? = nil,
        model: String? = nil,
        outcome: RequestOutcome? = nil,
        usageQuality: UsageQuality? = nil
    ) {
        self.since = since
        self.until = until
        self.routeID = routeID
        self.agentID = agentID
        self.protocolType = protocolType
        self.model = model
        self.outcome = outcome
        self.usageQuality = usageQuality
    }
}

public final class EventStore: UsageEventWriting, @unchecked Sendable {
    public let databaseURL: URL
    private let database: DatabasePool

    public init(databaseURL: URL = EventStore.defaultDatabaseURL) throws {
        self.databaseURL = databaseURL
        try FileManager.default.createDirectory(
            at: databaseURL.deletingLastPathComponent(),
            withIntermediateDirectories: true
        )
        var configuration = Configuration()
        configuration.busyMode = .timeout(5)
        configuration.prepareDatabase { db in
            try db.execute(sql: "PRAGMA foreign_keys = ON")
            try db.execute(sql: "PRAGMA journal_mode = WAL")
        }
        database = try DatabasePool(path: databaseURL.path, configuration: configuration)
        try Self.migrator.migrate(database)
    }

    public static var defaultDatabaseURL: URL {
        let root = FileManager.default.urls(for: .applicationSupportDirectory, in: .userDomainMask)[0]
        return root.appendingPathComponent("Transit/usage.sqlite")
    }

    public func save(_ event: UsageEvent) throws {
        try database.write { db in
            try DatabaseUsageEvent(event).insert(db)
        }
    }

    public func recentEvents(limit: Int = 100) throws -> [UsageEvent] {
        try database.read { db in
            try DatabaseUsageEvent
                .order(Column("started_at").desc)
                .limit(max(1, min(limit, 1_000)))
                .fetchAll(db)
                .map(\.usageEvent)
        }
    }

    public func summary(_ query: UsageQuery) throws -> UsageSummary {
        let filter = SQLFilter(query)
        return try database.read { db in
            let row = try Row.fetchOne(
                db,
                sql: """
                SELECT
                  COUNT(*) AS request_count,
                  COALESCE(SUM(CASE WHEN outcome = 'failed' THEN 1 ELSE 0 END), 0) AS failed_count,
                  COALESCE(SUM(input_tokens), 0) AS input_tokens,
                  COALESCE(SUM(output_tokens), 0) AS output_tokens,
                  COALESCE(SUM(cached_input_tokens), 0) AS cached_input_tokens,
                  COALESCE(SUM(reasoning_tokens), 0) AS reasoning_tokens,
                  COALESCE(SUM(total_tokens), 0) AS total_tokens,
                  COALESCE(AVG(latency_ms), 0) AS average_latency,
                  COALESCE(SUM(CASE WHEN usage_quality = 'missing' THEN 1 ELSE 0 END), 0) AS missing_count
                FROM usage_events
                WHERE \(filter.clause)
                """,
                arguments: filter.arguments
            )!

            let costRows = try Row.fetchAll(
                db,
                sql: """
                SELECT estimated_cost, currency
                FROM usage_events
                WHERE \(filter.clause) AND estimated_cost IS NOT NULL
                """,
                arguments: filter.arguments
            )
            let currencies = Set(costRows.compactMap { $0["currency"] as String? })
            let cost: Decimal? = !costRows.isEmpty && currencies.count == 1
                ? costRows.compactMap {
                    Decimal(
                        string: $0["estimated_cost"] as String? ?? "",
                        locale: Locale(identifier: "en_US_POSIX")
                    )
                }.reduce(0, +)
                : nil

            return UsageSummary(
                requestCount: row["request_count"],
                failedRequestCount: row["failed_count"],
                inputTokens: row["input_tokens"],
                outputTokens: row["output_tokens"],
                cachedInputTokens: row["cached_input_tokens"],
                reasoningTokens: row["reasoning_tokens"],
                totalTokens: row["total_tokens"],
                estimatedCost: cost,
                currency: currencies.count == 1 ? currencies.first : nil,
                averageLatencyMilliseconds: row["average_latency"],
                dataComplete: (row["missing_count"] as Int64) == 0
            )
        }
    }

    public func topModels(_ query: UsageQuery, limit: Int = 5) throws -> [ModelUsageSummary] {
        let filter = SQLFilter(query)
        return try database.read { db in
            try Row.fetchAll(
                db,
                sql: """
                SELECT model, COUNT(*) AS request_count, COALESCE(SUM(total_tokens), 0) AS total_tokens
                FROM usage_events
                WHERE \(filter.clause) AND model IS NOT NULL AND model != ''
                GROUP BY model
                ORDER BY total_tokens DESC
                LIMIT ?
                """,
                arguments: filter.arguments + [max(1, min(limit, 100))]
            ).map { row in
                ModelUsageSummary(
                    model: row["model"],
                    requestCount: row["request_count"],
                    totalTokens: row["total_tokens"]
                )
            }
        }
    }

    public func statusDistribution(_ query: UsageQuery) throws -> [HTTPStatusSummary] {
        let filter = SQLFilter(query)
        return try database.read { db in
            try Row.fetchAll(
                db,
                sql: """
                SELECT status_code, COUNT(*) AS request_count
                FROM usage_events
                WHERE \(filter.clause)
                GROUP BY status_code
                ORDER BY status_code IS NULL, status_code
                """,
                arguments: filter.arguments
            ).map { row in
                HTTPStatusSummary(
                    statusCode: row["status_code"],
                    requestCount: row["request_count"]
                )
            }
        }
    }

    public func breakdown(
        _ query: UsageQuery,
        dimension: UsageBreakdownDimension,
        limit: Int = 100
    ) throws -> [UsageBreakdownItem] {
        let filter = SQLFilter(query)
        let column: String
        switch dimension {
        case .agent: column = "agent_id"
        case .route: column = "route_id"
        case .protocolType: column = "protocol"
        case .model: column = "model"
        }
        return try database.read { db in
            try Row.fetchAll(
                db,
                sql: """
                SELECT
                  \(column) AS label,
                  COUNT(*) AS request_count,
                  SUM(CASE WHEN outcome = 'failed' THEN 1 ELSE 0 END) AS failed_count,
                  COALESCE(SUM(total_tokens), 0) AS total_tokens,
                  COALESCE(AVG(latency_ms), 0) AS average_latency
                FROM usage_events
                WHERE \(filter.clause) AND \(column) IS NOT NULL AND \(column) != ''
                GROUP BY \(column)
                ORDER BY total_tokens DESC
                LIMIT ?
                """,
                arguments: filter.arguments + [max(1, min(limit, 500))]
            ).map { row in
                UsageBreakdownItem(
                    label: row["label"],
                    requestCount: row["request_count"],
                    failedRequestCount: row["failed_count"],
                    totalTokens: row["total_tokens"],
                    averageLatencyMilliseconds: row["average_latency"]
                )
            }
        }
    }

    @discardableResult
    public func prune(olderThan date: Date) throws -> Int {
        try database.write { db in
            try db.execute(sql: "DELETE FROM usage_events WHERE started_at < ?", arguments: [date])
            return db.changesCount
        }
    }

    public func checkpoint() throws {
        _ = try database.writeWithoutTransaction { db in
            try db.checkpoint(.passive)
        }
    }

    public func close() throws {
        try database.close()
    }

    private static var migrator: DatabaseMigrator {
        var migrator = DatabaseMigrator()
        migrator.registerMigration("v1_usage_events") { db in
            try db.create(table: "usage_events") { table in
                table.column("id", .text).primaryKey()
                table.column("started_at", .datetime).notNull()
                table.column("completed_at", .datetime).notNull()
                table.column("route_id", .text).notNull()
                table.column("agent_id", .text)
                table.column("protocol", .text).notNull()
                table.column("method", .text).notNull()
                table.column("endpoint_kind", .text).notNull()
                table.column("status_code", .integer)
                table.column("outcome", .text).notNull()
                table.column("latency_ms", .integer).notNull()
                table.column("model", .text)
                table.column("key_fingerprint", .text)
                table.column("input_tokens", .integer)
                table.column("output_tokens", .integer)
                table.column("cached_input_tokens", .integer)
                table.column("reasoning_tokens", .integer)
                table.column("total_tokens", .integer)
                table.column("total_tokens_derived", .boolean).notNull()
                table.column("usage_quality", .text).notNull()
                table.column("usage_raw", .text)
                table.column("request_bytes", .integer).notNull()
                table.column("response_bytes", .integer).notNull()
                table.column("estimated_cost", .text)
                table.column("currency", .text)
                table.column("pricing_policy_id", .text)
                table.column("pricing_policy_version", .text)
                table.column("error_code", .text)
            }
            try db.create(index: "idx_usage_events_time", on: "usage_events", columns: ["started_at"])
            try db.create(index: "idx_usage_events_route_time", on: "usage_events", columns: ["route_id", "started_at"])
            try db.create(
                index: "idx_usage_events_agent_model_time",
                on: "usage_events",
                columns: ["agent_id", "model", "started_at"]
            )
        }
        return migrator
    }
}

private struct SQLFilter {
    let clause: String
    let arguments: StatementArguments

    init(_ query: UsageQuery) {
        var fragments = ["started_at >= ?"]
        var values: [DatabaseValueConvertible?] = [query.since]
        if let until = query.until { fragments.append("started_at < ?"); values.append(until) }
        if let routeID = query.routeID { fragments.append("route_id = ?"); values.append(routeID) }
        if let agentID = query.agentID { fragments.append("agent_id = ?"); values.append(agentID) }
        if let protocolType = query.protocolType { fragments.append("protocol = ?"); values.append(protocolType.rawValue) }
        if let model = query.model { fragments.append("model = ?"); values.append(model) }
        if let outcome = query.outcome { fragments.append("outcome = ?"); values.append(outcome.rawValue) }
        if let quality = query.usageQuality { fragments.append("usage_quality = ?"); values.append(quality.rawValue) }
        clause = fragments.joined(separator: " AND ")
        arguments = StatementArguments(values)
    }
}

private struct DatabaseUsageEvent: Codable, FetchableRecord, PersistableRecord {
    static let databaseTableName = "usage_events"

    let id: String
    let startedAt: Date
    let completedAt: Date
    let routeID: String
    let agentID: String?
    let protocolType: String
    let method: String
    let endpointKind: String
    let statusCode: Int?
    let outcome: String
    let latencyMilliseconds: Int64
    let model: String?
    let keyFingerprint: String?
    let inputTokens: Int64?
    let outputTokens: Int64?
    let cachedInputTokens: Int64?
    let reasoningTokens: Int64?
    let totalTokens: Int64?
    let totalTokensDerived: Bool
    let usageQuality: String
    let usageRaw: String?
    let requestBytes: Int64
    let responseBytes: Int64
    let estimatedCost: String?
    let currency: String?
    let pricingPolicyID: String?
    let pricingPolicyVersion: String?
    let errorCode: String?

    enum CodingKeys: String, CodingKey {
        case id
        case startedAt = "started_at"
        case completedAt = "completed_at"
        case routeID = "route_id"
        case agentID = "agent_id"
        case protocolType = "protocol"
        case method
        case endpointKind = "endpoint_kind"
        case statusCode = "status_code"
        case outcome
        case latencyMilliseconds = "latency_ms"
        case model
        case keyFingerprint = "key_fingerprint"
        case inputTokens = "input_tokens"
        case outputTokens = "output_tokens"
        case cachedInputTokens = "cached_input_tokens"
        case reasoningTokens = "reasoning_tokens"
        case totalTokens = "total_tokens"
        case totalTokensDerived = "total_tokens_derived"
        case usageQuality = "usage_quality"
        case usageRaw = "usage_raw"
        case requestBytes = "request_bytes"
        case responseBytes = "response_bytes"
        case estimatedCost = "estimated_cost"
        case currency
        case pricingPolicyID = "pricing_policy_id"
        case pricingPolicyVersion = "pricing_policy_version"
        case errorCode = "error_code"
    }

    init(_ event: UsageEvent) {
        id = event.id
        startedAt = event.startedAt
        completedAt = event.completedAt
        routeID = event.routeID
        agentID = event.agentID
        protocolType = event.protocolType.rawValue
        method = event.method
        endpointKind = event.endpointKind.rawValue
        statusCode = event.statusCode
        outcome = event.outcome.rawValue
        latencyMilliseconds = event.latencyMilliseconds
        model = event.model
        keyFingerprint = event.keyFingerprint
        inputTokens = event.usage.inputTokens
        outputTokens = event.usage.outputTokens
        cachedInputTokens = event.usage.cachedInputTokens
        reasoningTokens = event.usage.reasoningTokens
        totalTokens = event.usage.totalTokens
        totalTokensDerived = event.usage.totalTokensDerived
        usageQuality = event.usageQuality.rawValue
        usageRaw = event.usageRaw
        requestBytes = event.requestBytes
        responseBytes = event.responseBytes
        estimatedCost = event.estimatedCost
        currency = event.currency
        pricingPolicyID = event.pricingPolicyID
        pricingPolicyVersion = event.pricingPolicyVersion
        errorCode = event.errorCode
    }

    var usageEvent: UsageEvent {
        UsageEvent(
            id: id,
            startedAt: startedAt,
            completedAt: completedAt,
            routeID: routeID,
            agentID: agentID,
            protocolType: UsageProtocol(rawValue: protocolType) ?? .openAIResponses,
            method: method,
            endpointKind: EndpointKind(rawValue: endpointKind) ?? .unknown,
            statusCode: statusCode,
            outcome: RequestOutcome(rawValue: outcome) ?? .failed,
            latencyMilliseconds: latencyMilliseconds,
            model: model,
            keyFingerprint: keyFingerprint,
            usage: NormalizedUsage(
                inputTokens: inputTokens,
                outputTokens: outputTokens,
                cachedInputTokens: cachedInputTokens,
                reasoningTokens: reasoningTokens,
                totalTokens: totalTokens,
                totalTokensDerived: totalTokensDerived
            ),
            usageQuality: UsageQuality(rawValue: usageQuality) ?? .missing,
            usageRaw: usageRaw,
            requestBytes: requestBytes,
            responseBytes: responseBytes,
            estimatedCost: estimatedCost,
            currency: currency,
            pricingPolicyID: pricingPolicyID,
            pricingPolicyVersion: pricingPolicyVersion,
            errorCode: errorCode
        )
    }
}
