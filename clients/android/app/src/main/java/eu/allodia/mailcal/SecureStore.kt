// The OS-backed secure store for the Android client: every account's config (endpoints +
// credentials, the IMAP password or the Microsoft refresh token) is held in an [AccountVault]
// sealed under a key in the Android Keystore, not a plaintext file.
//
// The per-account model behind account management (removing an individual account), matching
// Windows' Credential Manager store (../../windows/Mailcal/Services/CredentialStore.cs) and
// macOS's Keychain (../../macos/KeychainHelper.swift). The host reads them all on launch and hands
// the configs to `MailcalApp.newAccounts`, appends a new one via `save` after `addAccount`
// connects, and drops one via `remove`.
package eu.allodia.mailcal

import android.content.Context

// Synchronised because the core persists a rotated token from its own threads while the activity
// or the background worker may be reading, and every write is a read, a change and a write back.
internal object SecureStore {
    private const val VAULT_FILE = "mailcal-accounts"

    // Every stored account's config TOML, in the order they were added (empty on first run).
    @Synchronized
    fun configs(context: Context): List<String> = vault(context).accounts().map { it.config }

    // Stores [config] under account [id], replacing any existing one (a reconnect after a
    // credential change) and appending it on first add, so the switcher stays stable.
    @Synchronized
    fun save(context: Context, id: String, config: String) = vault(context).save(id, config)

    // Removes account [id], so a later launch no longer loads it. The account's runtime removal
    // is the core's job (`MailcalApp.removeAccount`).
    @Synchronized
    fun remove(context: Context, id: String) = vault(context).remove(id)

    private fun vault(context: Context) = AccountVault(
        prefs = context.getSharedPreferences(VAULT_FILE, Context.MODE_PRIVATE),
        sealer = KeystoreSealer,
        legacy = LegacySecureStore(context),
        onDiscard = { reason -> logUiWarn("secure store: discarded an unreadable account vault: $reason") },
    )
}
