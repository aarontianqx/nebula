import AsyncHTTPClient
import Foundation
import NIOCore
import NIOHTTP1

private typealias RequestBodySequence = NIOAsyncSequenceProducer<
    ByteBuffer,
    NIOAsyncSequenceProducerBackPressureStrategies.HighLowWatermark,
    RequestBodyDelegate
>

final class ProxyChannelHandler: ChannelInboundHandler, @unchecked Sendable {
    typealias InboundIn = HTTPServerRequestPart
    typealias OutboundOut = HTTPServerResponsePart

    private let listenerPort: Int
    private let routeRegistry: RouteRegistry
    private let httpClient: HTTPClient
    private let secretStore: KeychainSecretStore
    private let eventPipeline: UsageEventPipeline
    private let childChannels: ChildChannelRegistry
    private weak var metrics: (any ProxyChannelMetrics)?
    private var current: RequestContext?
    private var executionTask: Task<Void, Never>?
    private var discardingRequest = false

    init(
        listenerPort: Int,
        routeRegistry: RouteRegistry,
        httpClient: HTTPClient,
        secretStore: KeychainSecretStore,
        eventPipeline: UsageEventPipeline,
        childChannels: ChildChannelRegistry,
        metrics: any ProxyChannelMetrics
    ) {
        self.listenerPort = listenerPort
        self.routeRegistry = routeRegistry
        self.httpClient = httpClient
        self.secretStore = secretStore
        self.eventPipeline = eventPipeline
        self.childChannels = childChannels
        self.metrics = metrics
    }

    func channelRead(context: ChannelHandlerContext, data: NIOAny) {
        switch unwrapInboundIn(data) {
        case .head(let head):
            handleHead(head, context: context)
        case .body(let body):
            handleBody(body, context: context)
        case .end:
            handleEnd()
        }
    }

    func channelInactive(context: ChannelHandlerContext) {
        current?.bodySource.finish()
        current?.markRequestEnded()
        current?.cancelUpstream()
        executionTask?.cancel()
        childChannels.remove(context.channel)
        context.fireChannelInactive()
    }

    func errorCaught(context: ChannelHandlerContext, error: Error) {
        current?.bodySource.finish()
        current?.markRequestEnded()
        current?.cancelUpstream()
        executionTask?.cancel()
        context.close(promise: nil)
    }

    private func handleHead(_ head: HTTPRequestHead, context: ChannelHandlerContext) {
        guard current == nil, executionTask == nil else {
            discardingRequest = true
            writeError(status: .tooManyRequests, code: "http_pipelining_not_supported", context: context)
            return
        }
        guard let match = routeRegistry.match(port: listenerPort, requestURI: head.uri) else {
            discardingRequest = true
            writeError(status: .notFound, code: "route_not_found", context: context)
            return
        }

        let delegate = RequestBodyDelegate(channel: context.channel)
        let stream = RequestBodySequence.makeSequence(
            backPressureStrategy: .init(lowWatermark: 2, highWatermark: 8),
            finishOnDeinit: false,
            delegate: delegate
        )
        let observer = UsageObserverFactory.make(protocolType: match.route.protocolType)
        let requestContext = RequestContext(
            head: head,
            match: match,
            observer: observer,
            bodySource: stream.source,
            bodySequence: stream.sequence
        )
        current = requestContext
        metrics?.requestStarted()
        let clientChannel = context.channel
        clientChannel.closeFuture.whenComplete { [weak requestContext] _ in
            requestContext?.bodySource.finish()
            requestContext?.markRequestEnded()
            requestContext?.cancelUpstream()
        }
        executionTask = Task { [weak self, weak requestContext] in
            guard let self, let requestContext else { return }
            await self.execute(requestContext, clientChannel: clientChannel)
        }
    }

