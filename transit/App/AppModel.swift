import AppKit
import Foundation
import ServiceManagement
import TransitCore
import TransitShared
import WidgetKit

@MainActor
final class AppModel: ObservableObject {
    @Published private(set) var configuration = TransitConfiguration()
    @Published private(set) var proxyStatus = ProxyStatus(state: .stopped)
    @Published private(set) var pipelineStatus = EventPipelineStatus(
        queued: 0,
        persisted: 0,
        dropped: 0,
        lastError: nil
    )
    @Published private(set) var summaries: [WidgetPeriod: UsageSummary] = [:]
    @Published private(set) var topModels: [ModelUsageSummary] = []
    @Published private(set) var statusDistribution: [HTTPStatusSummary] = []
    @Published private(set) var breakdown: [UsageBreakdownItem] = []
    @Published private(set) var recentEvents: [UsageEvent] = []
    @Published private(set) var configurationIssues: [ConfigurationIssue] = []
    @Published private(set) var lastError: String?
    @Published private(set) var routeTestResults: [String: String] = [:]
    @Published var selectedPeriod: WidgetPeriod = .today
    @Published private(set) var widgetPrimaryMetric: WidgetPrimaryMetric = .total
    @Published var breakdownDimension: UsageBreakdownDimension = .model
    @Published private(set) var isStarted = false
    @Published private(set) var isRefreshing = false
    @Published private(set) var launchAtLoginEnabled = false
    @Published private(set) var shouldSuggestLaunchAtLogin = false
    @Published private(set) var lastConfigurationResult = "尚未应用配置"
    @Published private(set) var widgetSnapshotError: String?
    @Published private(set) var keychainSecretReferences: [String] = []

    private let configurationStore = ConfigurationStore()
    private let secretStore = KeychainSecretStore()
    private var eventStore: EventStore?
    private var pipeline: UsageEventPipeline?
    private var proxyService: ProxyService?
    private var heartbeatTask: Task<Void, Never>?
    private var scheduledRefreshTask: Task<Void, Never>?
    private var dataRefreshPending = false
    private var lastPruneDay: Date?
    private var modelsByPeriod: [WidgetPeriod: [ModelUsageSummary]] = [:]
    private var configurationReadError: String?
    private var infrastructureError: String?
    private let launchSuggestionDismissedKey = "launch_at_login_suggestion_dismissed"
    private var lastWidgetSnapshot: WidgetSnapshot?

    init() {
        let refreshRelay = AppRefreshRelay()
        launchAtLoginEnabled = SMAppService.mainApp.status == .enabled
        widgetPrimaryMetric = WidgetSnapshotStore.loadPrimaryMetric()
        var initializationErrors: [String] = []
        do {
            configuration = try configurationStore.load()
        } catch {
            configurationReadError = "配置无法读取：\(error.localizedDescription)"
            initializationErrors.append(configurationReadError!)
        }

        let createdPipeline: UsageEventPipeline
        do {
            let eventStore = try EventStore()
            self.eventStore = eventStore
            createdPipeline = UsageEventPipeline(store: eventStore) { _ in
                refreshRelay.requestDataRefresh()
            }
        } catch {
            let message = "用量数据库不可用：\(error.localizedDescription)"
            infrastructureError = message
            initializationErrors.append(message)
            createdPipeline = UsageEventPipeline(store: UnavailableEventWriter(message: message)) { _ in
                refreshRelay.requestDataRefresh()
            }
        }
        pipeline = createdPipeline
        proxyService = ProxyService(eventPipeline: createdPipeline) { status in
            refreshRelay.publish(status: status)
        }
        refreshSecretReferences()
        lastError = initializationErrors.isEmpty ? nil : initializationErrors.joined(separator: "\n")
        refreshRelay.setHandler { [weak self] signal in
            Task { @MainActor [weak self] in
                guard let self else { return }
                switch signal {
                case .dataPersisted:
                    self.scheduleDataRefresh()
                case .proxyStatus(let status):
                    self.updateProxyStatus(status)
                }
            }
        }
    }

