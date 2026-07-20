import Foundation
import XCTest
@testable import TransitCore

final class ConfigurationTests: XCTestCase {
    func testValidConfigurationHasNoErrors() {
        let route = RouteConfiguration(
            displayName: "Primary",
            agentID: "agent-1",
            listener: ListenerConfiguration(port: 8787, pathPrefix: "/primary"),
            upstream: "https://llm.example.com/v1",
            protocolType: .openAIResponses
        )
        XCTAssertTrue(ConfigurationValidator.errors(in: TransitConfiguration(routes: [route])).isEmpty)
    }

    func testRejectsDuplicateListenerMatchAndInsecureUpstream() {
        let first = RouteConfiguration(
            id: "first",
            displayName: "First",
            agentID: "agent-1",
            upstream: "http://one.example.com/v1"
        )
        let second = RouteConfiguration(
            id: "second",
            displayName: "Second",
            agentID: "agent-2",
            upstream: "https://two.example.com/v1"
        )
        let codes = Set(ConfigurationValidator.errors(in: TransitConfiguration(routes: [first, second])).map(\.code))
        XCTAssertTrue(codes.contains("insecure_upstream_not_allowed"))
        XCTAssertTrue(codes.contains("duplicate_listener_match"))
    }

    func testNestedPrefixesAreAllowed() {
        let first = RouteConfiguration(
            id: "first",
            displayName: "First",
            agentID: "agent-1",
            listener: .init(port: 8787, pathPrefix: "/a"),
            upstream: "https://one.example.com/v1"
        )
        let second = RouteConfiguration(
            id: "second",
            displayName: "Second",
            agentID: "agent-2",
            listener: .init(port: 8787, pathPrefix: "/a/b"),
            upstream: "https://two.example.com/v1"
        )
        XCTAssertFalse(ConfigurationValidator.errors(in: TransitConfiguration(routes: [first, second])).contains {
            $0.code == "duplicate_listener_match"
        })
    }

    func testConfigurationStoreRejectsUnknownKeys() throws {
        let directory = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString)
        try FileManager.default.createDirectory(at: directory, withIntermediateDirectories: true)
        defer { try? FileManager.default.removeItem(at: directory) }
        let url = directory.appendingPathComponent("config.json")
        try Data(#"{"version":1,"routes":[],"pricing_policies":[],"storage":{"retention_days":90,"typo":true}}"#.utf8)
            .write(to: url)
        XCTAssertThrowsError(try ConfigurationStore(fileURL: url).load()) { error in
            guard case ConfigurationStoreError.unknownKey(let path) = error else {
                return XCTFail("Unexpected error: \(error)")
            }
            XCTAssertEqual(path, "storage.typo")
        }
    }

    func testConfigurationSaveIsVersionedBackedUpAndRejectsInvalidReplacement() throws {
        let directory = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString)
        try FileManager.default.createDirectory(at: directory, withIntermediateDirectories: true)
        defer { try? FileManager.default.removeItem(at: directory) }
        let url = directory.appendingPathComponent("config.json")
        let store = ConfigurationStore(fileURL: url)
        let first = TransitConfiguration(storage: .init(retentionDays: 30))
        let second = TransitConfiguration(storage: .init(retentionDays: 60))

        try store.save(first)
        try store.save(second)

        XCTAssertEqual(try store.load(), second)
        XCTAssertEqual(
            try ConfigurationStore(fileURL: url.appendingPathExtension("backup")).load(),
            first
        )

