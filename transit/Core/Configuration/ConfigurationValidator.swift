import Foundation

public struct ConfigurationIssue: Equatable, Identifiable, Sendable {
    public enum Severity: String, Sendable {
        case error
        case warning
    }

    public let id: String
    public let severity: Severity
    public let code: String
    public let path: String
    public let message: String
    public let routeID: String?

    public init(
        severity: Severity = .error,
        code: String,
        path: String,
        message: String,
        routeID: String? = nil
    ) {
        self.id = "\(path):\(code)"
        self.severity = severity
        self.code = code
        self.path = path
        self.message = message
        self.routeID = routeID
    }
}

public enum ConfigurationValidator {
    public static func validate(_ configuration: TransitConfiguration) -> [ConfigurationIssue] {
        var issues: [ConfigurationIssue] = []
        if configuration.version != TransitConfiguration.currentVersion {
            issues.append(.init(
                code: "unsupported_version",
                path: "version",
                message: "Unsupported configuration version \(configuration.version)."
            ))
        }
        if !(1...3650).contains(configuration.storage.retentionDays) {
            issues.append(.init(
                code: "invalid_retention",
                path: "storage.retention_days",
                message: "Retention must be between 1 and 3650 days."
            ))
        }

        issues += duplicateIssues(
            values: configuration.routes.map(\.id),
            path: "routes",
            code: "duplicate_route_id"
        )
        issues += duplicateIssues(
            values: configuration.pricingPolicies.map(\.id),
            path: "pricing_policies",
            code: "duplicate_pricing_policy_id"
        )

        let pricingIDs = Set(configuration.pricingPolicies.map(\.id))
        for (index, route) in configuration.routes.enumerated() {
            issues += validate(route, index: index, pricingIDs: pricingIDs)
        }
        issues += validateListenerConflicts(configuration.routes)

        for (index, policy) in configuration.pricingPolicies.enumerated() {
            let prefix = "pricing_policies[\(index)]"
            if policy.id.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty {
                issues.append(.init(code: "empty_id", path: "\(prefix).id", message: "Pricing policy ID is required."))
            }
            if policy.currency.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty {
                issues.append(.init(code: "empty_currency", path: "\(prefix).currency", message: "Currency is required."))
            }
            if policy.version.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty {
                issues.append(.init(code: "empty_version", path: "\(prefix).version", message: "Pricing policy version is required."))
            }
            if policy.rules.isEmpty {
                issues.append(.init(code: "empty_rules", path: "\(prefix).rules", message: "At least one pricing rule is required."))
            }
            for (ruleIndex, rule) in policy.rules.enumerated() where rule.modelPattern.isEmpty {
                issues.append(.init(
                    code: "empty_model_pattern",
                    path: "\(prefix).rules[\(ruleIndex)].model_pattern",
                    message: "Model pattern is required."
                ))
            }
            for (ruleIndex, rule) in policy.rules.enumerated()
            where rule.inputPerMillion < 0 || rule.cachedInputPerMillion < 0 || rule.outputPerMillion < 0 {
                issues.append(.init(
                    code: "negative_price",
                    path: "\(prefix).rules[\(ruleIndex)]",
                    message: "Prices cannot be negative."
                ))
            }
        }
        return issues
    }

    public static func errors(in configuration: TransitConfiguration) -> [ConfigurationIssue] {
        validate(configuration).filter { $0.severity == .error }
    }

    public static func validateSecretReferences(
        _ configuration: TransitConfiguration,
        using secretStore: any SecretStore
    ) -> [ConfigurationIssue] {
        configuration.routes.enumerated().compactMap { index, route in
            guard route.enabled,
                  route.authentication.mode != .passthrough,
                  let reference = route.authentication.secretRef,
                  !reference.isEmpty
            else { return nil }

            do {
                _ = try secretStore.read(reference: reference)
                return nil
            } catch let error as SecretStoreError {
                let code: String
                switch error {
                case .notFound:
                    code = "secret_not_found"
                case .invalidReference, .keychain:
                    code = "secret_unavailable"
                }
                return ConfigurationIssue(
                    code: code,
                    path: "routes[\(index)].auth.secret_ref",
                    message: error.localizedDescription,
                    routeID: route.id
                )
            } catch {
                return ConfigurationIssue(
                    code: "secret_unavailable",
                    path: "routes[\(index)].auth.secret_ref",
                    message: "Credential is unavailable: \(error.localizedDescription)",
                    routeID: route.id
                )
            }
        }
    }