    deinit {
        heartbeatTask?.cancel()
        scheduledRefreshTask?.cancel()
    }

    func start() {
        guard !isStarted else { return }
        isStarted = true
        configurationIssues = validateConfiguration(configuration)
        shouldSuggestLaunchAtLogin = configuration.routes.contains(where: \.enabled)
            && !launchAtLoginEnabled
            && !UserDefaults.standard.bool(forKey: launchSuggestionDismissedKey)
        Task {
            await applyRunnableConfiguration()
            await refresh()
        }
        heartbeatTask = Task { [weak self] in
            while !Task.isCancelled {
                do {
                    try await Task.sleep(for: .seconds(60))
                } catch {
                    return
                }
                guard let self else { return }
                await self.heartbeat()
            }
        }
    }

    func applyConfiguration(_ newConfiguration: TransitConfiguration) async -> Bool {
        let hadEnabledRoutes = configuration.routes.contains(where: \.enabled)
        let issues = validateConfiguration(newConfiguration)
        configurationIssues = issues
        guard !issues.contains(where: { $0.severity == .error }) else {
            lastError = issues.filter { $0.severity == .error }.map(\.message).joined(separator: "\n")
            lastConfigurationResult = "校验失败"
            return false
        }
        do {
            let prepared = try await proxyService?.prepare(configuration: newConfiguration)
            do {
                try configurationStore.save(newConfiguration)
                if let prepared { await proxyService?.commit(prepared) }
                configuration = newConfiguration
                configurationReadError = nil
                lastError = infrastructureError
                lastConfigurationResult = "配置已应用"
                if !hadEnabledRoutes,
                   newConfiguration.routes.contains(where: \.enabled),
                   !launchAtLoginEnabled,
                   !UserDefaults.standard.bool(forKey: launchSuggestionDismissedKey) {
                    shouldSuggestLaunchAtLogin = true
                }
                await pruneIfNeeded(force: true)
                await refresh()
                return true
            } catch {
                if let prepared { await proxyService?.discard(prepared) }
                throw error
            }
        } catch {
            lastError = error.localizedDescription
            lastConfigurationResult = "应用失败：\(error.localizedDescription)"
            return false
        }
    }

    func setProxyRunning(_ running: Bool) async {
        if running {
            await applyRunnableConfiguration()
        } else {
            await proxyService?.stop()
            proxyStatus = proxyService?.status() ?? ProxyStatus(state: .stopped)
            await refreshWidgetSnapshot(force: true)
        }
    }

    func refresh() async {
        guard !isRefreshing else {
            updateOperationalStatus()
            return
        }
        guard let eventStore else {
            updateOperationalStatus()
            await refreshWidgetSnapshot()
            return
        }
        isRefreshing = true
        let period = selectedPeriod
        let dimension = breakdownDimension
        typealias RefreshData = (
            [WidgetPeriod: UsageSummary],
            [WidgetPeriod: [ModelUsageSummary]],
            [HTTPStatusSummary],
            [UsageBreakdownItem],
            [UsageEvent]
        )
        let result: Result<RefreshData, Error> = await Task.detached(priority: .utility) {
            do {
                var summaries: [WidgetPeriod: UsageSummary] = [:]
                var models: [WidgetPeriod: [ModelUsageSummary]] = [:]
                for candidate in WidgetPeriod.allCases {
                    let query = UsageQuery(since: Self.startDate(for: candidate))
                    summaries[candidate] = try eventStore.summary(query)
                    models[candidate] = try eventStore.topModels(query)
                }
                let query = UsageQuery(since: Self.startDate(for: period))
                return .success((
                    summaries,
                    models,
                    try eventStore.statusDistribution(query),
                    try eventStore.breakdown(query, dimension: dimension),
                    try eventStore.recentEvents(limit: 50)
                ))
            } catch {
                return .failure(error)
            }
        }.value

        switch result {
        case .success(let value):
            if summaries != value.0 { summaries = value.0 }
            modelsByPeriod = value.1
            let refreshedTopModels = value.1[period] ?? []
            if topModels != refreshedTopModels { topModels = refreshedTopModels }
            if statusDistribution != value.2 { statusDistribution = value.2 }
            if breakdown != value.3 { breakdown = value.3 }
            if recentEvents != value.4 { recentEvents = value.4 }
            let refreshedError = pipeline?.status().lastError ?? infrastructureError ?? configurationReadError
            if lastError != refreshedError { lastError = refreshedError }
        case .failure(let error):
            if lastError != error.localizedDescription { lastError = error.localizedDescription }
        }
        updateOperationalStatus()
        isRefreshing = false
        await pruneIfNeeded(force: false)
        await refreshWidgetSnapshot()
    }

