import AsyncHTTPClient
import Foundation
import NIOCore
import NIOHTTP1
import NIOPosix

public final class ProxyService: @unchecked Sendable {
    public typealias StatusCallback = @Sendable (ProxyStatus) -> Void

    private let eventLoopGroup: MultiThreadedEventLoopGroup
    private let httpClient: HTTPClient
    private let eventPipeline: UsageEventPipeline
    private let flightRecorder: RequestFlightRecorder?
    private let secretStore: KeychainSecretStore
    private let routeRegistry = RouteRegistry()
    private let childChannels = ChildChannelRegistry()
    private let lock = NSLock()
    private var listeners: [Int: Channel] = [:]
    private var listenerStatuses: [Int: ListenerStatus] = [:]
    private var metrics = ProxyMetrics()
    private var shutdownComplete = false
    private let onStatus: StatusCallback?

    public init(
        eventPipeline: UsageEventPipeline,
        secretStore: KeychainSecretStore = KeychainSecretStore(),
        threadCount: Int? = nil,
        flightRecorder: RequestFlightRecorder? = nil,
        onStatus: StatusCallback? = nil
    ) {
        self.eventPipeline = eventPipeline
        self.secretStore = secretStore
        self.flightRecorder = flightRecorder
        self.onStatus = onStatus
        let resolvedThreadCount = threadCount ?? 2
        eventLoopGroup = MultiThreadedEventLoopGroup(numberOfThreads: max(1, resolvedThreadCount))
        var configuration = HTTPClient.Configuration()
        configuration.redirectConfiguration = .disallow
        configuration.timeout = .init(
            connect: .seconds(30),
            read: .seconds(600)
        )
        configuration.connectionPool.retryConnectionEstablishment = false
        httpClient = HTTPClient(eventLoopGroupProvider: .shared(eventLoopGroup), configuration: configuration)
    }

    public func apply(configuration: TransitConfiguration) async throws {
        let prepared = try await prepare(configuration: configuration)
        await commit(prepared)
    }

    @discardableResult
    public func startBestEffort(configuration: TransitConfiguration) async throws -> ProxyStatus {
        let errors = ConfigurationValidator.errors(in: configuration)
        guard errors.isEmpty else { throw ProxyServiceError.invalidConfiguration(errors) }
        guard !lock.withLock({ shutdownComplete }) else { throw ProxyServiceError.alreadyShutdown }

        let table = RouteTable(
            routes: configuration.routes,
            pricingPolicies: configuration.pricingPolicies
        )
        let desiredPorts = Set(table.ports)
        let currentPorts = Set(lock.withLock { listeners.keys })
        publishStarting(configuration.routes)

        var availablePorts = currentPorts.intersection(desiredPorts)
        var failedStatuses: [Int: ListenerStatus] = [:]
        for port in desiredPorts.subtracting(currentPorts).sorted() {
            do {
                let channel = try await bind(port: port)
                lock.withLock { listeners[port] = channel }
                availablePorts.insert(port)
            } catch {
                failedStatuses[port] = ListenerStatus(
                    port: port,
                    state: .failed,
                    error: error.localizedDescription
                )
            }
        }

        for port in currentPorts.subtracting(desiredPorts) {
            let channel = lock.withLock { listeners.removeValue(forKey: port) }
            try? await channel?.close()
        }

        let runnableRoutes = configuration.routes.filter {
            !$0.enabled || availablePorts.contains($0.listener.port)
        }
        routeRegistry.replace(RouteTable(
            routes: runnableRoutes,
            pricingPolicies: configuration.pricingPolicies
        ))
        lock.withLock {
            listenerStatuses = Dictionary(uniqueKeysWithValues: desiredPorts.map { port in
                if let failure = failedStatuses[port] {
                    return (port, failure)
                }
                return (port, ListenerStatus(port: port, state: .ready))
            })
            metrics.lastError = failedStatuses.values
                .sorted { $0.port < $1.port }
                .compactMap(\.error)
                .last
        }
        publishStatus()
        return status()
    }

