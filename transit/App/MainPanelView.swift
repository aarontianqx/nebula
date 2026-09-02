import AppKit
import SwiftUI
import TransitCore
import TransitShared

private enum PanelSection: String, CaseIterable, Identifiable {
    case overview = "概览"
    case breakdown = "明细"
    case routes = "Routes"
    case diagnostics = "诊断"
    case settings = "设置"

    var id: String { rawValue }
}

struct MainPanelView: View {
    @EnvironmentObject private var model: AppModel
    @State private var section: PanelSection = .overview

    var body: some View {
        VStack(spacing: 0) {
            header
            Divider()
            Group {
                switch section {
                case .overview: OverviewView()
                case .breakdown: BreakdownView()
                case .routes: RoutesView()
                case .diagnostics: DiagnosticsView()
                case .settings: SettingsView()
                }
            }
            .frame(maxWidth: .infinity, maxHeight: .infinity, alignment: .topLeading)
            Divider()
            footer
        }
        .frame(width: 520, height: 520)
        .onChange(of: model.selectedPeriod) { _, _ in Task { await model.refresh() } }
        .onChange(of: model.breakdownDimension) { _, _ in Task { await model.refresh() } }
    }

    private var header: some View {
        VStack(spacing: 10) {
            HStack {
                HStack(spacing: 7) {
                    Circle()
                        .fill(statusColor)
                        .frame(width: 9, height: 9)
                    Text("Transit")
                        .font(.headline)
                    Text(statusText)
                        .font(.caption)
                        .foregroundStyle(.secondary)
                }
                Spacer()
                if model.proxyStatus.activeRequests > 0 {
                    Text("\(model.proxyStatus.activeRequests) 个活跃请求")
                        .font(.caption)
                        .foregroundStyle(.blue)
                }
            }
            Picker("", selection: $section) {
                ForEach(PanelSection.allCases) { item in Text(item.rawValue).tag(item) }
            }
            .pickerStyle(.segmented)
            .labelsHidden()
            if let error = model.lastError, !error.isEmpty {
                HStack(alignment: .top, spacing: 6) {
                    Image(systemName: "exclamationmark.triangle.fill")
                    Text(error).lineLimit(3)
                    Spacer()
                }
                .font(.caption)
                .foregroundStyle(.orange)
            }
            if model.shouldSuggestLaunchAtLogin {
                HStack(spacing: 8) {
                    Image(systemName: "power")
                    Text("建议开启登录启动，避免 Agent 指向 Transit 时代理未运行。")
                        .font(.caption)
                    Spacer()
                    Button("暂不") { model.dismissLaunchAtLoginSuggestion() }
                    Button("开启") { model.setLaunchAtLogin(true) }
                        .buttonStyle(.borderedProminent)
                }
                .controlSize(.small)
                .padding(8)
                .background(.blue.opacity(0.08), in: RoundedRectangle(cornerRadius: 8))
            }
        }
        .padding(14)
    }

    private var footer: some View {
        HStack {
            Button {
                Task { await model.refresh() }
            } label: {
                if model.isRefreshing {
                    ProgressView().controlSize(.small)
                } else {
                    Label("刷新", systemImage: "arrow.clockwise")
                }
            }
            .disabled(model.isRefreshing)
            Spacer()
            Text("\(model.proxyStatus.forwardedRequests) requests")
                .font(.caption2)
                .foregroundStyle(.tertiary)
            Button("退出") { NSApplication.shared.terminate(nil) }
        }
        .controlSize(.small)
        .padding(12)
    }

    private var statusColor: Color {
        switch model.proxyStatus.state {
        case .running: .green
        case .starting: .blue
        case .degraded: .orange
        case .stopped: .secondary
        }
    }

    private var statusText: String {
        switch model.proxyStatus.state {
        case .running: "代理运行中"
        case .starting: "正在启动"
        case .degraded: "部分异常"
        case .stopped: "代理已停止"
        }
    }
}

private struct OverviewView: View {
    @EnvironmentObject private var model: AppModel

    private var summary: UsageSummary {
        model.summaries[model.selectedPeriod] ?? UsageSummary()
    }