    func testRoute(_ route: RouteConfiguration) async {
        routeTestResults[route.id] = "测试中…"
        guard let url = URL(string: route.upstream) else {
            routeTestResults[route.id] = "URL 无效"
            return
        }
        do {
            var request = URLRequest(url: url)
            request.httpMethod = "HEAD"
            request.timeoutInterval = 15
            request.setValue("identity", forHTTPHeaderField: "Accept-Encoding")
            switch route.authentication.mode {
            case .passthrough:
                break
            case .replaceBearer:
                let secret = try secretStore.read(reference: route.authentication.secretRef ?? "")
                request.setValue("Bearer \(secret)", forHTTPHeaderField: "Authorization")
            case .replaceHeader:
                let secret = try secretStore.read(reference: route.authentication.secretRef ?? "")
                request.setValue(secret, forHTTPHeaderField: route.authentication.headerName ?? "x-api-key")
            }
            let (_, response) = try await URLSession.shared.data(for: request)
            let status = (response as? HTTPURLResponse)?.statusCode ?? 0
            routeTestResults[route.id] = "HTTP \(status)"
        } catch {
            routeTestResults[route.id] = error.localizedDescription
        }
    }

    func saveSecret(_ value: String, reference: String? = nil) throws -> String {
        let resolved = reference?.isEmpty == false ? reference! : "secret-\(UUID().uuidString.lowercased())"
        try secretStore.save(value, reference: resolved)
        refreshSecretReferences()
        return resolved
    }

    func deleteSecret(reference: String) async {
        guard !configuration.routes.contains(where: { $0.authentication.secretRef == reference }) else {
            lastError = "Credential \(reference) is still referenced by a Route. Edit or delete that Route first."
            return
        }
        do {
            try secretStore.delete(reference: reference)
            refreshSecretReferences()
        } catch {
            lastError = error.localizedDescription
        }
    }

    func report(_ error: Error) {
        lastError = error.localizedDescription
    }

    func setLaunchAtLogin(_ enabled: Bool) {
        do {
            if enabled {
                try SMAppService.mainApp.register()
            } else {
                try SMAppService.mainApp.unregister()
            }
            launchAtLoginEnabled = SMAppService.mainApp.status == .enabled
            if launchAtLoginEnabled {
                shouldSuggestLaunchAtLogin = false
                UserDefaults.standard.set(true, forKey: launchSuggestionDismissedKey)
            }
            lastError = nil
        } catch {
            launchAtLoginEnabled = SMAppService.mainApp.status == .enabled
            lastError = error.localizedDescription
        }
    }

    func dismissLaunchAtLoginSuggestion() {
        shouldSuggestLaunchAtLogin = false
        UserDefaults.standard.set(true, forKey: launchSuggestionDismissedKey)
    }

    func setWidgetPrimaryMetric(_ metric: WidgetPrimaryMetric) {
        widgetPrimaryMetric = metric
        WidgetSnapshotStore.savePrimaryMetric(metric)
        WidgetCenter.shared.reloadAllTimelines()
    }

    var hasEnabledRoutes: Bool {
        configuration.routes.contains(where: \.enabled)
    }

    var storageHealthText: String {
        guard eventStore != nil else { return "不可用" }
        return pipelineStatus.lastError != nil || pipelineStatus.dropped > 0 ? "部分异常" : "健康"
    }

