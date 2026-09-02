import Foundation
import NIOCore

public enum ResponseActivityKind: Equatable, Sendable {
    case body
    case meaningfulSSE
    case heartbeat
    case sseTransport

    var phase: RequestDiagnosticPhase {
        switch self {
        case .body: .upstreamBodyActivity
        case .meaningfulSSE: .meaningfulSSEActivity
        case .heartbeat: .heartbeatActivity
        case .sseTransport: .upstreamBodyActivity
        }
    }

    var isMeaningful: Bool {
        self == .body || self == .meaningfulSSE
    }

    var diagnosticActivity: ResponseDiagnosticActivity {
        switch self {
        case .body: .body
        case .meaningfulSSE: .meaningfulSSE
        case .heartbeat: .heartbeat
        case .sseTransport: .sseTransport
        }
    }
}

public final class RequestFlightRecorder: @unchecked Sendable {
    public typealias ChangeCallback = @Sendable () -> Void

    private let store: RequestDiagnosticsStore
    private let stallThreshold: TimeInterval
    private let persistenceQueue = DispatchQueue(label: "com.aarontianqx.transit.diagnostics", qos: .utility)
    private let lock = NSLock()
    private var active: [String: RequestDiagnosticSnapshot] = [:]
    private var lastPersistedAt: [String: Date] = [:]
    private var timer: DispatchSourceTimer?
    private var closed = false
    private var lastStorageError: String?
    private let onChange: ChangeCallback?

    public init(
        store: RequestDiagnosticsStore,
        stallThreshold: TimeInterval = 60,
        onChange: ChangeCallback? = nil
    ) {
        self.store = store
        self.stallThreshold = stallThreshold
        self.onChange = onChange
        let interval = max(1, min(10, stallThreshold / 2))
        let timer = DispatchSource.makeTimerSource(queue: persistenceQueue)
        timer.schedule(deadline: .now() + interval, repeating: interval)
        timer.setEventHandler { [weak self] in self?.detectStalls() }
        self.timer = timer
        timer.resume()
        persistenceQueue.async { [weak self] in self?.performMaintenance() }
    }

