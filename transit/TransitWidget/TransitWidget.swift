import SwiftUI
import TransitShared
import WidgetKit
import AppIntents

@main
struct TransitWidgetBundle: WidgetBundle {
    var body: some Widget {
        TransitUsageWidget()
    }
}

struct TransitUsageWidget: Widget {
    var body: some WidgetConfiguration {
        StaticConfiguration(kind: "TransitUsageWidget", provider: TransitTimelineProvider()) { entry in
            TransitWidgetView(entry: entry)
                .containerBackground(.background, for: .widget)
        }
        .configurationDisplayName("Transit 用量")
        .description("查看本机代理观测到的模型用量")
        .supportedFamilies([.systemSmall, .systemMedium])
    }
}

struct TransitEntry: TimelineEntry {
    let date: Date
    let snapshot: WidgetSnapshot?
}

struct TransitTimelineProvider: TimelineProvider {
    func placeholder(in context: Context) -> TransitEntry {
        TransitEntry(date: Date(), snapshot: .preview)
    }

    func getSnapshot(in context: Context, completion: @escaping (TransitEntry) -> Void) {
        completion(TransitEntry(date: Date(), snapshot: WidgetSnapshotStore.load() ?? .preview))
    }

    func getTimeline(in context: Context, completion: @escaping (Timeline<TransitEntry>) -> Void) {
        let entry = TransitEntry(date: Date(), snapshot: WidgetSnapshotStore.load())
        let refresh = Calendar.current.date(byAdding: .minute, value: 15, to: Date())!
        completion(Timeline(entries: [entry], policy: .after(refresh)))
    }
}

private struct TransitWidgetView: View {
    @Environment(\.widgetFamily) private var family
    let entry: TransitEntry

    var body: some View {
        if let snapshot = entry.snapshot {
            let period = WidgetSnapshotStore.loadSelectedPeriod()
            let primaryMetric = WidgetSnapshotStore.loadPrimaryMetric()
            let summary = snapshot.summary(for: period)
            let stale = snapshot.isStale()
            VStack(alignment: .leading, spacing: 6) {
                HStack {
                    Image(systemName: stale ? "clock.badge.exclamationmark" : stateIcon(snapshot.proxyState))
                        .foregroundStyle(stale ? .orange : stateColor(snapshot.proxyState))
                    Button(intent: ToggleWidgetPeriodIntent()) {
                        Text(period.displayName)
                    }
                    .buttonStyle(.plain)
                        .font(.caption.weight(.medium))
                    Spacer()
                }
                Text(formatTokens(primaryMetric.value(in: summary)))
                    .font(.system(.title2, design: .rounded).weight(.bold))
                    .monospacedDigit()
                Text(primaryMetric.displayName)
                    .font(.caption2)
                    .foregroundStyle(.secondary)
                if family == .systemMedium {
                    HStack {
                        metric("输入", summary.inputTokens)
                        metric("输出", summary.outputTokens)
                        metric("缓存", summary.cachedInputTokens)
                        metric("推理", summary.reasoningTokens)
                    }
                    if let model = summary.topModels.first {
                        HStack {
                            Text(model.model).lineLimit(1)
                            Spacer()
                            Text(formatTokens(model.totalTokens)).monospacedDigit()
                        }
                        .font(.caption2)
                        .foregroundStyle(.secondary)
                    }
                }
                Spacer(minLength: 0)
                HStack {
                    if stale || !snapshot.eventsComplete {
                        Text(stale ? "数据已过期" : "数据不完整")
                            .foregroundStyle(.orange)
                    }
                    Spacer()
                    if let cost = summary.estimatedCost, let currency = summary.currency {
                        Text("\(currency) \(cost)")
                    }
                    Text(snapshot.generatedAt, style: .time)
                }
                .font(.caption2)
                .foregroundStyle(.tertiary)
            }
        } else {
            VStack(spacing: 6) {
                Image(systemName: "arrow.left.arrow.right.circle")
                    .font(.title2)
                Text("等待 Transit 数据")
                    .font(.caption)
            }
            .frame(maxWidth: .infinity, maxHeight: .infinity)
            .foregroundStyle(.secondary)
        }
    }

    private func metric(_ title: String, _ value: Int64) -> some View {
        VStack(alignment: .leading, spacing: 1) {
            Text(title).font(.caption2).foregroundStyle(.secondary)
            Text(formatTokens(value)).font(.caption.weight(.semibold)).monospacedDigit()
        }
        .frame(maxWidth: .infinity, alignment: .leading)
    }

    private func stateIcon(_ state: WidgetProxyState) -> String {
        switch state {
        case .running: "checkmark.circle.fill"
        case .starting: "clock.fill"
        case .degraded: "exclamationmark.triangle.fill"
        case .stopped: "stop.circle.fill"
        }
    }

    private func stateColor(_ state: WidgetProxyState) -> Color {
        switch state {
        case .running: .green
        case .starting: .blue
        case .degraded: .orange
        case .stopped: .secondary
        }
    }
}

private func formatTokens(_ value: Int64) -> String {
    switch value {
    case 1_000_000...: String(format: "%.1fM", Double(value) / 1_000_000)
    case 1_000...: String(format: "%.1fK", Double(value) / 1_000)
    default: "\(value)"
    }
}

private extension WidgetSnapshot {
    static let preview = WidgetSnapshot(
        proxyState: .running,
        eventsComplete: true,
        periods: [.today: WidgetUsageSummary(
            totalTokens: 1_284_500,
            inputTokens: 1_052_000,
            outputTokens: 232_500,
            cachedInputTokens: 640_000,
            reasoningTokens: 28_000,
            estimatedCost: nil,
            currency: nil,
            topModels: []
        )]
    )
}

struct ToggleWidgetPeriodIntent: AppIntent {
    static let title: LocalizedStringResource = "切换统计周期"

    func perform() async throws -> some IntentResult {
        let current = WidgetSnapshotStore.loadSelectedPeriod()
        let all = WidgetPeriod.allCases
        let index = all.firstIndex(of: current) ?? 0
        WidgetSnapshotStore.saveSelectedPeriod(all[(index + 1) % all.count])
        WidgetCenter.shared.reloadAllTimelines()
        return .result()
    }
}