    var widgetSnapshotHealthText: String {
        guard widgetSnapshotError == nil, let snapshot = lastWidgetSnapshot else { return "过期" }
        if snapshot.isStale() { return "过期" }
        return snapshot.eventsComplete ? "最新" : "数据不完整"
    }

    func shutdown(gracePeriod: Duration = .seconds(10)) async {
        let heartbeat = heartbeatTask
        heartbeat?.cancel()
        heartbeatTask = nil
        scheduledRefreshTask?.cancel()
        scheduledRefreshTask = nil
        dataRefreshPending = false
        await heartbeat?.value
        try? await proxyService?.shutdown(gracePeriod: gracePeriod)
        pipeline?.shutdown()
        try? eventStore?.checkpoint()
        try? eventStore?.close()
    }

    private func scheduleDataRefresh() {
        dataRefreshPending = true
        guard scheduledRefreshTask == nil else { return }
        scheduledRefreshTask = Task { [weak self] in
            do {
                try await Task.sleep(for: .milliseconds(250))
            } catch {
                return
            }
            guard let self else { return }
            await self.runScheduledRefresh()
        }
    }

    private func runScheduledRefresh() async {
        scheduledRefreshTask = nil
        guard dataRefreshPending else { return }
        guard !isRefreshing else {
            scheduleDataRefresh()
            return
        }
        dataRefreshPending = false
        await refresh()
        if dataRefreshPending { scheduleDataRefresh() }
    }

    private func heartbeat() async {
        updateOperationalStatus()
        let previousPruneDay = lastPruneDay
        await pruneIfNeeded(force: false)
        if previousPruneDay != lastPruneDay {
            await refresh()
        } else {
            await refreshWidgetSnapshot()
        }
    }

    private func updateOperationalStatus() {
        updateProxyStatus(proxyService?.status() ?? ProxyStatus(state: .stopped))
        if let refreshedPipelineStatus = pipeline?.status(), pipelineStatus != refreshedPipelineStatus {
            pipelineStatus = refreshedPipelineStatus
        }
    }

    private func updateProxyStatus(_ status: ProxyStatus) {
        if proxyStatus != status { proxyStatus = status }
    }

    private func applyRunnableConfiguration() async {
        guard let proxyService else { return }
        let issues = validateConfiguration(configuration)
        configurationIssues = issues
        let invalidRouteIDs = Set(issues.compactMap(\.routeID))
        var runnable = configuration
        runnable.routes.removeAll { invalidRouteIDs.contains($0.id) }
        do {
            if proxyService.status().listeners.isEmpty {
                try await proxyService.startBestEffort(configuration: runnable)
            } else {
                try await proxyService.apply(configuration: runnable)
            }
            proxyStatus = proxyService.status()
            lastError = issues.first(where: { $0.routeID == nil && $0.severity == .error })?.message
            lastConfigurationResult = proxyStatus.state == .degraded ? "部分 listener 启动失败" : "启动配置已应用"
        } catch {
            proxyStatus = proxyService.status()
            lastError = error.localizedDescription
            lastConfigurationResult = "启动配置失败：\(error.localizedDescription)"
        }
    }

    private func pruneIfNeeded(force: Bool) async {
        guard let eventStore else { return }
        let startOfToday = Calendar.current.startOfDay(for: Date())
        guard force || lastPruneDay != startOfToday else { return }
        lastPruneDay = startOfToday
        let cutoff = Calendar.current.date(
            byAdding: .day,
            value: -configuration.storage.retentionDays,
            to: Date()
        ) ?? .distantPast
        let result: Result<Void, Error> = await Task.detached(priority: .utility) {
            do {
                _ = try eventStore.prune(olderThan: cutoff)
                try eventStore.checkpoint()
                return .success(())
            } catch {
                return .failure(error)
            }
        }.value
        if case .failure(let error) = result {
            lastError = error.localizedDescription
        }
    }

