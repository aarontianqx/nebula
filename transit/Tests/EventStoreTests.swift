import Foundation
import XCTest
@testable import TransitCore

final class EventStoreTests: XCTestCase {
    func testEmptySummaryReturnsZeros() throws {
        let directory = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString)
        try FileManager.default.createDirectory(at: directory, withIntermediateDirectories: true)
        let store = try EventStore(databaseURL: directory.appendingPathComponent("usage.sqlite"))
        defer {
            try? store.close()
            try? FileManager.default.removeItem(at: directory)
        }
        let summary = try store.summary(UsageQuery(since: Date().addingTimeInterval(-3_600)))

        XCTAssertEqual(summary.requestCount, 0)
        XCTAssertEqual(summary.failedRequestCount, 0)
        XCTAssertEqual(summary.totalTokens, 0)
        XCTAssertEqual(summary.averageLatencyMilliseconds, 0)
        XCTAssertTrue(summary.dataComplete)
        XCTAssertNil(summary.estimatedCost)
        XCTAssertNil(summary.currency)
    }

    func testSaveSummaryTopModelsAndPrune() throws {
        let directory = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString)
        try FileManager.default.createDirectory(at: directory, withIntermediateDirectories: true)
        let store = try EventStore(databaseURL: directory.appendingPathComponent("usage.sqlite"))
        defer {
            try? store.close()
            try? FileManager.default.removeItem(at: directory)
        }
        let now = Date()
        try store.save(event(at: now.addingTimeInterval(-60), model: "model-a", total: 100, outcome: .completed))
        try store.save(event(at: now, model: "model-a", total: 50, outcome: .failed))
        try store.save(event(at: now, model: "model-b", total: 30, outcome: .completed))

        let query = UsageQuery(since: now.addingTimeInterval(-3_600))
        let summary = try store.summary(query)
        XCTAssertEqual(summary.requestCount, 3)
        XCTAssertEqual(summary.failedRequestCount, 1)
        XCTAssertEqual(summary.totalTokens, 180)
        XCTAssertEqual(summary.inputTokens, 134)
        XCTAssertEqual(summary.outputTokens, 44)
        XCTAssertEqual(try store.topModels(query).first?.model, "model-a")
        XCTAssertEqual(
            try store.statusDistribution(query),
            [
                HTTPStatusSummary(statusCode: 200, requestCount: 2),
                HTTPStatusSummary(statusCode: 500, requestCount: 1),
            ]
        )
        XCTAssertEqual(try store.prune(olderThan: now.addingTimeInterval(-30)), 1)
        XCTAssertEqual(try store.summary(query).requestCount, 2)
    }

    func testHistorySurvivesStoreRestart() throws {
        let directory = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString)
        try FileManager.default.createDirectory(at: directory, withIntermediateDirectories: true)
        defer { try? FileManager.default.removeItem(at: directory) }
        let databaseURL = directory.appendingPathComponent("usage.sqlite")
        let expected = event(at: Date(), model: "restart-model", total: 42, outcome: .completed)

        do {
            let store = try EventStore(databaseURL: databaseURL)
            try store.save(expected)
            try store.checkpoint()
            try store.close()
        }

        let reopened = try EventStore(databaseURL: databaseURL)
        defer { try? reopened.close() }
        let restored = try XCTUnwrap(reopened.recentEvents().first)
        XCTAssertEqual(restored.id, expected.id)
        XCTAssertEqual(restored.startedAt.timeIntervalSince1970, expected.startedAt.timeIntervalSince1970, accuracy: 0.001)
        XCTAssertEqual(restored.completedAt.timeIntervalSince1970, expected.completedAt.timeIntervalSince1970, accuracy: 0.001)
        XCTAssertEqual(restored.routeID, expected.routeID)
        XCTAssertEqual(restored.model, expected.model)
        XCTAssertEqual(restored.usage, expected.usage)
        XCTAssertEqual(restored.outcome, expected.outcome)
    }

    func testSummaryFiltersQualityOutcomeAndAvoidsMixedCurrencyTotals() throws {
        let directory = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString)
        try FileManager.default.createDirectory(at: directory, withIntermediateDirectories: true)
        let store = try EventStore(databaseURL: directory.appendingPathComponent("usage.sqlite"))
        defer {
            try? store.close()
            try? FileManager.default.removeItem(at: directory)
        }
        let now = Date()
        try store.save(event(
            at: now,
            model: "model-a",
            total: 10,
            outcome: .completed,
            quality: .reported,
            estimatedCost: "1.25",
            currency: "USD"
        ))
        try store.save(event(
            at: now,
            model: "model-b",
            total: 0,
            outcome: .failed,
            quality: .missing,
            estimatedCost: "2.00",
            currency: "EUR"
        ))

        let since = now.addingTimeInterval(-60)
        let missing = try store.summary(UsageQuery(since: since, usageQuality: .missing))
        XCTAssertEqual(missing.requestCount, 1)
        XCTAssertEqual(missing.failedRequestCount, 1)
        XCTAssertFalse(missing.dataComplete)

        let completed = try store.summary(UsageQuery(since: since, outcome: .completed))
        XCTAssertEqual(completed.requestCount, 1)
        XCTAssertEqual(completed.estimatedCost, Decimal(string: "1.25"))
        XCTAssertEqual(completed.currency, "USD")

        let combined = try store.summary(UsageQuery(since: since))
        XCTAssertNil(combined.estimatedCost)
        XCTAssertNil(combined.currency)
    }

    func testEventPipelinePersistsAndReportsDroppedEvents() throws {
        let directory = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString)
        try FileManager.default.createDirectory(at: directory, withIntermediateDirectories: true)
        let store = try EventStore(databaseURL: directory.appendingPathComponent("usage.sqlite"))
        defer {
            try? store.close()
            try? FileManager.default.removeItem(at: directory)
        }
        let pipeline = UsageEventPipeline(store: store, capacity: 2)
        XCTAssertTrue(pipeline.submit(event(at: Date(), model: "a", total: 10, outcome: .completed)))
        pipeline.flush()
        XCTAssertEqual(pipeline.status().persisted, 1)
        pipeline.shutdown()
        XCTAssertFalse(pipeline.submit(event(at: Date(), model: "b", total: 10, outcome: .completed)))
        XCTAssertEqual(pipeline.status().dropped, 1)
    }

    func testEventPipelineNotifiesOnlyAfterSuccessfulPersistence() {
        let recorder = PersistedEventRecorder()
        let expected = event(at: Date(), model: "persisted", total: 10, outcome: .completed)
        let pipeline = UsageEventPipeline(store: RecoveringEventWriter()) { event in
            recorder.record(event)
        }

        XCTAssertTrue(pipeline.submit(expected))
        pipeline.flush()
        XCTAssertTrue(recorder.events.isEmpty)

        XCTAssertTrue(pipeline.submit(expected))
        pipeline.shutdown()
        XCTAssertEqual(recorder.events, [expected])
    }

    func testEventPipelineReportsStorageFailureWithoutRejectingSubmission() {
        let pipeline = UsageEventPipeline(store: FailingEventWriter(), capacity: 2)

        XCTAssertTrue(pipeline.submit(event(at: Date(), model: "a", total: 10, outcome: .completed)))
        pipeline.flush()

        XCTAssertEqual(pipeline.status().persisted, 0)
        XCTAssertEqual(pipeline.status().dropped, 1)
        XCTAssertEqual(pipeline.status().lastError, "fixture storage unavailable")
    }

    func testEventPipelineDropsNewestEventWhenBoundedQueueIsFull() {
        let writer = BlockingEventWriter()
        let pipeline = UsageEventPipeline(store: writer, capacity: 1)
        XCTAssertTrue(pipeline.submit(event(at: Date(), model: "first", total: 1, outcome: .completed)))
        XCTAssertEqual(writer.started.wait(timeout: .now() + 1), .success)

        XCTAssertTrue(pipeline.submit(event(at: Date(), model: "second", total: 1, outcome: .completed)))
        XCTAssertFalse(pipeline.submit(event(at: Date(), model: "third", total: 1, outcome: .completed)))

        writer.release.signal()
        pipeline.shutdown()
        XCTAssertEqual(pipeline.status().persisted, 2)
        XCTAssertEqual(pipeline.status().dropped, 1)
    }

    func testEventPipelineRecoversAfterTransientWriterFailure() {
        let writer = RecoveringEventWriter()
        let pipeline = UsageEventPipeline(store: writer)

        XCTAssertTrue(pipeline.submit(event(at: Date(), model: "first", total: 1, outcome: .completed)))
        pipeline.flush()
        XCTAssertEqual(pipeline.status().dropped, 1)
        XCTAssertNotNil(pipeline.status().lastError)

        XCTAssertTrue(pipeline.submit(event(at: Date(), model: "second", total: 1, outcome: .completed)))
        pipeline.shutdown()
        XCTAssertEqual(pipeline.status().persisted, 1)
        XCTAssertEqual(pipeline.status().dropped, 1)
        XCTAssertNil(pipeline.status().lastError)
    }

    private func event(
        at date: Date,
        model: String,
        total: Int64,
        outcome: RequestOutcome,
        quality: UsageQuality = .reported,
        estimatedCost: String? = nil,
        currency: String? = nil
    ) -> UsageEvent {
        UsageEvent(
            startedAt: date,
            completedAt: date.addingTimeInterval(0.1),
            routeID: "route",
            agentID: "agent",
            protocolType: .openAIResponses,
            method: "POST",
            endpointKind: .responses,
            statusCode: outcome == .failed ? 500 : 200,
            outcome: outcome,
            latencyMilliseconds: 100,
            model: model,
            keyFingerprint: nil,
            usage: NormalizedUsage(
                inputTokens: total * 3 / 4,
                outputTokens: total / 4,
                totalTokens: total
            ),
            usageQuality: quality,
            usageRaw: nil,
            requestBytes: 10,
            responseBytes: 20,
            estimatedCost: estimatedCost,
            currency: currency,
            pricingPolicyID: nil,
            pricingPolicyVersion: nil,
            errorCode: outcome == .failed ? "upstream_status" : nil
        )
    }
}

