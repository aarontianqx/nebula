import Foundation

public struct StorageConfiguration: Codable, Equatable, Sendable {
    public var retentionDays: Int

    public init(retentionDays: Int = 90) {
        self.retentionDays = retentionDays
    }

    enum CodingKeys: String, CodingKey {
        case retentionDays = "retention_days"
    }
}
public struct TransitConfiguration: Codable, Equatable, Sendable {
    public static let currentVersion = 1

    public var version: Int
    public var routes: [RouteConfiguration]
    public var pricingPolicies: [PricingPolicy]
    public var storage: StorageConfiguration

    public init(
        version: Int = TransitConfiguration.currentVersion,
        routes: [RouteConfiguration] = [],
        pricingPolicies: [PricingPolicy] = [],
        storage: StorageConfiguration = StorageConfiguration()
    ) {
        self.version = version
        self.routes = routes
        self.pricingPolicies = pricingPolicies
        self.storage = storage
    }

    enum CodingKeys: String, CodingKey {
        case version
        case routes
        case pricingPolicies = "pricing_policies"
        case storage
    }

    public func pricingPolicy(id: String?) -> PricingPolicy? {
        guard let id else { return nil }
        return pricingPolicies.first { $0.id == id }
    }
}
