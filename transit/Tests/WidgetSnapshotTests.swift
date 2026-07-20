import Foundation
import TransitShared
import XCTest

final class WidgetSnapshotTests: XCTestCase {
    func testSnapshotRoundTripKeepsEveryPeriodAndStaleState() throws {
        let directory = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString)
        let url = directory.appendingPathComponent("widget_snapshot.json")
        defer { try? FileManager.default.removeItem(at: directory) }
        let generatedAt = Date(timeIntervalSince1970: 1_000)
        let snapshot = WidgetSnapshot(
            generatedAt: generatedAt,
            proxyState: .degraded,
            eventsComplete: false,
            periods: Dictionary(uniqueKeysWithValues: WidgetPeriod.allCases.enumerated().map { index, period in
                (period, WidgetUsageSummary(
                    totalTokens: Int64(index + 1) * 100,
                    inputTokens: 50,
                    outputTokens: 25,
                    cachedInputTokens: 20,
                    reasoningTokens: 5,
                    estimatedCost: index == 0 ? "1.25" : nil,
                    currency: index == 0 ? "USD" : nil,
                    topModels: [WidgetModelSummary(
                        id: "model-\(index)",
                        model: "model-\(index)",
                        totalTokens: 100
                    )]
                ))
            })
        )

        try WidgetSnapshotStore.save(snapshot, to: url)
        let decoded = try XCTUnwrap(WidgetSnapshotStore.load(from: url))

        XCTAssertEqual(decoded, snapshot)
        XCTAssertEqual(decoded.periods.count, WidgetPeriod.allCases.count)
        XCTAssertFalse(decoded.isStale(at: generatedAt.addingTimeInterval(300)))
        XCTAssertTrue(decoded.isStale(at: generatedAt.addingTimeInterval(301)))

        let object = try XCTUnwrap(
            JSONSerialization.jsonObject(with: Data(contentsOf: url)) as? [String: Any]
        )
        let periods = try XCTUnwrap(object["periods"] as? [String: Any])
        XCTAssertEqual(Set(periods.keys), Set(WidgetPeriod.allCases.map(\.rawValue)))
    }

    func testWidgetPrimaryMetricsSelectTheirOwnValues() {
        let summary = WidgetUsageSummary(
            totalTokens: 100,
            inputTokens: 60,
            outputTokens: 40,
            cachedInputTokens: 30,
            reasoningTokens: 10,
            estimatedCost: nil,
            currency: nil,
            topModels: []
        )

        XCTAssertEqual(WidgetPrimaryMetric.total.value(in: summary), 100)
        XCTAssertEqual(WidgetPrimaryMetric.input.value(in: summary), 60)
        XCTAssertEqual(WidgetPrimaryMetric.output.value(in: summary), 40)
        XCTAssertEqual(WidgetPrimaryMetric.cachedInput.value(in: summary), 30)
        XCTAssertEqual(WidgetPrimaryMetric.reasoning.value(in: summary), 10)
    }
}
