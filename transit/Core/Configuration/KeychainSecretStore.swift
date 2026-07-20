import CryptoKit
import Foundation
import Security

public protocol SecretStore: Sendable {
    func read(reference: String) throws -> String
    func save(_ value: String, reference: String) throws
    func delete(reference: String) throws
}

public struct KeychainSecretStore: SecretStore, Sendable {
    public let service: String

    public init(service: String = "com.aarontianqx.transit.secrets") {
        self.service = service
    }

    public func read(reference: String) throws -> String {
        var query = baseQuery(reference: reference)
        query[kSecReturnData as String] = true
        query[kSecMatchLimit as String] = kSecMatchLimitOne
        var result: CFTypeRef?
        let status = SecItemCopyMatching(query as CFDictionary, &result)
        guard status != errSecItemNotFound else { throw SecretStoreError.notFound(reference) }
        guard status == errSecSuccess, let data = result as? Data,
              let value = String(data: data, encoding: .utf8)
        else { throw SecretStoreError.keychain(status) }
        return value
    }

    public func save(_ value: String, reference: String) throws {
        guard !reference.isEmpty else { throw SecretStoreError.invalidReference }
        let data = Data(value.utf8)
        let query = baseQuery(reference: reference)
        let status = SecItemUpdate(
            query as CFDictionary,
            [kSecValueData as String: data] as CFDictionary
        )
        if status == errSecItemNotFound {
            var insert = query
            insert[kSecValueData as String] = data
            let insertStatus = SecItemAdd(insert as CFDictionary, nil)
            guard insertStatus == errSecSuccess else { throw SecretStoreError.keychain(insertStatus) }
        } else if status != errSecSuccess {
            throw SecretStoreError.keychain(status)
        }
    }

    public func delete(reference: String) throws {
        let status = SecItemDelete(baseQuery(reference: reference) as CFDictionary)
        guard status == errSecSuccess || status == errSecItemNotFound else {
            throw SecretStoreError.keychain(status)
        }
    }

    public func listReferences() throws -> [String] {
        let query: [String: Any] = [
            kSecClass as String: kSecClassGenericPassword,
            kSecAttrService as String: service,
            kSecReturnAttributes as String: true,
            kSecMatchLimit as String: kSecMatchLimitAll,
        ]
        var result: CFTypeRef?
        let status = SecItemCopyMatching(query as CFDictionary, &result)
        guard status != errSecItemNotFound else { return [] }
        guard status == errSecSuccess else { throw SecretStoreError.keychain(status) }

        let items: [[String: Any]]
        if let multiple = result as? [[String: Any]] {
            items = multiple
        } else if let single = result as? [String: Any] {
            items = [single]
        } else {
            items = []
        }
        return items.compactMap { $0[kSecAttrAccount as String] as? String }.sorted()
    }

    public func fingerprint(secret: String, installKeyReference: String = "install-hmac-key") throws -> String {
        let key: String
        if let existing = try? read(reference: installKeyReference) {
            key = existing
        } else {
            key = UUID().uuidString + UUID().uuidString
            try save(key, reference: installKeyReference)
        }
        let authenticationCode = HMAC<SHA256>.authenticationCode(
            for: Data(secret.utf8),
            using: SymmetricKey(data: Data(key.utf8))
        )
        return authenticationCode.prefix(12).map { String(format: "%02x", $0) }.joined()
    }

    private func baseQuery(reference: String) -> [String: Any] {
        [
            kSecClass as String: kSecClassGenericPassword,
            kSecAttrService as String: service,
            kSecAttrAccount as String: reference,
        ]
    }
}

public enum SecretStoreError: LocalizedError {
    case invalidReference
    case notFound(String)
    case keychain(OSStatus)

    public var errorDescription: String? {
        switch self {
        case .invalidReference: "Secret reference cannot be empty."
        case .notFound(let reference): "Secret not found: \(reference)"
        case .keychain(let status):
            SecCopyErrorMessageString(status, nil) as String? ?? "Keychain error \(status)."
        }
    }
}