    var body: some View {
        ScrollView {
            VStack(alignment: .leading, spacing: 14) {
                Picker("周期", selection: $model.selectedPeriod) {
                    ForEach(WidgetPeriod.allCases, id: \.self) { Text($0.displayName).tag($0) }
                }
                .pickerStyle(.segmented)
                HStack(spacing: 10) {
                    metricCard("总 Token", formatTokens(summary.totalTokens), "sum")
                    metricCard("请求", "\(summary.requestCount)", "arrow.up.arrow.down")
                    metricCard("平均延迟", formatLatency(summary.averageLatencyMilliseconds), "timer")
                }
                HStack(spacing: 10) {
                    tokenCard("输入", summary.inputTokens, .blue)
                    tokenCard("输出", summary.outputTokens, .green)
                    tokenCard("缓存", summary.cachedInputTokens, .purple)
                    tokenCard("推理", summary.reasoningTokens, .orange)
                }
                if let cost = summary.estimatedCost, let currency = summary.currency {
                    HStack {
                        Label("估算成本", systemImage: "banknote")
                        Spacer()
                        Text("\(currency) \(NSDecimalNumber(decimal: cost).stringValue)")
                            .font(.headline.monospacedDigit())
                    }
                    .padding(12)
                    .background(.quaternary.opacity(0.5), in: RoundedRectangle(cornerRadius: 10))
                }
                GroupBox("Top Models") {
                    if model.topModels.isEmpty {
                        ContentUnavailableView("暂无用量", systemImage: "chart.bar")
                            .frame(height: 100)
                    } else {
                        VStack(spacing: 8) {
                            ForEach(model.topModels.prefix(5)) { item in
                                HStack {
                                    Text(item.model).lineLimit(1)
                                    Spacer()
                                    Text(formatTokens(item.totalTokens))
                                        .monospacedDigit()
                                }
                                .font(.callout)
                            }
                        }
                        .padding(.vertical, 4)
                    }
                }
            }
            .padding(14)
        }
    }

    private func metricCard(_ title: String, _ value: String, _ icon: String) -> some View {
        VStack(alignment: .leading, spacing: 5) {
            Label(title, systemImage: icon).font(.caption).foregroundStyle(.secondary)
            Text(value).font(.title3.weight(.semibold)).monospacedDigit()
        }
        .frame(maxWidth: .infinity, alignment: .leading)
        .padding(10)
        .background(.quaternary.opacity(0.5), in: RoundedRectangle(cornerRadius: 10))
    }

    private func tokenCard(_ title: String, _ value: Int64, _ color: Color) -> some View {
        VStack(alignment: .leading, spacing: 4) {
            Text(title).font(.caption).foregroundStyle(.secondary)
            Text(formatTokens(value)).font(.callout.weight(.semibold)).monospacedDigit()
        }
        .frame(maxWidth: .infinity, alignment: .leading)
        .padding(9)
        .background(color.opacity(0.1), in: RoundedRectangle(cornerRadius: 8))
    }
}

private struct BreakdownView: View {
    @EnvironmentObject private var model: AppModel

    var body: some View {
        VStack(alignment: .leading, spacing: 12) {
            HStack {
                Picker("维度", selection: $model.breakdownDimension) {
                    ForEach(UsageBreakdownDimension.allCases) { Text($0.displayName).tag($0) }
                }
                .pickerStyle(.segmented)
                Spacer()
                Text(model.selectedPeriod.displayName)
                    .font(.caption)
                    .foregroundStyle(.secondary)
            }
            if model.breakdown.isEmpty {
                ContentUnavailableView("暂无明细", systemImage: "list.bullet.rectangle")
                    .frame(maxWidth: .infinity, maxHeight: .infinity)
            } else {
                List(model.breakdown) { item in
                    HStack {
                        VStack(alignment: .leading, spacing: 2) {
                            Text(item.label).lineLimit(1)
                            Text("\(item.requestCount) 请求 · \(item.failedRequestCount) 失败")
                                .font(.caption2)
                                .foregroundStyle(.secondary)
                        }
                        Spacer()
                        VStack(alignment: .trailing, spacing: 2) {
                            Text(formatTokens(item.totalTokens)).monospacedDigit()
                            Text(formatLatency(item.averageLatencyMilliseconds))
                                .font(.caption2)
                                .foregroundStyle(.secondary)
                        }
                    }
                }
                .listStyle(.inset)
            }
        }
        .padding(14)
    }
}

