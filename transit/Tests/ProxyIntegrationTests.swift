import Foundation
import NIOCore
import NIOHTTP1
import NIOPosix
import XCTest
@testable import TransitCore

final class ProxyIntegrationTests: XCTestCase {
    func testStreamsThroughProxyAndPersistsUsage() async throws {
        let upstream = try await FixtureUpstream.start()
        let directory = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString)
        try FileManager.default.createDirectory(at: directory, withIntermediateDirectories: true)
        let store = try EventStore(databaseURL: directory.appendingPathComponent("usage.sqlite"))
        let pipeline = UsageEventPipeline(store: store)
        let keychain = KeychainSecretStore(service: "com.aarontianqx.transit.tests.\(UUID().uuidString)")
        try keychain.save("upstream-secret", reference: "relay-secret")
        let proxy = ProxyService(eventPipeline: pipeline, secretStore: keychain, threadCount: 1)
        defer {
            try? keychain.delete(reference: "relay-secret")
            try? keychain.delete(reference: "install-hmac-key")
        }
        registerCleanup(proxy: proxy, pipeline: pipeline, store: store, upstream: upstream, directory: directory)

        let proxyPort = try await bindProxy(
            proxy,
            upstreamPort: upstream.port
        )
        var request = URLRequest(url: URL(string: "http://127.0.0.1:\(proxyPort)/proxy/responses?stream=true")!)
        request.httpMethod = "POST"
        request.setValue("application/json", forHTTPHeaderField: "Content-Type")
        request.setValue("integration-agent", forHTTPHeaderField: "User-Agent")
        request.setValue("Bearer client-secret", forHTTPHeaderField: "Authorization")
        request.setValue("client-key", forHTTPHeaderField: "x-api-key")
        request.httpBody = Data(#"{"model":"request-model","input":"hello"}"#.utf8)

        let (data, response) = try await URLSession.shared.data(for: request)
        XCTAssertEqual((response as? HTTPURLResponse)?.statusCode, 200)
        let responseText = String(decoding: data, as: UTF8.self)
        XCTAssertTrue(responseText.contains("response.completed"))
        XCTAssertEqual(upstream.capture.uri, "/v1/responses?stream=true")
        XCTAssertEqual(upstream.capture.userAgent, "integration-agent")
        XCTAssertEqual(upstream.capture.header("x-integration-secret"), "upstream-secret")
        XCTAssertNil(upstream.capture.header("authorization"))
        XCTAssertNil(upstream.capture.header("x-api-key"))
        XCTAssertEqual(String(decoding: upstream.capture.body, as: UTF8.self), #"{"model":"request-model","input":"hello"}"#)

        pipeline.flush()
        let events = try store.recentEvents()
        XCTAssertEqual(events.count, 1)
        XCTAssertEqual(events[0].model, "resolved-model")
        XCTAssertEqual(events[0].usage.inputTokens, 120)
        XCTAssertEqual(events[0].usage.outputTokens, 30)
        XCTAssertEqual(events[0].usage.cachedInputTokens, 80)
        XCTAssertEqual(events[0].usage.reasoningTokens, 5)
        XCTAssertEqual(events[0].usage.totalTokens, 150)
        XCTAssertEqual(events[0].usageQuality, .reported)
        XCTAssertEqual(events[0].outcome, .completed)
        XCTAssertNotNil(events[0].keyFingerprint)
        XCTAssertFalse(events[0].keyFingerprint?.contains("upstream-secret") == true)

        pipeline.shutdown()
        try store.checkpoint()
        try store.close()
        let databaseArtifacts = try FileManager.default.contentsOfDirectory(
            at: directory,
            includingPropertiesForKeys: nil
        ).compactMap { try? Data(contentsOf: $0) }
        for forbidden in [
            "upstream-secret",
            "client-secret",
            "client-key",
            #"\"input\":\"hello\""#,
            "response.output_text.delta",
        ] {
            let needle = Data(forbidden.utf8)
            XCTAssertFalse(databaseArtifacts.contains { $0.range(of: needle) != nil }, "Persisted forbidden data: \(forbidden)")
        }

    }

    func testSSEFirstChunkIsForwardedBeforeResponseCompletes() async throws {
        let upstream = try await FixtureUpstream.start(secondChunkDelay: .seconds(1))
        let (store, directory) = try makeStore()
        let pipeline = UsageEventPipeline(store: store)
        let proxy = ProxyService(eventPipeline: pipeline, threadCount: 1)
        registerCleanup(proxy: proxy, pipeline: pipeline, store: store, upstream: upstream, directory: directory)
        let proxyPort = try await bindProxy(proxy, upstreamPort: upstream.port, authentication: .init())
        let request = makeRequest(port: proxyPort)
        let clock = ContinuousClock()
        let started = clock.now

        let (bytes, response) = try await URLSession.shared.bytes(for: request)
        var lines = bytes.lines.makeAsyncIterator()
        let firstLine = try await lines.next()
        let firstChunkElapsed = started.duration(to: clock.now)

        XCTAssertEqual((response as? HTTPURLResponse)?.statusCode, 200)
        XCTAssertTrue(firstLine?.contains("response.output_text.delta") == true)
        XCTAssertLessThan(firstChunkElapsed, .milliseconds(700))

        var sawCompleted = false
        while let line = try await lines.next() {
            if line.contains("response.completed") { sawCompleted = true }
        }
        XCTAssertTrue(sawCompleted)
        XCTAssertGreaterThan(started.duration(to: clock.now), .milliseconds(900))

    }

    func testSSEHeartbeatKeepsLongLivedResponseOpen() async throws {
        let upstream = try await FixtureUpstream.start(
            secondChunkDelay: .seconds(2),
            heartbeatInterval: .milliseconds(100)
        )
        let (store, directory) = try makeStore()
        let pipeline = UsageEventPipeline(store: store)
        let proxy = ProxyService(eventPipeline: pipeline, threadCount: 1)
        registerCleanup(proxy: proxy, pipeline: pipeline, store: store, upstream: upstream, directory: directory)
        let proxyPort = try await bindProxy(proxy, upstreamPort: upstream.port, authentication: .init())
        var request = makeRequest(port: proxyPort)
        request.timeoutInterval = 10
        let clock = ContinuousClock()
        let started = clock.now

        let (bytes, response) = try await URLSession.shared.bytes(for: request)
        var heartbeatCount = 0
        var sawCompleted = false
        for try await line in bytes.lines {
            if line == ": heartbeat" {
                heartbeatCount += 1
            } else if line.contains("response.completed") {
                sawCompleted = true
            }
        }

        XCTAssertEqual((response as? HTTPURLResponse)?.statusCode, 200)
        XCTAssertGreaterThanOrEqual(started.duration(to: clock.now), .seconds(1))
        XCTAssertTrue(sawCompleted)
        XCTAssertGreaterThan(heartbeatCount, 0)
        pipeline.flush()
        let event = try XCTUnwrap(store.recentEvents().first)
        XCTAssertEqual(event.outcome, .completed)
        XCTAssertEqual(event.usage.totalTokens, 150)
    }

    func testGracefulStopLetsInflightResponseFinish() async throws {
        let upstream = try await FixtureUpstream.start(secondChunkDelay: .milliseconds(350))
        let (store, directory) = try makeStore()
        let pipeline = UsageEventPipeline(store: store)
        let proxy = ProxyService(eventPipeline: pipeline, threadCount: 1)
        registerCleanup(proxy: proxy, pipeline: pipeline, store: store, upstream: upstream, directory: directory)
        let proxyPort = try await bindProxy(proxy, upstreamPort: upstream.port, authentication: .init())
        let request = makeRequest(port: proxyPort)
        let requestTask = Task { try await URLSession.shared.data(for: request) }

        try await waitUntil { proxy.status().activeRequests == 1 }
        await proxy.stop(gracePeriod: .seconds(2))
        let (data, response) = try await requestTask.value

        XCTAssertEqual((response as? HTTPURLResponse)?.statusCode, 200)
        XCTAssertTrue(String(decoding: data, as: UTF8.self).contains("response.completed"))
        XCTAssertEqual(proxy.status().state, .stopped)
        pipeline.flush()
        XCTAssertEqual(try store.recentEvents().first?.outcome, .completed)

    }

    func testDiscardedPreparedConfigurationKeepsOldListenerAndClosesStagedListener() async throws {
        let upstream = try await FixtureUpstream.start()
        let (store, directory) = try makeStore()
        let pipeline = UsageEventPipeline(store: store)
        let proxy = ProxyService(eventPipeline: pipeline, threadCount: 1)
        registerCleanup(proxy: proxy, pipeline: pipeline, store: store, upstream: upstream, directory: directory)
        let proxyPort = try await bindProxy(proxy, upstreamPort: upstream.port, authentication: .init())
        let probe = try await FixtureUpstream.start()
        let stagedPort = probe.port
        try await probe.shutdown()

        let prepared = try await proxy.prepare(configuration: TransitConfiguration(routes: [
            route(id: "integration-route", listenerPort: proxyPort, upstreamPort: upstream.port),
            route(id: "staged-route", listenerPort: stagedPort, upstreamPort: upstream.port, prefix: "/staged"),
        ]))
        await proxy.discard(prepared)

        let (_, response) = try await URLSession.shared.data(for: makeRequest(port: proxyPort))
        XCTAssertEqual((response as? HTTPURLResponse)?.statusCode, 200)
        XCTAssertEqual(proxy.status().listeners.map(\.port), [proxyPort])

        do {
            var stagedRequest = makeRequest(port: stagedPort, prefix: "/staged")
            stagedRequest.timeoutInterval = 0.5
            _ = try await URLSession.shared.data(for: stagedRequest)
            XCTFail("Discarded staged listener unexpectedly accepted a request")
        } catch {
            // Expected: discard closes the newly staged listener.
        }

    }

    func testBindingFailurePreservesExistingListenerAndRouteTable() async throws {
        let upstream = try await FixtureUpstream.start()
        let occupied = try await FixtureUpstream.start()
        let (store, directory) = try makeStore()
        let pipeline = UsageEventPipeline(store: store)
        let proxy = ProxyService(eventPipeline: pipeline, threadCount: 1)
        registerCleanup(proxy: proxy, pipeline: pipeline, store: store, upstream: upstream, directory: directory)
        addTeardownBlock { try? await occupied.shutdown() }
        let proxyPort = try await bindProxy(proxy, upstreamPort: upstream.port, authentication: .init())

        do {
            _ = try await proxy.prepare(configuration: TransitConfiguration(routes: [
                route(id: "integration-route", listenerPort: proxyPort, upstreamPort: upstream.port),
                route(id: "conflict-route", listenerPort: occupied.port, upstreamPort: upstream.port, prefix: "/conflict"),
            ]))
            XCTFail("Preparing a listener on an occupied port should fail")
        } catch {
            XCTAssertTrue(error is ProxyServiceError)
        }

        let (_, response) = try await URLSession.shared.data(for: makeRequest(port: proxyPort))
        XCTAssertEqual((response as? HTTPURLResponse)?.statusCode, 200)
        XCTAssertEqual(proxy.status().listeners.map(\.port), [proxyPort])

    }

    func testBestEffortStartupIsolatesListenerPortConflict() async throws {
        let upstream = try await FixtureUpstream.start()
        let occupied = try await FixtureUpstream.start()
        let portProbe = try await FixtureUpstream.start()
        let workingPort = portProbe.port
        try await portProbe.shutdown()
        let (store, directory) = try makeStore()
        let pipeline = UsageEventPipeline(store: store)
        let proxy = ProxyService(eventPipeline: pipeline, threadCount: 1)
        registerCleanup(proxy: proxy, pipeline: pipeline, store: store, upstream: upstream, directory: directory)
        addTeardownBlock { try? await occupied.shutdown() }

        let status = try await proxy.startBestEffort(configuration: TransitConfiguration(routes: [
            route(id: "working-route", listenerPort: workingPort, upstreamPort: upstream.port),
            route(id: "conflict-route", listenerPort: occupied.port, upstreamPort: upstream.port, prefix: "/conflict"),
        ]))

        XCTAssertEqual(status.state, .degraded)
        XCTAssertEqual(status.listeners.first(where: { $0.port == workingPort })?.state, .ready)
        XCTAssertEqual(status.listeners.first(where: { $0.port == occupied.port })?.state, .failed)
        let (_, response) = try await URLSession.shared.data(for: makeRequest(port: workingPort))
        XCTAssertEqual((response as? HTTPURLResponse)?.statusCode, 200)
    }

    func testChunkedRequestBodyIsStreamedToUpstream() async throws {
        let upstream = try await FixtureUpstream.start()
        let (store, directory) = try makeStore()
        let pipeline = UsageEventPipeline(store: store)
        let proxy = ProxyService(eventPipeline: pipeline, threadCount: 1)
        registerCleanup(proxy: proxy, pipeline: pipeline, store: store, upstream: upstream, directory: directory)
        let proxyPort = try await bindProxy(proxy, upstreamPort: upstream.port, authentication: .init())

        let result = try await sendRawRequest(
            port: proxyPort,
            bodyChunks: [Data(#"{"model":"chunked","#.utf8), Data(#""input":"hello"}"#.utf8)]
        )

        XCTAssertEqual(result.statusCode, 200)
        XCTAssertEqual(String(decoding: upstream.capture.body, as: UTF8.self), #"{"model":"chunked","input":"hello"}"#)
        XCTAssertEqual(upstream.capture.header("transfer-encoding")?.lowercased(), "chunked")

    }

    func testClientCancellationCancelsUpstreamAndPersistsCancelledEvent() async throws {
        let upstream = try await FixtureUpstream.start(secondChunkDelay: .seconds(2))
        let (store, directory) = try makeStore()
        let pipeline = UsageEventPipeline(store: store)
        let proxy = ProxyService(eventPipeline: pipeline, threadCount: 1)
        registerCleanup(proxy: proxy, pipeline: pipeline, store: store, upstream: upstream, directory: directory)
        let proxyPort = try await bindProxy(proxy, upstreamPort: upstream.port, authentication: .init())

        let partial = try await sendRawRequest(
            port: proxyPort,
            bodyChunks: [Data(#"{"model":"cancelled","input":"hello"}"#.utf8)],
            closeAfterResponseHead: true
        )
        XCTAssertEqual(partial.statusCode, 200)

        try await waitUntil("cancelled event") {
            pipeline.flush()
            return (try? store.recentEvents().first?.outcome) == .cancelled
        }
        let event = try XCTUnwrap(store.recentEvents().first)
        XCTAssertEqual(event.errorCode, "client_cancelled")

    }

    func testStorageFailureDoesNotBreakProxyResponse() async throws {
        let upstream = try await FixtureUpstream.start()
        let pipeline = UsageEventPipeline(store: FailingProxyEventWriter())
        let proxy = ProxyService(eventPipeline: pipeline, threadCount: 1)
        addTeardownBlock {
            try? await proxy.shutdown()
            pipeline.shutdown()
            try? await upstream.shutdown()
        }
        let proxyPort = try await bindProxy(proxy, upstreamPort: upstream.port, authentication: .init())

        let (data, response) = try await URLSession.shared.data(for: makeRequest(port: proxyPort))
        pipeline.flush()

        XCTAssertEqual((response as? HTTPURLResponse)?.statusCode, 200)
        XCTAssertTrue(String(decoding: data, as: UTF8.self).contains("response.completed"))
        XCTAssertEqual(pipeline.status().persisted, 0)
        XCTAssertEqual(pipeline.status().dropped, 1)
        XCTAssertEqual(pipeline.status().lastError, "fixture storage failure")
    }

    func testUpstreamHTTPFailureIsForwardedAndPersistsReportedUsage() async throws {
        let upstream = try await FixtureUpstream.start(responseStatus: .serviceUnavailable)
        let (store, directory) = try makeStore()
        let pipeline = UsageEventPipeline(store: store)
        let proxy = ProxyService(eventPipeline: pipeline, threadCount: 1)
        registerCleanup(proxy: proxy, pipeline: pipeline, store: store, upstream: upstream, directory: directory)
        let proxyPort = try await bindProxy(proxy, upstreamPort: upstream.port, authentication: .init())

        let (data, response) = try await URLSession.shared.data(for: makeRequest(port: proxyPort))
        pipeline.flush()
        let event = try XCTUnwrap(store.recentEvents().first)

        XCTAssertEqual((response as? HTTPURLResponse)?.statusCode, 503)
        XCTAssertTrue(String(decoding: data, as: UTF8.self).contains("response.completed"))
        XCTAssertEqual(event.statusCode, 503)
        XCTAssertEqual(event.outcome, .failed)
        XCTAssertEqual(event.errorCode, "upstream_http_status")
        XCTAssertEqual(event.usageQuality, .reported)
        XCTAssertEqual(event.usage.totalTokens, 150)
    }

    func testUpstreamTransportFailureReturns502AndPersistsFailedEvent() async throws {
        let portProbe = try await FixtureUpstream.start()
        let unavailablePort = portProbe.port
        try await portProbe.shutdown()
        let (store, directory) = try makeStore()
        let pipeline = UsageEventPipeline(store: store)
        let proxy = ProxyService(eventPipeline: pipeline, threadCount: 1)
        addTeardownBlock {
            try? await proxy.shutdown()
            pipeline.shutdown()
            try? store.close()
            try? FileManager.default.removeItem(at: directory)
        }
        let proxyPort = try await bindProxy(proxy, upstreamPort: unavailablePort, authentication: .init())

        let request = makeRequest(port: proxyPort)
        let responseTask = Task { try await URLSession.shared.data(for: request) }
        try await waitUntil("failed event") {
            pipeline.flush()
            return (try? store.recentEvents().first?.outcome) == .failed
        }
        let (data, response) = try await responseTask.value
        let event = try XCTUnwrap(store.recentEvents().first)

        XCTAssertEqual((response as? HTTPURLResponse)?.statusCode, 502)
        XCTAssertTrue(String(decoding: data, as: UTF8.self).contains("upstream_request_failed"))
        XCTAssertNil(event.statusCode)
        XCTAssertEqual(event.outcome, .failed)
        XCTAssertEqual(event.errorCode, "upstream_request_failed")
        XCTAssertEqual(event.usageQuality, .missing)
        XCTAssertFalse(event.usage.hasAnyValue)
    }

    func testAuthenticationPoliciesRemainIsolatedAcrossRoutes() async throws {
        let passthroughUpstream = try await FixtureUpstream.start()
        let relayUpstream = try await FixtureUpstream.start()
        let portProbe = try await FixtureUpstream.start()
        let proxyPort = portProbe.port
        try await portProbe.shutdown()
        let (store, directory) = try makeStore()
        let pipeline = UsageEventPipeline(store: store)
        let keychain = KeychainSecretStore(service: "com.aarontianqx.transit.tests.\(UUID().uuidString)")
        try keychain.save("relay-value", reference: "relay-secret")
        let proxy = ProxyService(eventPipeline: pipeline, secretStore: keychain, threadCount: 1)
        addTeardownBlock {
            try? await proxy.shutdown()
            pipeline.shutdown()
            try? store.close()
            try? await passthroughUpstream.shutdown()
            try? await relayUpstream.shutdown()
            try? keychain.delete(reference: "relay-secret")
            try? keychain.delete(reference: "install-hmac-key")
            try? FileManager.default.removeItem(at: directory)
        }

        try await proxy.apply(configuration: TransitConfiguration(routes: [
            RouteConfiguration(
                id: "passthrough-route",
                displayName: "Passthrough",
                agentID: "agent-a",
                listener: .init(port: proxyPort, pathPrefix: "/pass"),
                upstream: "http://127.0.0.1:\(passthroughUpstream.port)/v1",
                protocolType: .openAIResponses,
                authentication: .init(),
                allowInsecureHTTP: true
            ),
            RouteConfiguration(
                id: "relay-route",
                displayName: "Relay",
                agentID: "agent-b",
                listener: .init(port: proxyPort, pathPrefix: "/relay"),
                upstream: "http://127.0.0.1:\(relayUpstream.port)/v1",
                protocolType: .openAIResponses,
                authentication: .init(mode: .replaceBearer, secretRef: "relay-secret"),
                allowInsecureHTTP: true
            ),
        ]))

        var passthroughRequest = makeRequest(port: proxyPort, prefix: "/pass")
        passthroughRequest.setValue("Bearer client-value", forHTTPHeaderField: "Authorization")
        passthroughRequest.setValue("client-api-key", forHTTPHeaderField: "x-api-key")
        let (_, passthroughResponse) = try await URLSession.shared.data(for: passthroughRequest)

        var relayRequest = makeRequest(port: proxyPort, prefix: "/relay")
        relayRequest.setValue("Bearer stale-client-value", forHTTPHeaderField: "Authorization")
        relayRequest.setValue("stale-client-key", forHTTPHeaderField: "x-api-key")
        let (_, relayResponse) = try await URLSession.shared.data(for: relayRequest)

        XCTAssertEqual((passthroughResponse as? HTTPURLResponse)?.statusCode, 200)
        XCTAssertEqual(passthroughUpstream.capture.header("authorization"), "Bearer client-value")
        XCTAssertEqual(passthroughUpstream.capture.header("x-api-key"), "client-api-key")
        XCTAssertEqual((relayResponse as? HTTPURLResponse)?.statusCode, 200)
        XCTAssertEqual(relayUpstream.capture.header("authorization"), "Bearer relay-value")
        XCTAssertNil(relayUpstream.capture.header("x-api-key"))
    }

    func testHopByHopHeadersAreRemovedAndUpstreamHostIsRebuilt() async throws {
        let upstream = try await FixtureUpstream.start()
        let (store, directory) = try makeStore()
        let pipeline = UsageEventPipeline(store: store)
        let proxy = ProxyService(eventPipeline: pipeline, threadCount: 1)
        registerCleanup(proxy: proxy, pipeline: pipeline, store: store, upstream: upstream, directory: directory)
        let proxyPort = try await bindProxy(proxy, upstreamPort: upstream.port, authentication: .init())

        let result = try await sendRawRequest(
            port: proxyPort,
            bodyChunks: [Data(#"{"model":"headers","input":"hello"}"#.utf8)],
            additionalHeaders: [
                "connection": "x-private-hop, keep-alive",
                "x-private-hop": "must-not-forward",
                "keep-alive": "timeout=5",
                "x-business-header": "preserved",
            ]
        )

        XCTAssertEqual(result.statusCode, 200)
        XCTAssertNil(upstream.capture.header("connection"))
        XCTAssertNil(upstream.capture.header("x-private-hop"))
        XCTAssertNil(upstream.capture.header("keep-alive"))
        XCTAssertEqual(upstream.capture.header("x-business-header"), "preserved")
        XCTAssertEqual(upstream.capture.header("host"), "127.0.0.1:\(upstream.port)")
        XCTAssertEqual(upstream.capture.header("accept-encoding"), "identity")
        XCTAssertNil(result.header("connection"))
        XCTAssertNil(result.header("x-private-response"))
        XCTAssertNil(result.header("keep-alive"))
        XCTAssertEqual(result.header("x-response-business"), "preserved")
    }

    private func registerCleanup(
        proxy: ProxyService,
        pipeline: UsageEventPipeline,
        store: EventStore,
        upstream: FixtureUpstream,
        directory: URL
    ) {
        addTeardownBlock {
            try? await proxy.shutdown()
            pipeline.shutdown()
            try? store.close()
            try? await upstream.shutdown()
            try? FileManager.default.removeItem(at: directory)
        }
    }

    private func makeStore() throws -> (EventStore, URL) {
        let directory = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString)
        try FileManager.default.createDirectory(at: directory, withIntermediateDirectories: true)
        return (try EventStore(databaseURL: directory.appendingPathComponent("usage.sqlite")), directory)
    }

    private func makeRequest(port: Int, prefix: String = "/proxy") -> URLRequest {
        var request = URLRequest(url: URL(string: "http://127.0.0.1:\(port)\(prefix)/responses?stream=true")!)
        request.httpMethod = "POST"
        request.setValue("application/json", forHTTPHeaderField: "Content-Type")
        request.httpBody = Data(#"{"model":"request-model","input":"hello"}"#.utf8)
        return request
    }

    private func route(
        id: String,
        listenerPort: Int,
        upstreamPort: Int,
        prefix: String = "/proxy"
    ) -> RouteConfiguration {
        RouteConfiguration(
            id: id,
            displayName: id,
            agentID: "integration-agent",
            listener: .init(port: listenerPort, pathPrefix: prefix),
            upstream: "http://127.0.0.1:\(upstreamPort)/v1",
            protocolType: .openAIResponses,
            allowInsecureHTTP: true
        )
    }

    private func waitUntil(
        _ description: String = "condition",
        timeout: Duration = .seconds(3),
        condition: @escaping @Sendable () -> Bool
    ) async throws {
        let clock = ContinuousClock()
        let deadline = clock.now.advanced(by: timeout)
        while !condition() {
            guard clock.now < deadline else { throw IntegrationTestError.timedOut(description) }
            try await Task.sleep(for: .milliseconds(20))
        }
    }

    private func bindProxy(
        _ proxy: ProxyService,
        upstreamPort: Int,
        authentication: AuthenticationPolicy = AuthenticationPolicy(
            mode: .replaceHeader,
            secretRef: "relay-secret",
            headerName: "x-integration-secret"
        )
    ) async throws -> Int {
        var lastError: Error?
        for _ in 0..<20 {
            let port = Int.random(in: 20_000...55_000)
            let route = RouteConfiguration(
                id: "integration-route",
                displayName: "Integration",
                agentID: "integration-agent",
                listener: .init(port: port, pathPrefix: "/proxy"),
                upstream: "http://127.0.0.1:\(upstreamPort)/v1",
                protocolType: .openAIResponses,
                authentication: authentication,
                allowInsecureHTTP: true
            )
            do {
                try await proxy.apply(configuration: TransitConfiguration(routes: [route]))
                return port
            } catch {
                lastError = error
            }
        }
        throw lastError ?? IntegrationTestError.noPort
    }

}

private enum IntegrationTestError: Error {
    case noPort
    case timedOut(String)
}

private struct RawHTTPResult: Sendable {
    let statusCode: Int
    let body: Data
    let headers: [String: String]

    func header(_ name: String) -> String? {
        headers[name.lowercased()]
    }
}

private enum RawHTTPClientError: Error {
    case connectionClosed
}

private struct FailingProxyEventWriter: UsageEventWriting {
    func save(_ event: UsageEvent) throws {
        throw FailingProxyEventWriterError()
    }
}

private struct FailingProxyEventWriterError: LocalizedError {
    var errorDescription: String? { "fixture storage failure" }
}

private func sendRawRequest(
    port: Int,
    bodyChunks: [Data],
    closeAfterResponseHead: Bool = false,
    closeAfterFirstResponseBody: Bool = false,
    additionalHeaders: [String: String] = [:]
) async throws -> RawHTTPResult {
    let group = MultiThreadedEventLoopGroup(numberOfThreads: 1)
    let promise = group.next().makePromise(of: RawHTTPResult.self)
    var channel: Channel?
    do {
        let connected = try await ClientBootstrap(group: group)
            .channelInitializer { channel in
                channel.pipeline.addHTTPClientHandlers().flatMap {
                    channel.pipeline.addHandler(RawHTTPResponseHandler(
                        promise: promise,
                        closeAfterResponseHead: closeAfterResponseHead,
                        closeAfterFirstResponseBody: closeAfterFirstResponseBody
                    ))
                }
            }
            .connect(host: "127.0.0.1", port: port)
            .get()
        channel = connected

        var headers = HTTPHeaders()
        headers.add(name: "host", value: "127.0.0.1:\(port)")
        headers.add(name: "content-type", value: "application/json")
        headers.add(name: "transfer-encoding", value: "chunked")
        for (name, value) in additionalHeaders {
            headers.replaceOrAdd(name: name, value: value)
        }
        try await connected.writeAndFlush(HTTPClientRequestPart.head(.init(
            version: .http1_1,
            method: .POST,
            uri: "/proxy/responses?stream=true",
            headers: headers
        ))).get()
        for data in bodyChunks {
            var buffer = connected.allocator.buffer(capacity: data.count)
            buffer.writeBytes(data)
            try await connected.writeAndFlush(HTTPClientRequestPart.body(.byteBuffer(buffer))).get()
        }
        try await connected.writeAndFlush(HTTPClientRequestPart.end(nil)).get()

        let result = try await promise.futureResult.get()
        try? await connected.close()
        try await group.shutdownGracefully()
        return result
    } catch {
        try? await channel?.close()
        try? await group.shutdownGracefully()
        throw error
    }
}

private final class RawHTTPResponseHandler: ChannelInboundHandler, @unchecked Sendable {
    typealias InboundIn = HTTPClientResponsePart

    private let promise: EventLoopPromise<RawHTTPResult>
    private let closeAfterResponseHead: Bool
    private let closeAfterFirstResponseBody: Bool
    private var statusCode = 0
    private var body = Data()
    private var headers: [String: String] = [:]
    private var completed = false

    init(
        promise: EventLoopPromise<RawHTTPResult>,
        closeAfterResponseHead: Bool,
        closeAfterFirstResponseBody: Bool
    ) {
        self.promise = promise
        self.closeAfterResponseHead = closeAfterResponseHead
        self.closeAfterFirstResponseBody = closeAfterFirstResponseBody
    }

    func channelRead(context: ChannelHandlerContext, data: NIOAny) {
        switch unwrapInboundIn(data) {
        case .head(let head):
            statusCode = Int(head.status.code)
            headers = Dictionary(
                head.headers.map { ($0.name.lowercased(), $0.value) },
                uniquingKeysWith: { _, last in last }
            )
            if closeAfterResponseHead {
                complete()
                context.close(promise: nil)
            }
        case .body(let buffer):
            if let bytes = buffer.getBytes(at: buffer.readerIndex, length: buffer.readableBytes) {
                body.append(contentsOf: bytes)
            }
            if closeAfterFirstResponseBody {
                complete()
                context.close(promise: nil)
            }
        case .end:
            complete()
        }
    }

    func channelInactive(context: ChannelHandlerContext) {
        if !completed {
            completed = true
            promise.fail(RawHTTPClientError.connectionClosed)
        }
        context.fireChannelInactive()
    }

    func errorCaught(context: ChannelHandlerContext, error: Error) {
        if !completed {
            completed = true
            promise.fail(error)
        }
        context.close(promise: nil)
    }

    private func complete() {
        guard !completed else { return }
        completed = true
        promise.succeed(RawHTTPResult(statusCode: statusCode, body: body, headers: headers))
    }
}

private final class FixtureCapture: @unchecked Sendable {
    private let lock = NSLock()
    private var storedURI = ""
    private var storedUserAgent = ""
    private var storedBody = Data()
    private var storedHeaders: [String: String] = [:]
    private var storedBodyChunks = 0
    private var connectionClosed = false

    var uri: String { lock.withLock { storedURI } }
    var userAgent: String { lock.withLock { storedUserAgent } }
    var body: Data { lock.withLock { storedBody } }
    var bodyChunks: Int { lock.withLock { storedBodyChunks } }
    var isConnectionClosed: Bool { lock.withLock { connectionClosed } }

    func header(_ name: String) -> String? {
        lock.withLock { storedHeaders[name.lowercased()] }
    }

    func setHead(_ head: HTTPRequestHead) {
        lock.withLock {
            storedURI = head.uri
            storedUserAgent = head.headers.first(name: "user-agent") ?? ""
            storedHeaders = Dictionary(
                head.headers.map { ($0.name.lowercased(), $0.value) },
                uniquingKeysWith: { _, last in last }
            )
        }
    }

    func append(_ buffer: ByteBuffer) {
        guard let bytes = buffer.getBytes(at: buffer.readerIndex, length: buffer.readableBytes) else { return }
        lock.withLock {
            storedBody.append(contentsOf: bytes)
            storedBodyChunks += 1
        }
    }

    func markConnectionClosed() {
        lock.withLock { connectionClosed = true }
    }
}

private final class FixtureUpstream: @unchecked Sendable {
    let port: Int
    let capture: FixtureCapture
    private let channel: Channel
    private let group: MultiThreadedEventLoopGroup

    private init(
        port: Int,
        capture: FixtureCapture,
        channel: Channel,
        group: MultiThreadedEventLoopGroup
    ) {
        self.port = port
        self.capture = capture
        self.channel = channel
        self.group = group
    }

    static func start(
        secondChunkDelay: TimeAmount? = nil,
        heartbeatInterval: TimeAmount? = nil,
        responseStatus: HTTPResponseStatus = .ok
    ) async throws -> FixtureUpstream {
        let group = MultiThreadedEventLoopGroup(numberOfThreads: 1)
        let capture = FixtureCapture()
        let channel = try await ServerBootstrap(group: group)
            .serverChannelOption(ChannelOptions.socketOption(.so_reuseaddr), value: 1)
            .childChannelInitializer { channel in
                return channel.pipeline.configureHTTPServerPipeline().flatMap {
                    channel.pipeline.addHandler(FixtureHandler(
                        capture: capture,
                        secondChunkDelay: secondChunkDelay,
                        heartbeatInterval: heartbeatInterval,
                        responseStatus: responseStatus
                    ))
                }
            }
            .bind(host: "127.0.0.1", port: 0)
            .get()
        return FixtureUpstream(
            port: channel.localAddress!.port!,
            capture: capture,
            channel: channel,
            group: group
        )
    }

    func shutdown() async throws {
        try await channel.close()
        try await group.shutdownGracefully()
    }
}

private final class FixtureHandler: ChannelInboundHandler, @unchecked Sendable {
    typealias InboundIn = HTTPServerRequestPart
    typealias OutboundOut = HTTPServerResponsePart

    private let capture: FixtureCapture
    private let secondChunkDelay: TimeAmount?
    private let heartbeatInterval: TimeAmount?
    private let responseStatus: HTTPResponseStatus
    private var heartbeatTask: RepeatedTask?

    init(
        capture: FixtureCapture,
        secondChunkDelay: TimeAmount?,
        heartbeatInterval: TimeAmount?,
        responseStatus: HTTPResponseStatus
    ) {
        self.capture = capture
        self.secondChunkDelay = secondChunkDelay
        self.heartbeatInterval = heartbeatInterval
        self.responseStatus = responseStatus
    }

    func channelRead(context: ChannelHandlerContext, data: NIOAny) {
        switch unwrapInboundIn(data) {
        case .head(let head):
            capture.setHead(head)
        case .body(let body):
            capture.append(body)
        case .end:
            var headers = HTTPHeaders()
            headers.add(name: "content-type", value: "text/event-stream")
            headers.add(name: "connection", value: "x-private-response, keep-alive")
            headers.add(name: "x-private-response", value: "must-not-forward")
            headers.add(name: "keep-alive", value: "timeout=5")
            headers.add(name: "x-response-business", value: "preserved")
            context.write(wrapOutboundOut(.head(.init(
                version: .http1_1,
                status: responseStatus,
                headers: headers
            ))), promise: nil)
            let first = ByteBuffer(string: #"data: {"type":"response.output_text.delta","delta":"hello"}"# + "\n\n")
            let second = ByteBuffer(string: #"data: {"type":"response.completed","response":{"model":"resolved-model","usage":{"input_tokens":120,"output_tokens":30,"total_tokens":150,"input_tokens_details":{"cached_tokens":80},"output_tokens_details":{"reasoning_tokens":5}}}}"# + "\n\n")
            context.writeAndFlush(wrapOutboundOut(.body(.byteBuffer(first))), promise: nil)
            if let secondChunkDelay {
                let channel = context.channel
                if let heartbeatInterval {
                    heartbeatTask = context.eventLoop.scheduleRepeatedTask(
                        initialDelay: heartbeatInterval,
                        delay: heartbeatInterval
                    ) { task in
                        guard channel.isActive else {
                            task.cancel()
                            return
                        }
                        channel.writeAndFlush(
                            HTTPServerResponsePart.body(.byteBuffer(ByteBuffer(string: ": heartbeat\n\n"))),
                            promise: nil
                        )
                    }
                }
                context.eventLoop.scheduleTask(in: secondChunkDelay) { [weak self] in
                    self?.heartbeatTask?.cancel()
                    guard channel.isActive else { return }
                    channel.write(HTTPServerResponsePart.body(.byteBuffer(second)), promise: nil)
                    channel.writeAndFlush(HTTPServerResponsePart.end(nil), promise: nil)
                }
            } else {
                context.write(wrapOutboundOut(.body(.byteBuffer(second))), promise: nil)
                context.writeAndFlush(wrapOutboundOut(.end(nil)), promise: nil)
            }
        }
    }

    func channelInactive(context: ChannelHandlerContext) {
        heartbeatTask?.cancel()
        capture.markConnectionClosed()
        context.fireChannelInactive()
    }
}

private extension NSLock {
    func withLock<T>(_ body: () throws -> T) rethrows -> T {
        lock()
        defer { unlock() }
        return try body()
    }
}