    private func handleBody(_ body: ByteBuffer, context: ChannelHandlerContext) {
        guard !discardingRequest, let current else { return }
        current.addRequestBytes(Int64(body.readableBytes))
        current.observeRequest(
            contentType: current.head.headers.first(name: "content-type"),
            bytes: body
        )
        switch current.bodySource.yield(body) {
        case .stopProducing:
            let channel = context.channel
            context.channel.setOption(ChannelOptions.autoRead, value: false).whenFailure { _ in
                channel.close(promise: nil)
            }
        case .dropped:
            context.close(promise: nil)
        case .produceMore:
            break
        }
    }

    private func handleEnd() {
        if discardingRequest {
            discardingRequest = false
            return
        }
        current?.bodySource.finish()
        current?.markRequestEnded()
    }

    private func execute(_ requestContext: RequestContext, clientChannel: Channel) async {
        let delegate = ProxyResponseDelegate(
            requestContext: requestContext,
            clientChannel: clientChannel
        )
        do {
            try Task.checkCancellation()
            let built = try buildRequest(requestContext, clientEventLoop: clientChannel.eventLoop)
            requestContext.setKeyFingerprint(built.fingerprint)
            let upstreamTask = httpClient.execute(
                request: built.request,
                delegate: delegate
            )
            requestContext.setUpstreamTask(upstreamTask)
            try Task.checkCancellation()
            try await upstreamTask.get()
            try await clientChannel.writeAndFlush(HTTPServerResponsePart.end(nil)).get()
            await requestContext.waitForRequestEnd()
            let outcome: RequestOutcome = (delegate.statusCode ?? 0) >= 400 ? .failed : .completed
            finish(requestContext, outcome: outcome, errorCode: outcome == .failed ? "upstream_http_status" : nil)
        } catch is CancellationError {
            requestContext.cancelUpstream()
            finish(requestContext, outcome: .cancelled, errorCode: "client_cancelled")
        } catch {
            if Task.isCancelled || isCancellation(error) {
                requestContext.cancelUpstream()
                finish(requestContext, outcome: .cancelled, errorCode: "client_cancelled")
                await clearRequest(requestContext, on: clientChannel)
                return
            }
            if !delegate.sentResponseHead, clientChannel.isActive {
                try? await writeErrorAsync(
                    status: .badGateway,
                    code: "upstream_request_failed",
                    channel: clientChannel
                )
            } else {
                try? await clientChannel.close()
            }
            finish(requestContext, outcome: .failed, errorCode: "upstream_request_failed", error: error)
        }

        await clearRequest(requestContext, on: clientChannel)
    }

    private func isCancellation(_ error: Error) -> Bool {
        guard let clientError = error as? HTTPClientError else { return false }
        return clientError == .cancelled
    }

    private func clearRequest(_ requestContext: RequestContext, on clientChannel: Channel) async {
        try? await clientChannel.eventLoop.submit { [weak self] in
            guard let self, self.current === requestContext else { return }
            self.current = nil
            self.executionTask = nil
        }.get()
    }