    private static func validate(
        _ route: RouteConfiguration,
        index: Int,
        pricingIDs: Set<String>
    ) -> [ConfigurationIssue] {
        var issues: [ConfigurationIssue] = []
        let prefix = "routes[\(index)]"
        func issue(_ code: String, _ field: String, _ message: String) -> ConfigurationIssue {
            .init(code: code, path: "\(prefix).\(field)", message: message, routeID: route.id)
        }

        if route.id.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty {
            issues.append(issue("empty_id", "id", "Route ID is required."))
        }
        if route.displayName.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty {
            issues.append(issue("empty_display_name", "display_name", "Display name is required."))
        }
        if route.agentID.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty {
            issues.append(issue("empty_agent_id", "agent_id", "Agent label is required."))
        }
        if !(1...65535).contains(route.listener.port) {
            issues.append(issue("invalid_port", "listener.port", "Port must be between 1 and 65535."))
        }
        if !isValidPrefix(route.listener.pathPrefix) {
            issues.append(issue(
                "invalid_path_prefix",
                "listener.path_prefix",
                "Path prefix must start with /, contain no query or fragment, and have no trailing slash unless it is /."
            ))
        }

        if let url = URL(string: route.upstream), let components = URLComponents(url: url, resolvingAgainstBaseURL: false),
           let scheme = components.scheme?.lowercased(), components.host != nil, components.url != nil {
            if components.user != nil || components.password != nil {
                issues.append(issue("embedded_credentials", "upstream", "Upstream URL must not contain credentials."))
            }
            if scheme == "http" && !route.allowInsecureHTTP {
                issues.append(issue("insecure_upstream_not_allowed", "upstream", "HTTP upstream requires explicit confirmation."))
            } else if scheme != "https" && scheme != "http" {
                issues.append(issue("invalid_upstream_scheme", "upstream", "Upstream scheme must be https or explicitly allowed http."))
            }
            if components.query != nil || components.fragment != nil {
                issues.append(issue(
                    "upstream_has_query_or_fragment",
                    "upstream",
                    "Upstream base URL must not contain a query or fragment."
                ))
            }
        } else {
            issues.append(issue("invalid_upstream", "upstream", "Upstream must be an absolute URL with a host."))
        }

        switch route.authentication.mode {
        case .passthrough:
            break
        case .replaceBearer:
            if route.authentication.secretRef?.isEmpty != false {
                issues.append(issue("missing_secret_ref", "auth.secret_ref", "Bearer replacement requires a secret reference."))
            }
        case .replaceHeader:
            if route.authentication.secretRef?.isEmpty != false {
                issues.append(issue("missing_secret_ref", "auth.secret_ref", "Header replacement requires a secret reference."))
            }
            if !isValidHeaderName(route.authentication.headerName) {
                issues.append(issue("invalid_header_name", "auth.header_name", "A valid header name is required."))
            } else if let header = route.authentication.headerName?.lowercased(), reservedHeaders.contains(header) {
                issues.append(issue(
                    "reserved_auth_header",
                    "auth.header_name",
                    "This transport header cannot be used for credential injection."
                ))
            }
        }

        if let pricingID = route.pricingPolicyID, !pricingIDs.contains(pricingID) {
            issues.append(issue("unknown_pricing_policy", "pricing_policy_id", "Referenced pricing policy does not exist."))
        }
        return issues
    }

    private static func validateListenerConflicts(_ routes: [RouteConfiguration]) -> [ConfigurationIssue] {
        var seen: [String: String] = [:]
        var issues: [ConfigurationIssue] = []
        for (index, route) in routes.enumerated() where route.enabled {
            let key = "\(route.listener.port):\(route.listener.pathPrefix.lowercased())"
            if let existing = seen[key] {
                issues.append(.init(
                    code: "duplicate_listener_match",
                    path: "routes[\(index)].listener",
                    message: "The listener port and path prefix duplicate route \(existing).",
                    routeID: route.id
                ))
            } else {
                seen[key] = route.id
            }
        }
        return issues
    }

    private static func duplicateIssues(values: [String], path: String, code: String) -> [ConfigurationIssue] {
        var seen: Set<String> = []
        var duplicates: Set<String> = []
        for value in values where !seen.insert(value).inserted { duplicates.insert(value) }
        return duplicates.sorted().map {
            .init(code: code, path: path, message: "Duplicate ID: \($0)")
        }
    }

    private static func isValidPrefix(_ prefix: String) -> Bool {
        guard prefix.first == "/", !prefix.contains("?"), !prefix.contains("#") else { return false }
        return prefix == "/" || !prefix.hasSuffix("/")
    }

    private static func isValidHeaderName(_ name: String?) -> Bool {
        guard let name, !name.isEmpty else { return false }
        let allowed = CharacterSet(charactersIn: "!#$%&'*+-.^_`|~0123456789abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ")
        return name.unicodeScalars.allSatisfy(allowed.contains)
    }

    private static let reservedHeaders: Set<String> = [
        "host", "content-length", "connection", "transfer-encoding", "accept-encoding",
        "keep-alive", "upgrade", "trailer", "te", "proxy-authenticate", "proxy-authorization",
    ]
}
