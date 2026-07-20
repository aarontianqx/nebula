import Foundation

public struct RouteTable: Sendable {
    private let routesByPort: [Int: [RouteConfiguration]]
    private let pricingPolicies: [String: PricingPolicy]

    public init(routes: [RouteConfiguration], pricingPolicies: [PricingPolicy] = []) {
        routesByPort = Dictionary(grouping: routes.filter(\.enabled), by: { $0.listener.port })
            .mapValues { routes in
                routes.sorted { $0.listener.pathPrefix.count > $1.listener.pathPrefix.count }
            }
        self.pricingPolicies = Dictionary(uniqueKeysWithValues: pricingPolicies.map { ($0.id, $0) })
    }

    public var ports: [Int] {
        routesByPort.keys.sorted()
    }

    public func routes(on port: Int) -> [RouteConfiguration] {
        routesByPort[port] ?? []
    }

    public func match(port: Int, requestURI: String) -> RouteMatch? {
        guard let components = URLComponents(string: requestURI) else { return nil }
        let requestPath = components.percentEncodedPath.isEmpty ? "/" : components.percentEncodedPath
        guard let route = routesByPort[port]?.first(where: {
            Self.prefix($0.listener.pathPrefix, matches: requestPath)
        }), let upstreamURL = Self.upstreamURL(route: route, requestComponents: components)
        else { return nil }
        return RouteMatch(
            route: route,
            upstreamURL: upstreamURL,
            pricingPolicy: route.pricingPolicyID.flatMap { pricingPolicies[$0] }
        )
    }

    public static func prefix(_ prefix: String, matches path: String) -> Bool {
        if prefix == "/" { return path.hasPrefix("/") }
        return path == prefix || path.hasPrefix(prefix + "/")
    }

    private static func upstreamURL(
        route: RouteConfiguration,
        requestComponents: URLComponents
    ) -> URL? {
        guard var upstream = URLComponents(string: route.upstream) else { return nil }
        let requestPath = requestComponents.percentEncodedPath.isEmpty ? "/" : requestComponents.percentEncodedPath
        let prefix = route.listener.pathPrefix
        let remainder: String
        if prefix == "/" {
            remainder = requestPath
        } else if requestPath == prefix {
            remainder = "/"
        } else {
            remainder = String(requestPath.dropFirst(prefix.count))
        }
        let basePath = upstream.percentEncodedPath == "/"
            ? ""
            : upstream.percentEncodedPath.trimmingCharacters(in: CharacterSet(charactersIn: "/"))
        let suffix = remainder.trimmingCharacters(in: CharacterSet(charactersIn: "/"))
        upstream.percentEncodedPath = "/" + [basePath, suffix].filter { !$0.isEmpty }.joined(separator: "/")
        upstream.percentEncodedQuery = requestComponents.percentEncodedQuery
        upstream.fragment = nil
        return upstream.url
    }
}
