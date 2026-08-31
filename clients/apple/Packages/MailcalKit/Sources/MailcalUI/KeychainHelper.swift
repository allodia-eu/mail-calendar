// Secure credential storage for Apple platforms: every account's config (TOML, including the password
// for IMAP or the refresh token for Microsoft 365) is kept in the Keychain, the OS secure
// store, rather than a plaintext file on disk.
//
// Layout: ONE generic-password item per account (account "account:<id>"), plus a small index
// item ("account-index") holding the ordered ids, so the switcher keeps add-order and an
// account can be added, replaced, or removed on its own. This is the per-account model behind
// account management (removing an individual account), matching Windows' Credential Manager
// store (../windows/Mailcal/Services/CredentialStore.cs) and Android's
// EncryptedSharedPreferences (../android/.../SecureStore.kt). No size chunking here, unlike
// Windows' 2560-byte per-credential cap, the Keychain holds a large refresh-token config in one
// item. Split out so the Security-framework glue stays isolated.

import Foundation
import MailcalBindings
import Security

/// Reads/writes the configured accounts in the Apple Keychain, one generic-password item per
/// account under an ordered index, so adding, replacing, or removing one never disturbs the
/// others.
enum KeychainStore {
    // Production, or, in a DEBUG build, the isolated dev namespace that replaces it. The
    // decision (and the reasoning) lives in `DevNamespace`, shared with the engine store directory
    // and the preference domain so the three cannot drift apart.
    private static let service = DevNamespace.currentKeychainService
    // The ordered list of account ids; each account's config lives under itemAccount(id).
    private static let indexAccount = "account-index"

    // macOS has two keychains: the ACL-scoped file (login) keychain and the iOS-style,
    // access-group-scoped DATA-PROTECTION keychain. Use the data-protection keychain whenever this
    // build carries a keychain-access-group entitlement, the Mac App Store build (its Mac App Store
    // provisioning profile grants the group) and iOS, so credential storage matches iOS and is
    // prompt-free (the file keychain re-prompts when the code signature that owns an item changes).
    // The Developer ID .dmg and the ad-hoc dev build have NO access group, so the data-protection
    // keychain would reject every SecItem call with errSecMissingEntitlement; they stay on the file
    // keychain, unchanged. We read our OWN entitlements to decide, so one binary does the right
    // thing under each signing/distribution with no build flag. (On the file-keychain path this
    // flag is false, identical to omitting it, macOS defaults to the file keychain.)
    #if os(macOS)
    private static let useDataProtectionKeychain: Bool = {
        guard let task = SecTaskCreateFromSelf(nil),
            let groups = SecTaskCopyValueForEntitlement(
                task, "keychain-access-groups" as CFString, nil) as? [String]
        else { return false }
        return !groups.isEmpty
    }()
    #else
    // iOS/iPadOS have only the data-protection keychain.
    private static let useDataProtectionKeychain = true
    #endif

    private static func itemAccount(_ id: String) -> String { "account:\(id)" }

    /// Every stored account's config TOML, in the order they were added (empty on first run).
    /// The host passes these to `MailcalApp.newAccounts`, which re-derives each id. An indexed
    /// id whose item is missing is skipped rather than failing the launch.
    static func configs() -> [String] {
        readIndex().compactMap { id in
            readData(account: itemAccount(id)).flatMap { String(data: $0, encoding: .utf8) }
        }
    }

    /// Stores `config` under account `id` in its own item, replacing that account's entry (a
    /// reconnect / rotated token) and appending the id to the ordered index on first add, so
    /// the switcher stays stable. Returns whether the config and the index both landed.
    ///
    /// It used to return nothing and drop every `OSStatus` on the floor, which was defensible
    /// only while the caller had nothing to do with the answer. The core now decides what a
    /// refused write means, rolling an add back, or reporting a rotation it cannot recover
    /// from, so a silent failure here would be a `nil` where a reason belongs.
    @discardableResult
    static func save(id: String, config: String) -> Bool {
        guard let data = config.data(using: .utf8) else { return false }
        guard writeData(account: itemAccount(id), data) else { return false }
        var ids = readIndex()
        if !ids.contains(id) {
            ids.append(id)
            return writeIndex(ids)
        }
        return true
    }

