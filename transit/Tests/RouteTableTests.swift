import XCTest
@testable import TransitCore

final class RouteTableTests: XCTestCase {
    func testLongestSegmentPrefixAndPathJoin() {
        let broad = route(id: "broad", prefix: "/a", upstream: "https://one.example.com/api")
        let nested = route(id: "nested", prefix: "/a/b", upstream: "https://two.example.com/v1")
        let table = RouteTable(routes: [broad, nested])

        let match = table.match(port: 8787, requestURI: "/a/b/responses?stream=true")
        XCTAssertEqual(match?.route.id, "nested")
        XCTAssertEqual(match?.upstreamURL.absoluteString, "https://two.example.com/v1/responses?stream=true")
        XCTAssertNil(table.match(port: 8787, requestURI: "/abc/responses"))
    }

    func testDedicatedPortRoutesIndependently() {
        let first = route(id: "first", port: 8787, prefix: "/", upstream: "https://one.example.com")
        let second = route(id: "second", port: 8788, prefix: "/", upstream: "https://two.example.com")
        let table = RouteTable(routes: [first, second])
        XCTAssertEqual(table.match(port: 8788, requestURI: "/messages")?.route.id, "second")
    }

    private func route(id: String, port: Int = 8787, prefix: String, upstream: String) -> RouteConfiguration {
        RouteConfiguration(
            id: id,
            displayName: id,
            agentID: "agent",
            listener: .init(port: port, pathPrefix: prefix),
            upstream: upstream,
            protocolType: .openAIResponses
        )
    }
}