private struct DiagnosticsView: View {
    @EnvironmentObject private var model: AppModel

    var body: some View {
        ScrollView {
            VStack(alignment: .leading, spacing: 14) {
                GroupBox("Listeners") {
                    VStack(spacing: 8) {
                        if model.proxyStatus.listeners.isEmpty {
                            Text("没有运行中的 listener").foregroundStyle(.secondary)
                        }
                        ForEach(model.proxyStatus.listeners) { listener in
                            HStack {
                                Image(systemName: listener.state == .ready ? "checkmark.circle.fill" : "exclamationmark.triangle.fill")
                                    .foregroundStyle(listener.state == .ready ? .green : .orange)
                                Text("127.0.0.1:\(listener.port)")
                                Spacer()
                                Text(listener.error ?? listener.state.rawValue)
                                    .font(.caption)
                                    .foregroundStyle(.secondary)
                            }
                        }
                    }
                    .padding(.vertical, 4)
                }
                GroupBox("采集") {
                    Grid(alignment: .leading, horizontalSpacing: 20, verticalSpacing: 8) {
                        diagnosticRow("转发请求", "\(model.proxyStatus.forwardedRequests)")
                        diagnosticRow("活跃连接", "\(model.proxyStatus.activeConnections)")
                        diagnosticRow(
                            "Usage 已报告",
                            "\(max(0, model.proxyStatus.forwardedRequests - model.proxyStatus.parseMissing))"
                        )
                        diagnosticRow("Usage 缺失", "\(model.proxyStatus.parseMissing)")
                        diagnosticRow("解析错误", "\(model.proxyStatus.parseErrors)")
                        diagnosticRow("已持久化", "\(model.pipelineStatus.persisted)")
                        diagnosticRow("队列中", "\(model.pipelineStatus.queued)")
                        diagnosticRow("丢弃事件", "\(model.pipelineStatus.dropped)")
                        diagnosticRow("存储状态", model.storageHealthText)
                        diagnosticRow("诊断存储", model.diagnosticsStorageError == nil ? "健康" : "异常")
                        diagnosticRow("Widget 快照", model.widgetSnapshotHealthText)
                    }
                    .padding(.vertical, 4)
                }
                GroupBox("上游与配置") {
                    let summary = model.summaries[model.selectedPeriod] ?? UsageSummary()
                    Grid(alignment: .leading, horizontalSpacing: 20, verticalSpacing: 8) {
                        diagnosticRow("统计周期", model.selectedPeriod.displayName)
                        diagnosticRow("失败请求", "\(summary.failedRequestCount)")
                        diagnosticRow("平均延迟", formatLatency(summary.averageLatencyMilliseconds))
                        diagnosticRow("配置应用", model.lastConfigurationResult)
                    }
                    .padding(.vertical, 4)
                }
                GroupBox("请求 Flight Recorder") {
                    if model.requestDiagnostics.isEmpty {
                        Text("没有活跃或异常请求")
                            .foregroundStyle(.secondary)
                    } else {
                        VStack(spacing: 9) {
                            ForEach(model.requestDiagnostics.prefix(20)) { item in
                                VStack(alignment: .leading, spacing: 4) {
                                    HStack {
                                        Image(systemName: diagnosticIcon(item.state))
                                            .foregroundStyle(diagnosticColor(item.state))
                                        Text(item.routeID).lineLimit(1)
                                        Text(item.protocolType.displayName)
                                            .foregroundStyle(.secondary)
                                        Spacer()
                                        Text(item.state.displayName)
                                    }
                                    HStack {
                                        Text(item.phase.displayName)
                                        if let activity = item.lastResponseActivity {
                                            Text("· \(activity.displayName)")
                                        }
                                        Spacer()
                                        if let status = item.statusCode { Text("HTTP \(status)") }
                                        Text("↑\(formatBytes(item.requestBytes)) ↓\(formatBytes(item.responseBytes))")
                                        Text(item.updatedAt, style: .time)
                                    }
                                    .foregroundStyle(.secondary)
                                }
                                .font(.caption)
                            }
                        }
                        .padding(.vertical, 4)
                    }
                }
                GroupBox("HTTP 状态分布") {
                    if model.statusDistribution.isEmpty {
                        Text("暂无请求").foregroundStyle(.secondary)
                    } else {
                        VStack(spacing: 7) {
                            ForEach(model.statusDistribution) { item in
                                HStack {
                                    Text(item.displayName)
                                    Spacer()
                                    Text("\(item.requestCount)").monospacedDigit()
                                }
                            }
                        }
                        .font(.caption)
                        .padding(.vertical, 4)
                    }
                }
                if !model.configurationIssues.isEmpty {
                    GroupBox("配置") {
                        VStack(alignment: .leading, spacing: 7) {
                            ForEach(model.configurationIssues) { issue in
                                HStack(alignment: .top, spacing: 7) {
                                    Image(systemName: issue.severity == .error
                                        ? "xmark.circle.fill"
                                        : "exclamationmark.triangle.fill")
                                        .foregroundStyle(issue.severity == .error ? .red : .orange)
                                    VStack(alignment: .leading, spacing: 2) {
                                        Text(issue.message)
                                        Text(issue.path)
                                            .font(.caption2.monospaced())
                                            .foregroundStyle(.tertiary)
                                    }
                                }
                                .font(.caption)
                            }
                        }
                        .padding(.vertical, 4)
                    }
                }
                if let storageError = model.pipelineStatus.lastError {
                    GroupBox("存储") {
                        Label(storageError, systemImage: "externaldrive.badge.exclamationmark")
                            .font(.caption)
                            .foregroundStyle(.orange)
                            .padding(.vertical, 4)
                    }
                }
                if let diagnosticsError = model.diagnosticsStorageError {
                    GroupBox("诊断存储") {
                        Label(diagnosticsError, systemImage: "externaldrive.badge.exclamationmark")
                            .font(.caption)
                            .foregroundStyle(.orange)
                            .padding(.vertical, 4)
                    }
                }
                if let widgetError = model.widgetSnapshotError {
                    GroupBox("Widget") {
                        Label(widgetError, systemImage: "rectangle.badge.exclamationmark")
                            .font(.caption)
                            .foregroundStyle(.orange)
                            .padding(.vertical, 4)
                    }
                }
                GroupBox("最近请求") {
                    if model.recentEvents.isEmpty {
                        Text("暂无请求").foregroundStyle(.secondary)
                    } else {
                        VStack(spacing: 7) {
                            ForEach(model.recentEvents.prefix(10)) { event in
                                HStack {
                                    Image(systemName: event.outcome == .completed ? "checkmark.circle" : "exclamationmark.circle")
                                        .foregroundStyle(event.outcome == .completed ? .green : .orange)
                                    Text(event.model ?? event.endpointKind.rawValue).lineLimit(1)
                                    Spacer()
                                    Text(formatTokens(event.usage.totalTokens ?? 0)).monospacedDigit()
                                    Text(event.completedAt, style: .time)
                                        .foregroundStyle(.tertiary)
                                }
                                .font(.caption)
                            }
                        }
                    }
                }
            }
            .padding(14)
        }
    }