    /// Removes account `id`: deletes its item and drops it from the ordered index, so a later
    /// launch no longer loads it. Returns whether nothing is stored for `id` any more, an id
    /// that was not there is a success, since that is already the desired end state. The
    /// account's runtime removal is the core's job (`MailcalApp.removeAccount`), which calls
    /// this itself.
    @discardableResult
    static func remove(id: String) -> Bool {
        guard deleteItem(account: itemAccount(id)) else { return false }
        return writeIndex(readIndex().filter { $0 != id })
    }

    private static func readIndex() -> [String] {
        guard let data = readData(account: indexAccount),
            let ids = try? JSONDecoder().decode([String].self, from: data)
        else { return [] }
        return ids
    }

    private static func writeIndex(_ ids: [String]) -> Bool {
        guard let data = try? JSONEncoder().encode(ids) else { return false }
        return writeData(account: indexAccount, data)
    }

    // --- Security-framework glue over one generic-password item, keyed by `account` ----------

    private static func readData(account: String) -> Data? {
        let query: [String: Any] = [
            kSecClass as String: kSecClassGenericPassword,
            kSecAttrService as String: service,
            kSecAttrAccount as String: account,
            kSecReturnData as String: true,
            kSecMatchLimit as String: kSecMatchLimitOne,
            kSecUseDataProtectionKeychain as String: useDataProtectionKeychain,
        ]
        var item: CFTypeRef?
        guard SecItemCopyMatching(query as CFDictionary, &item) == errSecSuccess,
            let data = item as? Data
        else { return nil }
        return data
    }

    private static func writeData(account: String, _ data: Data) -> Bool {
        let identity: [String: Any] = [
            kSecClass as String: kSecClassGenericPassword,
            kSecAttrService as String: service,
            kSecAttrAccount as String: account,
            kSecUseDataProtectionKeychain as String: useDataProtectionKeychain,
        ]
        // Update in place when the item already exists. Delete-then-add changes the item's
        // Keychain ACL owner to the current code signature, which is hostile to local dev builds:
        // the app can start prompting for the Keychain password again after every rebuild.
        let update: [String: Any] = [kSecValueData as String: data]
        let status = SecItemUpdate(identity as CFDictionary, update as CFDictionary)
        guard status == errSecItemNotFound else { return status == errSecSuccess }

        var add = identity
        add[kSecValueData as String] = data
        add[kSecAttrAccessible as String] = kSecAttrAccessibleAfterFirstUnlock
        // No custom `kSecAttrAccess` here, deliberately. An "any application" ACL (a nil
        // trusted-application list, as `security add-generic-password -A` writes) looks like it
        // would stop a rebuilt dev build re-prompting, but it does NOT: since Sierra the file
        // keychain also gates access on a **partition id**, and the item's partition is the
        // creating binary's `cdhash:` when it is ad-hoc signed. Measured, an item written that
        // way is unreadable without a prompt by a second binary, and so is one written by
        // `-A` itself. What actually fixes it is signing the dev build with a persistent
        // certificate, which makes the partition `teamid:` and the ACL entry cert-based, both
        // stable across rebuilds; `Scripts/build-and-run.sh` does that.
        return SecItemAdd(add as CFDictionary, nil) == errSecSuccess
    }

    private static func deleteItem(account: String) -> Bool {
        let identity: [String: Any] = [
            kSecClass as String: kSecClassGenericPassword,
            kSecAttrService as String: service,
            kSecAttrAccount as String: account,
            kSecUseDataProtectionKeychain as String: useDataProtectionKeychain,
        ]
        // `errSecItemNotFound` is the end state this was asked for, so it is a success.
        let status = SecItemDelete(identity as CFDictionary)
        return status == errSecSuccess || status == errSecItemNotFound
    }
}