    private func buildRequest(
        _ context: RequestContext,
        clientEventLoop: EventLoop
    ) throws -> (request: HTTPClient.Request, fingerprint: String?) {
        var headers = Self.filteredRequestHeaders(context.head.headers)
        headers.replaceOrAdd(name: "accept-encoding", value: "identity")

        let fingerprint: String?
        switch context.match.route.authentication.mode {
        case .passthrough:
            let credential = context.head.headers.first(name: "authorization")
                ?? context.head.headers.first(name: "x-api-key")
            fingerprint = credential.flatMap { try? secretStore.fingerprint(secret: $0) }
        case .replaceBearer:
            let secret = try secretStore.read(reference: context.match.route.authentication.secretRef ?? "")
            headers.remove(name: "authorization")
            headers.remove(name: "x-api-key")
            headers.add(name: "authorization", value: "Bearer \(secret)")
            fingerprint = try? secretStore.fingerprint(secret: secret)
        case .replaceHeader:
            let secret = try secretStore.read(reference: context.match.route.authentication.secretRef ?? "")
            let name = context.match.route.authentication.headerName ?? "x-api-key"
            headers.remove(name: "authorization")
            headers.remove(name: "x-api-key")
            headers.remove(name: name)
            headers.add(name: name, value: secret)
            fingerprint = try? secretStore.fingerprint(secret: secret)
        }

        let contentLength = context.head.headers.first(name: "content-length").flatMap(Int64.init)
        let hasBody = contentLength != nil
            || context.head.headers.contains(name: "transfer-encoding")
            || [.POST, .PUT, .PATCH].contains(context.head.method)
        let body: HTTPClient.Body? = hasBody
            ? .stream(contentLength: contentLength) { writer in
                let completion = clientEventLoop.makePromise(of: Void.self)
                Task {
                    do {
                        for await chunk in context.bodySequence {
                            try await writer.write(.byteBuffer(chunk)).get()
                        }
                        completion.succeed(())
                    } catch {
                        completion.fail(error)
                    }
                }
                return completion.futureResult
            }
            : nil
        let request = try HTTPClient.Request(
            url: context.match.upstreamURL.absoluteString,
            method: context.head.method,
            headers: headers,
            body: body
        )
        return (request, fingerprint)
    }

    private func finish(
        _ context: RequestContext,
        outcome: RequestOutcome,
        errorCode: String?,
        error: Error? = nil
    ) {
        guard context.markFinished() else { return }
        let snapshot = context.finishSnapshot(outcome: outcome)
        let observation = snapshot.observation
        let cost = PricingEngine.estimate(
            usage: observation.usage,
            model: observation.model,
            policy: context.match.pricingPolicy
        )
        let completedAt = Date()
        let latency = Int64((DispatchTime.now().uptimeNanoseconds - context.startedUptime) / 1_000_000)
        let event = UsageEvent(
            startedAt: context.startedAt,
            completedAt: completedAt,
            routeID: context.match.route.id,
            agentID: context.match.route.agentID,
            protocolType: context.match.route.protocolType,
            method: context.head.method.rawValue,
            endpointKind: observation.endpointKind,
            statusCode: snapshot.statusCode,
            outcome: outcome,
            latencyMilliseconds: latency,
            model: observation.model,
            keyFingerprint: snapshot.keyFingerprint,
            usage: observation.usage,
            usageQuality: observation.quality,
            usageRaw: observation.rawUsageJSON,
            requestBytes: snapshot.requestBytes,
            responseBytes: snapshot.responseBytes,
            estimatedCost: cost.map { NSDecimalNumber(decimal: $0.amount).stringValue },
            currency: cost?.currency,
            pricingPolicyID: cost?.policyID,
            pricingPolicyVersion: cost?.policyVersion,
            errorCode: errorCode ?? observation.parserErrorCode
        )
        eventPipeline.submit(event)
        metrics?.requestFinished(observation: observation, error: error?.localizedDescription)
    }