    @discardableResult
    public func start(routeID: String, protocolType: UsageProtocol, method: String) -> String {
        let now = Date()
        let snapshot = RequestDiagnosticSnapshot(
            id: UUID().uuidString.lowercased(),
            routeID: routeID,
            protocolType: protocolType,
            method: method,
            phase: .clientRequestStarted,
            state: .active,
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
        lock.withLock { active[snapshot.id] = snapshot }
        persist(snapshot)
        notifyChange()
        return snapshot.id
    }

    public func clientBodyCompleted(id: String, requestBytes: Int64) {
        update(id: id, phase: .clientRequestBodyCompleted, meaningful: true, forcePersist: true) {
            $0.requestBytes = requestBytes
        }
    }

    public func clientBodyActivity(id: String, requestBytes: Int64) {
        update(id: id, phase: .clientRequestBodyActivity, meaningful: true, forcePersist: false) {
            $0.requestBytes = requestBytes
        }
    }

    public func upstreamStarted(id: String) {
        update(id: id, phase: .upstreamRequestStarted, meaningful: true, forcePersist: true)
    }

    public func responseHead(id: String, statusCode: Int) {
        update(id: id, phase: .upstreamResponseHeadReceived, meaningful: true, forcePersist: true) {
            $0.statusCode = statusCode
            $0.lastUpstreamActivityAt = $0.updatedAt
        }
    }

    public func upstreamActivity(id: String, kind: ResponseActivityKind, responseBytes: Int64) {
        update(id: id, phase: kind.phase, meaningful: kind.isMeaningful, forcePersist: false) {
            $0.responseBytes = responseBytes
            $0.lastUpstreamActivityAt = $0.updatedAt
            $0.lastResponseActivity = kind.diagnosticActivity
        }
    }

    public func downstreamWriteCompleted(id: String, meaningful: Bool) {
        update(id: id, phase: .downstreamWriteCompleted, meaningful: meaningful, forcePersist: false) {
            $0.lastDownstreamWriteAt = $0.updatedAt
        }
    }

    public func upstreamCompleted(id: String) {
        update(id: id, phase: .upstreamCompleted, meaningful: true, forcePersist: true)
    }

    public func finish(id: String, state: RequestDiagnosticState, errorCode: String?) {
        let snapshot = lock.withLock { () -> RequestDiagnosticSnapshot? in
            guard var snapshot = active.removeValue(forKey: id) else { return nil }
            snapshot.state = state
            snapshot.errorCode = errorCode
            snapshot.updatedAt = Date()
            lastPersistedAt.removeValue(forKey: id)
            return snapshot
        }
        guard let snapshot else { return }
        persistenceQueue.async { [weak self, store, onChange] in
            do {
                if state == .completed {
                    try store.delete(id: id)
                } else {
                    try store.upsert(snapshot)
                    try store.prune(olderThan: Date().addingTimeInterval(-7 * 86_400), retaining: 200)
                }
                self?.setStorageError(nil)
            } catch {
                self?.setStorageError(error.localizedDescription)
            }
            onChange?()
        }
    }

    public func snapshots(limit: Int = 200) -> [RequestDiagnosticSnapshot] {
        let activeSnapshots = lock.withLock { Array(active.values) }
        let persisted = (try? store.recent(limit: limit)) ?? []
        var merged = Dictionary(uniqueKeysWithValues: persisted.map { ($0.id, $0) })
        for snapshot in activeSnapshots { merged[snapshot.id] = snapshot }
        return merged.values.sorted { $0.updatedAt > $1.updatedAt }.prefix(limit).map { $0 }
    }

    public func runMaintenance() {
        persistenceQueue.async { [weak self] in self?.performMaintenance() }
    }

    public func storageError() -> String? {
        lock.withLock { lastStorageError }
    }

    public func close() {
        let shouldClose = lock.withLock { () -> Bool in
            guard !closed else { return false }
            closed = true
            return true
        }
        guard shouldClose else { return }
        timer?.cancel()
        timer = nil
        persistenceQueue.sync {
            performMaintenance()
            try? store.close()
        }
    }

    private func update(
        id: String,
        phase: RequestDiagnosticPhase,
        meaningful: Bool,
        forcePersist: Bool,
        mutate: (inout RequestDiagnosticSnapshot) -> Void = { _ in }
    ) {
        let result = lock.withLock { () -> (RequestDiagnosticSnapshot, Bool)? in
            guard var snapshot = active[id] else { return nil }
            let now = Date()
            snapshot.phase = phase
            if meaningful { snapshot.state = .active }
            snapshot.updatedAt = now
            if meaningful { snapshot.lastMeaningfulActivityAt = now }
            mutate(&snapshot)
            active[id] = snapshot
            let last = lastPersistedAt[id] ?? .distantPast
            let shouldPersist = forcePersist || now.timeIntervalSince(last) >= 5
            if shouldPersist { lastPersistedAt[id] = now }
            return (snapshot, shouldPersist)
        }
        guard let (snapshot, shouldPersist) = result else { return }
        if shouldPersist {
            persist(snapshot)
            notifyChange()
        }
    }

    private func persist(_ snapshot: RequestDiagnosticSnapshot) {
        persistenceQueue.async { [weak self, store, onChange] in
            do {
                try store.upsert(snapshot)
                self?.setStorageError(nil)
            } catch {
                self?.setStorageError(error.localizedDescription)
            }
            onChange?()
        }
    }

    func detectStalls(referenceDate now: Date = Date()) {
        let stalled = lock.withLock { () -> [RequestDiagnosticSnapshot] in
            var changed: [RequestDiagnosticSnapshot] = []
            for (id, var snapshot) in active
            where snapshot.state == .active
                && now.timeIntervalSince(snapshot.lastMeaningfulActivityAt) >= stallThreshold {
                snapshot.state = .stalled
                snapshot.updatedAt = now
                active[id] = snapshot
                lastPersistedAt[id] = now
                changed.append(snapshot)
            }
            return changed
        }
        for snapshot in stalled {
            do {
                try store.upsert(snapshot)
                setStorageError(nil)
            } catch {
                setStorageError(error.localizedDescription)
            }
        }
        if !stalled.isEmpty { notifyChange() }
    }

    private func performMaintenance() {
        do {
            _ = try store.prune(olderThan: Date().addingTimeInterval(-7 * 86_400), retaining: 200)
            try store.checkpoint()
            setStorageError(nil)
        } catch {
            setStorageError(error.localizedDescription)
            notifyChange()
        }
    }

    func flush() {
        persistenceQueue.sync {}
    }

    private func notifyChange() {
        onChange?()
    }

    private func setStorageError(_ value: String?) {
        lock.withLock { lastStorageError = value }
    }
}

struct SSEActivityClassifier: Sendable {
    private var pending = ""

    mutating func classify(contentType: String?, bytes: ByteBuffer) -> ResponseActivityKind {
        guard contentType?.lowercased().contains("text/event-stream") == true else { return .body }
        var copy = bytes
        let text = copy.readString(length: copy.readableBytes) ?? ""
        pending.append(text)
        if pending.utf8.count > 8_192 {
            pending = String(pending.suffix(4_096))
        }
        let normalized = pending.replacingOccurrences(of: "\r\n", with: "\n")
        let lines = normalized.split(separator: "\n", omittingEmptySubsequences: false)
        pending = normalized.hasSuffix("\n") ? "" : String(lines.last ?? "")
        let completed = normalized.hasSuffix("\n") ? lines : lines.dropLast()
        var sawHeartbeat = false
        for line in completed {
            if line.hasPrefix(":") {
                sawHeartbeat = true
            } else if line.hasPrefix("data:")
                && !String(line.dropFirst(5)).trimmingCharacters(in: .whitespaces).isEmpty {
                return .meaningfulSSE
            }
        }
        return sawHeartbeat ? .heartbeat : .sseTransport
    }
}

private extension NSLock {
    func withLock<T>(_ body: () throws -> T) rethrows -> T {
        lock()
        defer { unlock() }
        return try body()
    }
}