    private func diagnosticRow(_ title: String, _ value: String) -> some View {
        GridRow {
            Text(title).foregroundStyle(.secondary)
            Text(value).monospacedDigit()
        }
    }

    private func diagnosticIcon(_ state: RequestDiagnosticState) -> String {
        switch state {
        case .active: "arrow.triangle.2.circlepath"
        case .stalled: "exclamationmark.triangle.fill"
        case .completed: "checkmark.circle"
        case .failed: "xmark.circle.fill"
        case .cancelled: "slash.circle"
        }
    }

    private func diagnosticColor(_ state: RequestDiagnosticState) -> Color {
        switch state {
        case .active: .blue
        case .stalled: .orange
        case .completed: .green
        case .failed: .red
        case .cancelled: .secondary
        }
    }
}

func formatTokens(_ value: Int64) -> String {
    switch value {
    case 1_000_000...: String(format: "%.1fM", Double(value) / 1_000_000)
    case 1_000...: String(format: "%.1fK", Double(value) / 1_000)
    default: "\(value)"
    }
}

private func formatLatency(_ value: Double) -> String {
    value >= 1_000 ? String(format: "%.1fs", value / 1_000) : "\(Int(value.rounded()))ms"
}

private func formatBytes(_ value: Int64) -> String {
    value >= 1_024 ? String(format: "%.1fKB", Double(value) / 1_024) : "\(value)B"
}
