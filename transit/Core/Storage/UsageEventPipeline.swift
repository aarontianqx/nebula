import Foundation

public struct EventPipelineStatus: Equatable, Sendable {
    public let queued: Int
    public let persisted: Int64
    public let dropped: Int64
    public let lastError: String?

    public init(queued: Int, persisted: Int64, dropped: Int64, lastError: String?) {
        self.queued = queued
        self.persisted = persisted
        self.dropped = dropped
        self.lastError = lastError
    }
}

public final class UsageEventPipeline: @unchecked Sendable {
    public typealias PersistCallback = @Sendable (UsageEvent) -> Void

    private let store: any UsageEventWriting
    private let capacity: Int
    private let worker = DispatchQueue(label: "com.aarontianqx.transit.event-writer", qos: .utility)
    private let lock = NSLock()
    private var buffer: [UsageEvent?]
    private var head = 0
    private var tail = 0
    private var count = 0
    private var drainScheduled = false
    private var accepting = true
    private var persisted: Int64 = 0
    private var dropped: Int64 = 0
    private var lastError: String?
    private let onPersist: PersistCallback?

    public init(store: any UsageEventWriting, capacity: Int = 1_024, onPersist: PersistCallback? = nil) {
        self.store = store
        self.capacity = max(1, min(capacity, 65_536))
        self.onPersist = onPersist
        buffer = Array(repeating: nil, count: self.capacity)
    }

    @discardableResult
    public func submit(_ event: UsageEvent) -> Bool {
        lock.lock()
        guard accepting, count < capacity else {
            dropped += 1
            lock.unlock()
            return false
        }
        buffer[tail] = event
        tail = (tail + 1) % capacity
        count += 1
        let shouldSchedule = !drainScheduled
        if shouldSchedule { drainScheduled = true }
        lock.unlock()

        if shouldSchedule {
            worker.async { [weak self] in self?.drain() }
        }
        return true
    }

    public func status() -> EventPipelineStatus {
        lock.withLock {
            EventPipelineStatus(
                queued: count,
                persisted: persisted,
                dropped: dropped,
                lastError: lastError
            )
        }
    }

    public func flush() {
        worker.sync { drain() }
    }

    public func shutdown() {
        lock.withLock { accepting = false }
        flush()
    }

    private func drain() {
        while true {
            let event: UsageEvent? = lock.withLock {
                guard count > 0 else {
                    drainScheduled = false
                    return nil
                }
                let event = buffer[head]
                buffer[head] = nil
                head = (head + 1) % capacity
                count -= 1
                return event
            }
            guard let event else { return }
            do {
                try store.save(event)
                lock.withLock {
                    persisted += 1
                    lastError = nil
                }
                onPersist?(event)
            } catch {
                lock.withLock {
                    dropped += 1
                    lastError = error.localizedDescription
                }
            }
        }
    }
}

private extension NSLock {
    func withLock<T>(_ body: () throws -> T) rethrows -> T {
        lock()
        defer { unlock() }
        return try body()
    }
}