    private func refreshWidgetSnapshot(force: Bool = false) async {
        let state: WidgetProxyState
        switch proxyStatus.state {
        case .stopped: state = .stopped
        case .starting: state = .starting
        case .running: state = .running
        case .degraded: state = .degraded
        }
        let periods = Dictionary(uniqueKeysWithValues: WidgetPeriod.allCases.map { period in
            let summary = summaries[period] ?? UsageSummary()
            let models = modelsByPeriod[period] ?? []
            return (period, WidgetUsageSummary(
                totalTokens: summary.totalTokens,
                inputTokens: summary.inputTokens,
                outputTokens: summary.outputTokens,
                cachedInputTokens: summary.cachedInputTokens,
                reasoningTokens: summary.reasoningTokens,
                estimatedCost: summary.estimatedCost.map { NSDecimalNumber(decimal: $0).stringValue },
                currency: summary.currency,
                topModels: models.map {
                    WidgetModelSummary(id: $0.model, model: $0.model, totalTokens: $0.totalTokens)
                }
            ))
        })
        let snapshot = WidgetSnapshot(
            proxyState: state,
            eventsComplete: eventStore != nil
                && pipelineStatus.dropped == 0
                && summaries.values.allSatisfy(\.dataComplete),
            periods: periods
        )
        if !force, let previous = lastWidgetSnapshot {
            let contentChanged = previous.proxyState != snapshot.proxyState
                || previous.eventsComplete != snapshot.eventsComplete
                || previous.periods != snapshot.periods
            let minimumInterval: TimeInterval = contentChanged ? 5 : 60
            guard snapshot.generatedAt.timeIntervalSince(previous.generatedAt) >= minimumInterval else {
                return
            }
        }
        do {
            try WidgetSnapshotStore.save(snapshot)
            lastWidgetSnapshot = snapshot
            if widgetSnapshotError != nil { widgetSnapshotError = nil }
            WidgetCenter.shared.reloadAllTimelines()
        } catch {
            if widgetSnapshotError != error.localizedDescription {
                widgetSnapshotError = error.localizedDescription
            }
        }
    }

    nonisolated private static func startDate(for period: WidgetPeriod) -> Date {
        switch period {
        case .hour:
            Date().addingTimeInterval(-3_600)
        case .today:
            Calendar.current.startOfDay(for: Date())
        case .sevenDays:
            Calendar.current.date(byAdding: .day, value: -7, to: Date()) ?? .distantPast
        }
    }

    private func validateConfiguration(_ configuration: TransitConfiguration) -> [ConfigurationIssue] {
        ConfigurationValidator.validate(configuration)
            + ConfigurationValidator.validateSecretReferences(configuration, using: secretStore)
    }

    private func refreshSecretReferences() {
        keychainSecretReferences = (try? secretStore.listReferences())?
            .filter { $0 != "install-hmac-key" } ?? []
    }
}

private final class AppRefreshRelay: @unchecked Sendable {
    enum Signal: Sendable {
        case dataPersisted
        case proxyStatus(ProxyStatus)
    }

    typealias Handler = @Sendable (Signal) -> Void

    private let lock = NSLock()
    private var handler: Handler?

    func setHandler(_ handler: @escaping Handler) {
        lock.withLock { self.handler = handler }
    }

    func requestDataRefresh() {
        send(.dataPersisted)
    }

    func publish(status: ProxyStatus) {
        send(.proxyStatus(status))
    }

    private func send(_ signal: Signal) {
        let currentHandler = lock.withLock { handler }
        currentHandler?(signal)
    }
}

private extension NSLock {
    func withLock<T>(_ body: () throws -> T) rethrows -> T {
        lock()
        defer { unlock() }
        return try body()
    }
}

private struct UnavailableEventWriter: UsageEventWriting {
    let message: String

    func save(_ event: UsageEvent) throws {
        throw UnavailableEventWriterError(message: message)
    }
}

private struct UnavailableEventWriterError: LocalizedError {
    let message: String
    var errorDescription: String? { message }
}
