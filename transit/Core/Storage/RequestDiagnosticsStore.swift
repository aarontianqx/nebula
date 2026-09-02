import Foundation
import GRDB

public final class RequestDiagnosticsStore: @unchecked Sendable {
    public let databaseURL: URL
    private let database: DatabasePool

    public init(databaseURL: URL = RequestDiagnosticsStore.defaultDatabaseURL) throws {
        self.databaseURL = databaseURL
        try FileManager.default.createDirectory(
            at: databaseURL.deletingLastPathComponent(),
            withIntermediateDirectories: true
        )
        var configuration = Configuration()
        configuration.busyMode = .timeout(5)
        configuration.prepareDatabase { db in
            try db.execute(sql: "PRAGMA journal_mode = WAL")
            try db.execute(sql: "PRAGMA wal_autocheckpoint = 128")
            try db.execute(sql: "PRAGMA journal_size_limit = 524288")
        }
        database = try DatabasePool(path: databaseURL.path, configuration: configuration)
        try Self.migrator.migrate(database)
        try database.write { db in
            try db.execute(
                sql: """
                UPDATE request_diagnostics
                SET state = 'stalled', error_code = COALESCE(error_code, 'app_restarted')
                WHERE state = 'active'
                """
            )
        }
    }

    public static var defaultDatabaseURL: URL {
        let root = FileManager.default.urls(for: .applicationSupportDirectory, in: .userDomainMask)[0]
        return root.appendingPathComponent("Transit/diagnostics.sqlite")
    }

    public func upsert(_ snapshot: RequestDiagnosticSnapshot) throws {
        try database.write { db in
            try DatabaseRequestDiagnostic(snapshot).save(db)
        }
    }

    public func delete(id: String) throws {
        try database.write { db in
            _ = try DatabaseRequestDiagnostic.deleteOne(db, key: id)
        }
    }

    public func recent(limit: Int = 200) throws -> [RequestDiagnosticSnapshot] {
        try database.read { db in
            try DatabaseRequestDiagnostic
                .order(Column("updated_at").desc)
                .limit(max(1, min(limit, 200)))
                .fetchAll(db)
                .map(\.snapshot)
        }
    }

    @discardableResult
    public func prune(olderThan cutoff: Date, retaining limit: Int = 200) throws -> Int {
        try database.write { db in
            try db.execute(sql: "DELETE FROM request_diagnostics WHERE updated_at < ?", arguments: [cutoff])
            let expired = db.changesCount
            try db.execute(
                sql: """
                DELETE FROM request_diagnostics
                WHERE id IN (
                    SELECT id FROM request_diagnostics
                    ORDER BY updated_at DESC
                    LIMIT -1 OFFSET ?
                )
                """,
                arguments: [max(1, min(limit, 200))]
            )
            return expired + db.changesCount
        }
    }

    public func checkpoint() throws {
        _ = try database.writeWithoutTransaction { db in
            try db.checkpoint(.truncate)
        }
    }

    public func close() throws {
        try database.close()
    }

    private static var migrator: DatabaseMigrator {
        var migrator = DatabaseMigrator()
        migrator.registerMigration("v1_request_diagnostics") { db in
            try db.create(table: "request_diagnostics") { table in
                table.column("id", .text).primaryKey()
                table.column("route_id", .text).notNull()
                table.column("protocol", .text).notNull()
                table.column("method", .text).notNull()
                table.column("phase", .text).notNull()
                table.column("state", .text).notNull()
                table.column("started_at", .datetime).notNull()
                table.column("updated_at", .datetime).notNull()
                table.column("last_upstream_activity_at", .datetime)
                table.column("last_meaningful_activity_at", .datetime).notNull()
                table.column("last_downstream_write_at", .datetime)
                table.column("last_response_activity", .text)
                table.column("request_bytes", .integer).notNull()
                table.column("response_bytes", .integer).notNull()
                table.column("status_code", .integer)
                table.column("error_code", .text)
            }
            try db.create(
                index: "idx_request_diagnostics_updated_at",
                on: "request_diagnostics",
                columns: ["updated_at"]
            )
        }
        return migrator
    }
}

private struct DatabaseRequestDiagnostic: Codable, FetchableRecord, PersistableRecord {
    static let databaseTableName = "request_diagnostics"

    let id: String
    let routeID: String
    let protocolType: String
    let method: String
    let phase: String
    let state: String
    let startedAt: Date
    let updatedAt: Date
    let lastUpstreamActivityAt: Date?
    let lastMeaningfulActivityAt: Date
    let lastDownstreamWriteAt: Date?
    let lastResponseActivity: String?
    let requestBytes: Int64
    let responseBytes: Int64
    let statusCode: Int?
    let errorCode: String?

    enum CodingKeys: String, CodingKey {
        case id
        case routeID = "route_id"
        case protocolType = "protocol"
        case method
        case phase
        case state
        case startedAt = "started_at"
        case updatedAt = "updated_at"
        case lastUpstreamActivityAt = "last_upstream_activity_at"
        case lastMeaningfulActivityAt = "last_meaningful_activity_at"
        case lastDownstreamWriteAt = "last_downstream_write_at"
        case lastResponseActivity = "last_response_activity"
        case requestBytes = "request_bytes"
        case responseBytes = "response_bytes"
        case statusCode = "status_code"
        case errorCode = "error_code"
    }

    init(_ snapshot: RequestDiagnosticSnapshot) {
        id = snapshot.id
        routeID = snapshot.routeID
        protocolType = snapshot.protocolType.rawValue
        method = snapshot.method
        phase = snapshot.phase.rawValue
        state = snapshot.state.rawValue
        startedAt = snapshot.startedAt
        updatedAt = snapshot.updatedAt
        lastUpstreamActivityAt = snapshot.lastUpstreamActivityAt
        lastMeaningfulActivityAt = snapshot.lastMeaningfulActivityAt
        lastDownstreamWriteAt = snapshot.lastDownstreamWriteAt
        lastResponseActivity = snapshot.lastResponseActivity?.rawValue
        requestBytes = snapshot.requestBytes
        responseBytes = snapshot.responseBytes
        statusCode = snapshot.statusCode
        errorCode = snapshot.errorCode
    }

    var snapshot: RequestDiagnosticSnapshot {
        RequestDiagnosticSnapshot(
            id: id,
            routeID: routeID,
            protocolType: UsageProtocol(rawValue: protocolType) ?? .openAIChat,
            method: method,
            phase: RequestDiagnosticPhase(rawValue: phase) ?? .clientRequestStarted,
            state: RequestDiagnosticState(rawValue: state) ?? .failed,
            startedAt: startedAt,
            updatedAt: updatedAt,
            lastUpstreamActivityAt: lastUpstreamActivityAt,
            lastMeaningfulActivityAt: lastMeaningfulActivityAt,
            lastDownstreamWriteAt: lastDownstreamWriteAt,
            lastResponseActivity: lastResponseActivity.flatMap(ResponseDiagnosticActivity.init(rawValue:)),
            requestBytes: requestBytes,
            responseBytes: responseBytes,
            statusCode: statusCode,
            errorCode: errorCode
        )
    }
}
