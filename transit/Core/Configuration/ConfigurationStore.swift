import Foundation

public struct ConfigurationStore: Sendable {
    public let fileURL: URL

    public init(fileURL: URL = ConfigurationStore.defaultFileURL) {
        self.fileURL = fileURL
    }

    public static var defaultFileURL: URL {
        let root = FileManager.default.urls(for: .applicationSupportDirectory, in: .userDomainMask)[0]
        return root.appendingPathComponent("Transit/config.json")
    }

    public func load() throws -> TransitConfiguration {
        guard FileManager.default.fileExists(atPath: fileURL.path) else {
            return TransitConfiguration()
        }
        let data = try Data(contentsOf: fileURL)
        try StrictConfigurationKeys.validate(data)
        let decoder = JSONDecoder()
        return try decoder.decode(TransitConfiguration.self, from: data)
    }

    public func save(_ configuration: TransitConfiguration) throws {
        let errors = ConfigurationValidator.errors(in: configuration)
        guard errors.isEmpty else { throw ConfigurationStoreError.invalidConfiguration(errors) }
        try FileManager.default.createDirectory(
            at: fileURL.deletingLastPathComponent(),
            withIntermediateDirectories: true
        )
        if FileManager.default.fileExists(atPath: fileURL.path) {
            let backup = fileURL.appendingPathExtension("backup")
            try? FileManager.default.removeItem(at: backup)
            try FileManager.default.copyItem(at: fileURL, to: backup)
        }
        let encoder = JSONEncoder()
        encoder.outputFormatting = [.prettyPrinted, .sortedKeys]
        try encoder.encode(configuration).write(to: fileURL, options: .atomic)
    }
}
public enum ConfigurationStoreError: LocalizedError {
    case invalidConfiguration([ConfigurationIssue])
    case unknownKey(path: String)

    public var errorDescription: String? {
        switch self {
        case .invalidConfiguration(let issues):
            issues.map(\.message).joined(separator: "\n")
        case .unknownKey(let path):
            "Unknown configuration key: \(path)"
        }
    }
}

private enum StrictConfigurationKeys {
    static func validate(_ data: Data) throws {
        let root = try JSONSerialization.jsonObject(with: data)
        guard let object = root as? [String: Any] else { return }
        try check(object, allowed: ["version", "routes", "pricing_policies", "storage"], path: "")

        if let storage = object["storage"] as? [String: Any] {
            try check(storage, allowed: ["retention_days"], path: "storage")
        }
        for (index, value) in (object["routes"] as? [Any] ?? []).enumerated() {
            guard let route = value as? [String: Any] else { continue }
            let path = "routes[\(index)]"
            try check(route, allowed: [
                "id", "display_name", "agent_id", "listener", "upstream", "protocol",
                "auth", "pricing_policy_id", "enabled", "allow_insecure_http",
            ], path: path)
            if let listener = route["listener"] as? [String: Any] {
                try check(listener, allowed: ["port", "path_prefix"], path: "\(path).listener")
            }
            if let auth = route["auth"] as? [String: Any] {
                try check(auth, allowed: ["mode", "secret_ref", "header_name"], path: "\(path).auth")
            }
        }
        for (index, value) in (object["pricing_policies"] as? [Any] ?? []).enumerated() {
            guard let policy = value as? [String: Any] else { continue }
            let path = "pricing_policies[\(index)]"
            try check(policy, allowed: ["id", "version", "currency", "rules"], path: path)
            for (ruleIndex, ruleValue) in (policy["rules"] as? [Any] ?? []).enumerated() {
                guard let rule = ruleValue as? [String: Any] else { continue }
                try check(rule, allowed: [
                    "model_pattern", "input_per_million", "cached_input_per_million",
                    "output_per_million",
                ], path: "\(path).rules[\(ruleIndex)]")
            }
        }
    }

    private static func check(_ object: [String: Any], allowed: Set<String>, path: String) throws {
        if let unknown = object.keys.first(where: { !allowed.contains($0) }) {
            let fullPath = path.isEmpty ? unknown : "\(path).\(unknown)"
            throw ConfigurationStoreError.unknownKey(path: fullPath)
        }
    }
}