    public func prepare(configuration: TransitConfiguration) async throws -> PreparedProxyConfiguration {
        let errors = ConfigurationValidator.errors(in: configuration)
        guard errors.isEmpty else { throw ProxyServiceError.invalidConfiguration(errors) }
        guard !lock.withLock({ shutdownComplete }) else { throw ProxyServiceError.alreadyShutdown }

        let previousStatuses = lock.withLock { listenerStatuses }
        publishStarting(configuration.routes)
        let newTable = RouteTable(
            routes: configuration.routes,
            pricingPolicies: configuration.pricingPolicies
        )
        let desiredPorts = Set(newTable.ports)
        let currentPorts = Set(lock.withLock { listeners.keys })
        let addedPorts = desiredPorts.subtracting(currentPorts).sorted()
        var staged: [Int: Channel] = [:]

        do {
            for port in addedPorts {
                staged[port] = try await bind(port: port)
            }
        } catch {
            for channel in staged.values { try? await channel.close() }
            let port = addedPorts.first { staged[$0] == nil } ?? 0
            lock.withLock {
                listenerStatuses = previousStatuses
                metrics.lastError = error.localizedDescription
            }
            publishStatus()
            throw ProxyServiceError.listenerBindingFailed(port: port, underlying: error)
        }

        return PreparedProxyConfiguration(
            table: newTable,
            desiredPorts: desiredPorts,
            currentPorts: currentPorts,
            stagedListeners: staged,
            previousStatuses: previousStatuses
        )
    }

    public func commit(_ prepared: PreparedProxyConfiguration) async {
        guard prepared.takeForCommit() else { return }
        routeRegistry.replace(prepared.table)
        lock.withLock {
            for (port, channel) in prepared.stagedListeners { listeners[port] = channel }
        }
        for port in prepared.currentPorts.subtracting(prepared.desiredPorts) {
            let channel = lock.withLock { listeners.removeValue(forKey: port) }
            try? await channel?.close()
        }
        lock.withLock {
            listenerStatuses = Dictionary(uniqueKeysWithValues: prepared.desiredPorts.map {
                ($0, ListenerStatus(port: $0, state: .ready))
            })
            metrics.lastError = nil
        }
        publishStatus()
    }

    public func discard(_ prepared: PreparedProxyConfiguration) async {
        guard prepared.takeForDiscard() else { return }
        for channel in prepared.stagedListeners.values { try? await channel.close() }
        lock.withLock { listenerStatuses = prepared.previousStatuses }
        publishStatus()
    }

    public func stop(gracePeriod: Duration = .seconds(10)) async {
        let channels = lock.withLock { () -> [Channel] in
            let values = Array(listeners.values)
            listeners.removeAll()
            listenerStatuses.removeAll()
            return values
        }
        for channel in channels { try? await channel.close() }
        let clock = ContinuousClock()
        let deadline = clock.now.advanced(by: gracePeriod)
        while lock.withLock({ metrics.activeRequests > 0 }), clock.now < deadline {
            try? await Task.sleep(for: .milliseconds(50))
        }
        await childChannels.closeAll()
        routeRegistry.replace(RouteTable(routes: []))
        publishStatus()
    }

    public func shutdown(gracePeriod: Duration = .seconds(10)) async throws {
        let shouldShutdown = lock.withLock { () -> Bool in
            guard !shutdownComplete else { return false }
            shutdownComplete = true
            return true
        }
        guard shouldShutdown else { return }
        await stop(gracePeriod: gracePeriod)
        try await httpClient.shutdown().get()
        try await eventLoopGroup.shutdownGracefully()
    }

    public func status() -> ProxyStatus {
        lock.withLock { makeStatus() }
    }

    private func bind(port: Int) async throws -> Channel {
        try await ServerBootstrap(group: eventLoopGroup)
            .serverChannelOption(ChannelOptions.backlog, value: 256)
            .serverChannelOption(ChannelOptions.socketOption(.so_reuseaddr), value: 1)
            .childChannelOption(ChannelOptions.socketOption(.so_reuseaddr), value: 1)
            .childChannelOption(ChannelOptions.maxMessagesPerRead, value: 16)
            .childChannelOption(ChannelOptions.allowRemoteHalfClosure, value: false)
            .childChannelInitializer { [weak self] channel in
                guard let self else { return channel.eventLoop.makeFailedFuture(ProxyServiceError.alreadyShutdown) }
                self.childChannels.add(channel)
                return channel.pipeline.configureHTTPServerPipeline(withErrorHandling: true).flatMap {
                    channel.pipeline.addHandler(ProxyChannelHandler(
                        listenerPort: port,
                        routeRegistry: self.routeRegistry,
                        httpClient: self.httpClient,
                        secretStore: self.secretStore,
                        eventPipeline: self.eventPipeline,
                        flightRecorder: self.flightRecorder,
                        childChannels: self.childChannels,
                        metrics: self
                    ))
                }
            }
            .bind(host: "127.0.0.1", port: port)
            .get()
    }

