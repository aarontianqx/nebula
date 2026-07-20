import Foundation

public enum AuthenticationMode: String, Codable, CaseIterable, Sendable {
    case passthrough
    case replaceBearer = "replace_bearer"
    case replaceHeader = "replace_header"
}

public struct AuthenticationPolicy: Codable, Equatable, Sendable {
    public var mode: AuthenticationMode
    public var secretRef: String?
    public var headerName: String?

    public init(
        mode: AuthenticationMode = .passthrough,
        secretRef: String? = nil,
        headerName: String? = nil
    ) {
        self.mode = mode
        self.secretRef = secretRef
        self.headerName = headerName
    }

    enum CodingKeys: String, CodingKey {
        case mode
        case secretRef = "secret_ref"
        case headerName = "header_name"
    }
}

public struct ListenerConfiguration: Codable, Equatable, Sendable {
    public var port: Int
    public var pathPrefix: String

    public init(port: Int = 8787, pathPrefix: String = "/") {
        self.port = port
        self.pathPrefix = pathPrefix
    }

    enum CodingKeys: String, CodingKey {
        case port
        case pathPrefix = "path_prefix"
    }
}

public struct RouteConfiguration: Codable, Equatable, Identifiable, Sendable {
    public var id: String
    public var displayName: String
    public var agentID: String
    public var listener: ListenerConfiguration
    public var upstream: String
    public var protocolType: UsageProtocol
    public var authentication: AuthenticationPolicy
    public var pricingPolicyID: String?
    public var enabled: Bool
    public var allowInsecureHTTP: Bool

    public init(
        id: String = UUID().uuidString.lowercased(),
        displayName: String = "",
        agentID: String = "",
        listener: ListenerConfiguration = ListenerConfiguration(),
        upstream: String = "",
        protocolType: UsageProtocol = .openAIResponses,
        authentication: AuthenticationPolicy = AuthenticationPolicy(),
        pricingPolicyID: String? = nil,
        enabled: Bool = true,
        allowInsecureHTTP: Bool = false
    ) {
        self.id = id
        self.displayName = displayName
        self.agentID = agentID
        self.listener = listener
        self.upstream = upstream
        self.protocolType = protocolType
        self.authentication = authentication
        self.pricingPolicyID = pricingPolicyID
        self.enabled = enabled
        self.allowInsecureHTTP = allowInsecureHTTP
    }

    enum CodingKeys: String, CodingKey {
        case id
        case displayName = "display_name"
        case agentID = "agent_id"
        case listener
        case upstream
        case protocolType = "protocol"
        case authentication = "auth"
        case pricingPolicyID = "pricing_policy_id"
        case enabled
        case allowInsecureHTTP = "allow_insecure_http"
    }

    public var localBaseURL: String {
        let suffix = listener.pathPrefix == "/" ? "" : listener.pathPrefix
        return "http://127.0.0.1:\(listener.port)\(suffix)"
    }
}

public struct RouteMatch: Equatable, Sendable {
    public let route: RouteConfiguration
    public let upstreamURL: URL
    public let pricingPolicy: PricingPolicy?

    public init(route: RouteConfiguration, upstreamURL: URL, pricingPolicy: PricingPolicy? = nil) {
        self.route = route
        self.upstreamURL = upstreamURL
        self.pricingPolicy = pricingPolicy
    }
}