    private func writeError(status: HTTPResponseStatus, code: String, context: ChannelHandlerContext) {
        let body = ByteBuffer(string: #"{"error":{"code":"\#(code)"}}"#)
        var headers = HTTPHeaders()
        headers.add(name: "content-type", value: "application/json")
        headers.add(name: "content-length", value: "\(body.readableBytes)")
        headers.add(name: "connection", value: "close")
        context.write(wrapOutboundOut(.head(.init(version: .http1_1, status: status, headers: headers))), promise: nil)
        context.write(wrapOutboundOut(.body(.byteBuffer(body))), promise: nil)
        let channel = context.channel
        context.writeAndFlush(wrapOutboundOut(.end(nil))).whenComplete { _ in channel.close(promise: nil) }
    }

    private func writeErrorAsync(status: HTTPResponseStatus, code: String, channel: Channel) async throws {
        let body = ByteBuffer(string: #"{"error":{"code":"\#(code)"}}"#)
        var headers = HTTPHeaders()
        headers.add(name: "content-type", value: "application/json")
        headers.add(name: "content-length", value: "\(body.readableBytes)")
        try await channel.writeAndFlush(HTTPServerResponsePart.head(.init(
            version: .http1_1,
            status: status,
            headers: headers
        ))).get()
        try await channel.writeAndFlush(HTTPServerResponsePart.body(.byteBuffer(body))).get()
        try await channel.writeAndFlush(HTTPServerResponsePart.end(nil)).get()
    }

    fileprivate static func filteredRequestHeaders(_ input: HTTPHeaders) -> HTTPHeaders {
        filterHeaders(input, alsoRemoving: ["host", "content-length", "accept-encoding"])
    }

    fileprivate static func filteredResponseHeaders(_ input: HTTPHeaders) -> HTTPHeaders {
        filterHeaders(input, alsoRemoving: [])
    }

    private static func filterHeaders(_ input: HTTPHeaders, alsoRemoving: Set<String>) -> HTTPHeaders {
        var connectionNamed: Set<String> = []
        for value in input[canonicalForm: "connection"] {
            connectionNamed.formUnion(value.split(separator: ",").map {
                $0.trimmingCharacters(in: .whitespacesAndNewlines).lowercased()
            })
        }
        let hopByHop: Set<String> = [
            "connection", "keep-alive", "proxy-authenticate", "proxy-authorization",
            "te", "trailer", "transfer-encoding", "upgrade",
        ]
        let removed = hopByHop.union(connectionNamed).union(alsoRemoving)
        var result = HTTPHeaders()
        for (name, value) in input where !removed.contains(name.lowercased()) {
            result.add(name: name, value: value)
        }
        return result
    }
}

private final class ProxyResponseDelegate: HTTPClientResponseDelegate, @unchecked Sendable {
    typealias Response = Void

    private let requestContext: RequestContext
    private let clientChannel: Channel
    private let lock = NSLock()
    private var responseContentType: String?
    private var storedStatusCode: Int?
    private var storedSentResponseHead = false

    init(requestContext: RequestContext, clientChannel: Channel) {
        self.requestContext = requestContext
        self.clientChannel = clientChannel
    }

    var statusCode: Int? {
        lock.withLock { storedStatusCode }
    }

    var sentResponseHead: Bool {
        lock.withLock { storedSentResponseHead }
    }

    func didReceiveHead(
        task: HTTPClient.Task<Void>,
        _ head: HTTPResponseHead
    ) -> EventLoopFuture<Void> {
        lock.withLock {
            storedStatusCode = Int(head.status.code)
            storedSentResponseHead = true
            responseContentType = head.headers.first(name: "content-type")
        }
        requestContext.setStatusCode(Int(head.status.code))
        let responseHead = HTTPResponseHead(
            version: .http1_1,
            status: head.status,
            headers: ProxyChannelHandler.filteredResponseHeaders(head.headers)
        )
        return clientChannel.writeAndFlush(HTTPServerResponsePart.head(responseHead))
    }

    func didReceiveBodyPart(
        task: HTTPClient.Task<Void>,
        _ buffer: ByteBuffer
    ) -> EventLoopFuture<Void> {
        requestContext.addResponseBytes(Int64(buffer.readableBytes))
        requestContext.observeResponse(
            contentType: lock.withLock { responseContentType },
            bytes: buffer
        )
        return clientChannel.writeAndFlush(HTTPServerResponsePart.body(.byteBuffer(buffer)))
    }

    func didFinishRequest(task: HTTPClient.Task<Void>) throws {}
}

private final class RequestContext: @unchecked Sendable {
    let head: HTTPRequestHead
    let match: RouteMatch
    let observer: UsageProtocolObserver
    let bodySource: RequestBodySequence.Source
    let bodySequence: RequestBodySequence
    let startedAt = Date()
    let startedUptime = DispatchTime.now().uptimeNanoseconds
    private let stateLock = NSLock()
    private var finished = false
    private var requestBytes: Int64 = 0
    private var responseBytes: Int64 = 0
    private var statusCode: Int?
    private var keyFingerprint: String?
    private var requestDidEnd = false
    private var requestEndWaiters: [CheckedContinuation<Void, Never>] = []
    private var upstreamTask: HTTPClient.Task<Void>?
    private var upstreamCancellationRequested = false

    init(
        head: HTTPRequestHead,
        match: RouteMatch,
        observer: UsageProtocolObserver,
        bodySource: RequestBodySequence.Source,
        bodySequence: RequestBodySequence
    ) {
        self.head = head
        self.match = match
        self.observer = observer
        self.bodySource = bodySource
        self.bodySequence = bodySequence
    }

    func markFinished() -> Bool {
        stateLock.lock()
        defer { stateLock.unlock() }
        guard !finished else { return false }
        finished = true
        return true
    }

    func markRequestEnded() {
        stateLock.lock()
        guard !requestDidEnd else {
            stateLock.unlock()
            return
        }
        requestDidEnd = true
        let waiters = requestEndWaiters
        requestEndWaiters.removeAll()
        stateLock.unlock()
        for waiter in waiters { waiter.resume() }
    }

    func waitForRequestEnd() async {
        await withCheckedContinuation { continuation in
            stateLock.lock()
            if requestDidEnd {
                stateLock.unlock()
                continuation.resume()
            } else {
                requestEndWaiters.append(continuation)
                stateLock.unlock()
            }
        }
    }

    func addRequestBytes(_ count: Int64) {
        stateLock.lock()
        requestBytes += count
        stateLock.unlock()
    }

    func addResponseBytes(_ count: Int64) {
        stateLock.lock()
        responseBytes += count
        stateLock.unlock()
    }

    func setStatusCode(_ value: Int?) {
        stateLock.lock()
        statusCode = value
        stateLock.unlock()
    }

    func setKeyFingerprint(_ value: String?) {
        stateLock.lock()
        keyFingerprint = value
        stateLock.unlock()
    }

    func setUpstreamTask(_ task: HTTPClient.Task<Void>) {
        stateLock.lock()
        upstreamTask = task
        let shouldCancel = upstreamCancellationRequested
        stateLock.unlock()
        if shouldCancel { task.cancel() }
    }

    func cancelUpstream() {
        stateLock.lock()
        upstreamCancellationRequested = true
        let task = upstreamTask
        stateLock.unlock()
        task?.cancel()
    }

    func observeRequest(contentType: String?, bytes: ByteBuffer) {
        stateLock.lock()
        observer.observeRequest(contentType: contentType, bytes: bytes)
        stateLock.unlock()
    }

    func observeResponse(contentType: String?, bytes: ByteBuffer) {
        stateLock.lock()
        observer.observeResponse(contentType: contentType, bytes: bytes)
        stateLock.unlock()
    }

    func finishSnapshot(outcome: RequestOutcome) -> (
        observation: UsageObservation,
        requestBytes: Int64,
        responseBytes: Int64,
        statusCode: Int?,
        keyFingerprint: String?
    ) {
        stateLock.lock()
        defer { stateLock.unlock() }
        return (
            observer.finish(outcome: outcome),
            requestBytes,
            responseBytes,
            statusCode,
            keyFingerprint
        )
    }
}

private final class RequestBodyDelegate: NIOAsyncSequenceProducerDelegate, @unchecked Sendable {
    private weak var channel: Channel?

    init(channel: Channel) {
        self.channel = channel
    }

    func produceMore() {
        guard let channel else { return }
        channel.eventLoop.execute {
            channel.setOption(ChannelOptions.autoRead, value: true).whenSuccess {
                channel.read()
            }
        }
    }

    func didTerminate() {}
}

private extension NSLock {
    func withLock<T>(_ body: () throws -> T) rethrows -> T {
        lock()
        defer { unlock() }
        return try body()
    }
}