        let invalid = TransitConfiguration(storage: .init(retentionDays: 0))
        XCTAssertThrowsError(try store.save(invalid))
        XCTAssertEqual(try store.load(), second)
    }

    func testPricingSeparatesCachedInput() {
        let policy = PricingPolicy(
            id: "price",
            version: "1",
            currency: "USD",
            rules: [PricingRule(
                modelPattern: "model-*",
                inputPerMillion: 2,
                cachedInputPerMillion: 0.5,
                outputPerMillion: 4
            )]
        )
        let result = PricingEngine.estimate(
            usage: NormalizedUsage(inputTokens: 1_000_000, outputTokens: 500_000, cachedInputTokens: 400_000),
            model: "model-a",
            policy: policy
        )
        XCTAssertEqual(result?.amount, Decimal(string: "3.4"))
    }

    func testPricingDoesNotInventCostWithoutPolicyOrMatchingModel() {
        let usage = NormalizedUsage(inputTokens: 10, outputTokens: 5)
        let policy = PricingPolicy(
            id: "price",
            version: "1",
            currency: "USD",
            rules: [PricingRule(modelPattern: "other-*", inputPerMillion: 1, outputPerMillion: 1)]
        )

        XCTAssertNil(PricingEngine.estimate(usage: usage, model: "model-a", policy: nil))
        XCTAssertNil(PricingEngine.estimate(usage: usage, model: "model-a", policy: policy))
    }

    func testRejectsReservedCredentialHeaderAndNegativePrice() {
        let route = RouteConfiguration(
            displayName: "Route",
            agentID: "agent",
            upstream: "https://llm.example.com/v1",
            authentication: AuthenticationPolicy(
                mode: .replaceHeader,
                secretRef: "secret",
                headerName: "Content-Length"
            ),
            pricingPolicyID: "price"
        )
        let policy = PricingPolicy(
            id: "price",
            version: "1",
            currency: "USD",
            rules: [PricingRule(modelPattern: "*", inputPerMillion: -1, outputPerMillion: 1)]
        )
        let codes = Set(ConfigurationValidator.errors(
            in: TransitConfiguration(routes: [route], pricingPolicies: [policy])
        ).map(\.code))
        XCTAssertTrue(codes.contains("reserved_auth_header"))
        XCTAssertTrue(codes.contains("negative_price"))
    }

    func testKeychainStoreRoundTripAndFingerprint() throws {
        let store = KeychainSecretStore(service: "com.aarontianqx.transit.tests.\(UUID().uuidString)")
        defer {
            try? store.delete(reference: "value")
            try? store.delete(reference: "install-hmac-key")
        }
        try store.save("sensitive-value", reference: "value")
        XCTAssertEqual(try store.read(reference: "value"), "sensitive-value")
        XCTAssertEqual(try store.listReferences(), ["value"])
        let first = try store.fingerprint(secret: "sensitive-value")
        let second = try store.fingerprint(secret: "sensitive-value")
        XCTAssertEqual(first, second)
        XCTAssertFalse(first.contains("sensitive-value"))
        XCTAssertEqual(try store.listReferences(), ["install-hmac-key", "value"])
        try store.delete(reference: "value")
        XCTAssertThrowsError(try store.read(reference: "value"))
    }

    func testMissingKeychainReferenceInvalidatesOnlyAffectedEnabledRoute() {
        let valid = RouteConfiguration(
            id: "valid",
            displayName: "Valid",
            agentID: "agent",
            listener: .init(port: 8787, pathPrefix: "/valid"),
            upstream: "https://valid.example.com",
            authentication: .init(mode: .replaceBearer, secretRef: "available")
        )
        let missing = RouteConfiguration(
            id: "missing",
            displayName: "Missing",
            agentID: "agent",
            listener: .init(port: 8787, pathPrefix: "/missing"),
            upstream: "https://missing.example.com",
            authentication: .init(mode: .replaceBearer, secretRef: "missing")
        )
        var disabled = missing
        disabled.id = "disabled"
        disabled.listener.pathPrefix = "/disabled"
        disabled.enabled = false

        let issues = ConfigurationValidator.validateSecretReferences(
            TransitConfiguration(routes: [valid, missing, disabled]),
            using: FixtureSecretStore(availableReferences: ["available"])
        )

        XCTAssertEqual(issues.map(\.routeID), ["missing"])
        XCTAssertEqual(issues.map(\.code), ["secret_not_found"])
    }
}

private struct FixtureSecretStore: SecretStore {
    let availableReferences: Set<String>

    func read(reference: String) throws -> String {
        guard availableReferences.contains(reference) else { throw SecretStoreError.notFound(reference) }
        return "fixture-secret"
    }

    func save(_ value: String, reference: String) throws {}
    func delete(reference: String) throws {}
}
