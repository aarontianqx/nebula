import Foundation
import NIOCore
import XCTest
@testable import TransitCore

final class RequestFlightRecorderTests: XCTestCase {
    func testStoreUpsertsOneSnapshotAndMarksInterruptedRequestAfterRestart() throws {
        let (store, databaseURL, directory) = try makeStore()
        var snapshot = makeSnapshot(id: "request-1", state: .active)
        try store.upsert(snapshot)
        snapshot.phase = .heartbeatActivity
        snapshot.lastResponseActivity = .heartbeat
        snapshot.responseBytes = 42
        try store.upsert(snapshot)

        XCTAssertEqual(try store.recent().count, 1)
        XCTAssertEqual(try store.recent().first?.responseBytes, 42)
        try store.close()

        let reopened = try RequestDiagnosticsStore(databaseURL: databaseURL)
        let restored = try XCTUnwrap(reopened.recent().first)
        XCTAssertEqual(restored.state, .stalled)
        XCTAssertEqual(restored.errorCode, "app_restarted")
        XCTAssertEqual(restored.lastResponseActivity, .heartbeat)
        try reopened.delete(id: restored.id)
        XCTAssertTrue(try reopened.recent().isEmpty)
        try reopened.close()
        try FileManager.default.removeItem(at: directory)
    }

    func testStorePrunesByAgeAndCount() throws {
        let (store, _, directory) = try makeStore()
        defer {
            try? store.close()
            try? FileManager.default.removeItem(at: directory)
        }
        let now = Date()
        for index in 0..<205 {
            var snapshot = makeSnapshot(id: "request-\(index)", state: .failed)
            snapshot.updatedAt = now.addingTimeInterval(TimeInterval(-index))
            try store.upsert(snapshot)
        }
        var expired = makeSnapshot(id: "expired", state: .failed)
        expired.updatedAt = now.addingTimeInterval(-8 * 86_400)
        try store.upsert(expired)

        _ = try store.prune(olderThan: now.addingTimeInterval(-7 * 86_400), retaining: 200)

        let retained = try store.recent(limit: 200)
        XCTAssertEqual(retained.count, 200)
        XCTAssertFalse(retained.contains { $0.id == "expired" })
    }

    func testRecorderDeletesCompletedAndRetainsFailedRequests() throws {
        let (store, _, directory) = try makeStore()
        let recorder = RequestFlightRecorder(store: store, stallThreshold: 60)
        defer {
            recorder.close()
            try? FileManager.default.removeItem(at: directory)
        }

        let completedID = recorder.start(routeID: "route-a", protocolType: .openAIChat, method: "POST")
        recorder.upstreamStarted(id: completedID)
        recorder.finish(id: completedID, state: .completed, errorCode: nil)
        recorder.flush()
        XCTAssertTrue(recorder.snapshots().isEmpty)

        let failedID = recorder.start(routeID: "route-b", protocolType: .anthropicMessages, method: "POST")
        recorder.finish(id: failedID, state: .failed, errorCode: "upstream_request_failed")
        recorder.flush()
        let failed = try XCTUnwrap(recorder.snapshots().first)
        XCTAssertEqual(failed.id, failedID)
        XCTAssertEqual(failed.state, .failed)
        XCTAssertEqual(failed.errorCode, "upstream_request_failed")
    }

    func testHeartbeatDoesNotClearStallButMeaningfulDataDoes() throws {
        let (store, _, directory) = try makeStore()
        let recorder = RequestFlightRecorder(store: store, stallThreshold: 10)
        defer {
            recorder.close()
            try? FileManager.default.removeItem(at: directory)
        }
        let id = recorder.start(routeID: "route", protocolType: .openAIChat, method: "POST")
        recorder.upstreamStarted(id: id)
        let active = try XCTUnwrap(recorder.snapshots().first)

        recorder.detectStalls(referenceDate: active.lastMeaningfulActivityAt.addingTimeInterval(11))
        XCTAssertEqual(recorder.snapshots().first?.state, .stalled)

        recorder.upstreamActivity(id: id, kind: .heartbeat, responseBytes: 12)
        XCTAssertEqual(recorder.snapshots().first?.state, .stalled)
        XCTAssertEqual(recorder.snapshots().first?.lastResponseActivity, .heartbeat)

        recorder.upstreamActivity(id: id, kind: .meaningfulSSE, responseBytes: 24)
        XCTAssertEqual(recorder.snapshots().first?.state, .active)
        XCTAssertEqual(recorder.snapshots().first?.lastResponseActivity, .meaningfulSSE)
    }

    func testSSEClassifierDistinguishesSplitHeartbeatAndData() {
        var classifier = SSEActivityClassifier()
        XCTAssertEqual(
            classifier.classify(contentType: "text/event-stream", bytes: ByteBuffer(string: ": heart")),
            .sseTransport
        )
        XCTAssertEqual(
            classifier.classify(contentType: "text/event-stream", bytes: ByteBuffer(string: "beat\n\n")),
            .heartbeat
        )
        XCTAssertEqual(
            classifier.classify(contentType: "text/event-stream", bytes: ByteBuffer(string: "data: {\"type\"")),
            .sseTransport
        )
        XCTAssertEqual(
            classifier.classify(contentType: "text/event-stream", bytes: ByteBuffer(string: ":\"delta\"}\n\n")),
            .meaningfulSSE
        )
        XCTAssertEqual(
            classifier.classify(contentType: "application/json", bytes: ByteBuffer(string: "{}")),
            .body
        )
    }

    private func makeStore() throws -> (RequestDiagnosticsStore, URL, URL) {
        let directory = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString)
        let databaseURL = directory.appendingPathComponent("diagnostics.sqlite")
        return (try RequestDiagnosticsStore(databaseURL: databaseURL), databaseURL, directory)
    }

    private func makeSnapshot(id: String, state: RequestDiagnosticState) -> RequestDiagnosticSnapshot {
        let now = Date()
        return RequestDiagnosticSnapshot(
            id: id,
            routeID: "fixture-route",
            protocolType: .openAIResponses,
            method: "POST",
            phase: .upstreamRequestStarted,
            state: state,
            startedAt: now,
            updatedAt: now,
            lastUpstreamActivityAt: nil,
            lastMeaningfulActivityAt: now,
            lastDownstreamWriteAt: nil,
            lastResponseActivity: nil,
            requestBytes: 0,
            responseBytes: 0,
            statusCode: nil,
            errorCode: nil
        )
    }
}