private struct FailingEventWriter: UsageEventWriting {
    func save(_ event: UsageEvent) throws {
        throw FailingEventWriterError()
    }
}

private struct FailingEventWriterError: LocalizedError {
    var errorDescription: String? { "fixture storage unavailable" }
}

private final class BlockingEventWriter: UsageEventWriting, @unchecked Sendable {
    let started = DispatchSemaphore(value: 0)
    let release = DispatchSemaphore(value: 0)
    private let lock = NSLock()
    private var writeCount = 0

    func save(_ event: UsageEvent) throws {
        lock.lock()
        writeCount += 1
        let shouldBlock = writeCount == 1
        lock.unlock()
        if shouldBlock {
            started.signal()
            release.wait()
        }
    }
}

private final class RecoveringEventWriter: UsageEventWriting, @unchecked Sendable {
    private let lock = NSLock()
    private var shouldFail = true

    func save(_ event: UsageEvent) throws {
        let fail = lock.withLock {
            defer { shouldFail = false }
            return shouldFail
        }
        if fail { throw FailingEventWriterError() }
    }
}

private final class PersistedEventRecorder: @unchecked Sendable {
    private let lock = NSLock()
    private var recordedEvents: [UsageEvent] = []

    var events: [UsageEvent] {
        lock.withLock { recordedEvents }
    }

    func record(_ event: UsageEvent) {
        lock.withLock { recordedEvents.append(event) }
    }
}

private extension NSLock {
    func withLock<T>(_ body: () throws -> T) rethrows -> T {
        lock()
        defer { unlock() }
        return try body()
    }
}