    private func publishStarting(_ routes: [RouteConfiguration]) {
        let ports = Set(routes.filter(\.enabled).map(\.listener.port))
        lock.withLock {
            listenerStatuses = Dictionary(uniqueKeysWithValues: ports.map {
                ($0, ListenerStatus(port: $0, state: .binding))
            })
        }
        publishStatus(stateOverride: .starting)
    }

    private func publishStatus(stateOverride: ProxyState? = nil) {
        let status = lock.withLock { makeStatus(stateOverride: stateOverride) }
        onStatus?(status)
    }

    private func makeStatus(stateOverride: ProxyState? = nil) -> ProxyStatus {
        let state: ProxyState
        if let stateOverride {
            state = stateOverride
        } else if listenerStatuses.values.contains(where: { $0.state == .failed }) || metrics.lastError != nil {
            state = .degraded
        } else if listeners.isEmpty {
            state = .stopped
        } else {
            state = .running
        }
        return ProxyStatus(
            state: state,
            listeners: listenerStatuses.values.sorted { $0.port < $1.port },
            activeConnections: childChannels.count,
            activeRequests: metrics.activeRequests,
            forwardedRequests: metrics.forwardedRequests,
            parseMissing: metrics.parseMissing,
            parseErrors: metrics.parseErrors,
            lastError: metrics.lastError
        )
    }
}

public final class PreparedProxyConfiguration: @unchecked Sendable {
    fileprivate let table: RouteTable
    fileprivate let desiredPorts: Set<Int>
    fileprivate let currentPorts: Set<Int>
    fileprivate let stagedListeners: [Int: Channel]
    fileprivate let previousStatuses: [Int: ListenerStatus]
    private let lock = NSLock()
    private var consumed = false

    fileprivate init(
        table: RouteTable,
        desiredPorts: Set<Int>,
        currentPorts: Set<Int>,
        stagedListeners: [Int: Channel],
        previousStatuses: [Int: ListenerStatus]
    ) {
        self.table = table
        self.desiredPorts = desiredPorts
        self.currentPorts = currentPorts
        self.stagedListeners = stagedListeners
        self.previousStatuses = previousStatuses
    }

    fileprivate func takeForCommit() -> Bool { take() }
    fileprivate func takeForDiscard() -> Bool { take() }

    private func take() -> Bool {
        lock.lock()
        defer { lock.unlock() }
        guard !consumed else { return false }
        consumed = true
        return true
    }
}

extension ProxyService: ProxyChannelMetrics {
    func requestStarted() {
        lock.withLock { metrics.activeRequests += 1 }
        publishStatus()
    }

    func requestFinished(observation: UsageObservation, error: String?) {
        lock.withLock {
            metrics.activeRequests = max(0, metrics.activeRequests - 1)
            metrics.forwardedRequests += 1
            if observation.quality == .missing { metrics.parseMissing += 1 }
            if observation.parserErrorCode != nil { metrics.parseErrors += 1 }
            metrics.lastError = error
        }
        publishStatus()
    }
}

private struct ProxyMetrics {
    var activeRequests = 0
    var forwardedRequests: Int64 = 0
    var parseMissing: Int64 = 0
    var parseErrors: Int64 = 0
    var lastError: String?
}

final class RouteRegistry: @unchecked Sendable {
    private let lock = NSLock()
    private var table = RouteTable(routes: [])

    func replace(_ table: RouteTable) {
        lock.withLock { self.table = table }
    }

    func match(port: Int, requestURI: String) -> RouteMatch? {
        lock.withLock { table.match(port: port, requestURI: requestURI) }
    }
}

final class ChildChannelRegistry: @unchecked Sendable {
    private let lock = NSLock()
    private var channels: [ObjectIdentifier: Channel] = [:]

    func add(_ channel: Channel) {
        lock.withLock { channels[ObjectIdentifier(channel)] = channel }
    }

    func remove(_ channel: Channel) {
        _ = lock.withLock { channels.removeValue(forKey: ObjectIdentifier(channel)) }
    }

    var count: Int {
        lock.withLock { channels.count }
    }

    func closeAll() async {
        let current = lock.withLock { () -> [Channel] in
            let values = Array(channels.values)
            channels.removeAll()
            return values
        }
        for channel in current { try? await channel.close() }
    }
}

protocol ProxyChannelMetrics: AnyObject, Sendable {
    func requestStarted()
    func requestFinished(observation: UsageObservation, error: String?)
}

private extension NSLock {
    func withLock<T>(_ body: () throws -> T) rethrows -> T {
        lock()
        defer { unlock() }
        return try body()
    }
}
