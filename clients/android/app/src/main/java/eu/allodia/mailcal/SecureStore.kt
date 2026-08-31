// The OS-backed secure store for the Android client: every account's config (endpoints +
// credentials, the IMAP password or the Microsoft refresh token) is held in
// EncryptedSharedPreferences over an AES256-GCM master key in the Android Keystore, not a
// plaintext file.
//
// Layout: ONE entry per account (key "account:<id>"), plus a small index entry ("account-index")
// holding the ordered ids, so the switcher keeps add-order and an account can be added,
// replaced, or removed on its own. This is the per-account model behind account management
// (removing an individual account), matching Windows' Credential Manager store
// (../../windows/Mailcal/Services/CredentialStore.cs) and macOS's Keychain
// (../../macos/KeychainHelper.swift). The host reads them all on launch and hands the configs to
// `MailcalApp.newAccounts`, appends a new one via `save` after `addAccount` connects, and drops
// one via `remove`.
package eu.allodia.mailcal

import android.content.Context
import androidx.security.crypto.EncryptedSharedPreferences
import androidx.security.crypto.MasterKeys
import org.json.JSONArray

// A tiny wrapper around EncryptedSharedPreferences holding one entry per account under an ordered
// index. security-crypto 1.0.0 exposes the AES256-GCM master key via `MasterKeys` (the later
// `MasterKey.Builder` API only lands in 1.1.0); the key alias is generated/fetched from the
// Android Keystore and reused across launches.
internal object SecureStore {
    private const val PREFS_FILE = "mailcal-secure"
    // The ordered list of account ids; each account's config lives under accountKey(id).
    private const val KEY_INDEX = "account-index"

    private fun accountKey(id: String) = "account:$id"

    // Every stored account's config TOML, in the order they were added (empty on first run). An
    // indexed id whose entry is missing is skipped rather than failing the launch.
    fun configs(context: Context): List<String> =
        readIndex(context).mapNotNull { id -> prefs(context).getString(accountKey(id), null) }

    // Stores [config] under account [id] in its own entry, replacing any existing one (a
    // reconnect after a credential change) and appending the id to the ordered index on first
    // add, so the switcher stays stable.
    fun save(context: Context, id: String, config: String) {
        val editor = prefs(context).edit().putString(accountKey(id), config)
        val ids = readIndex(context)
        if (!ids.contains(id)) {
            editor.putString(KEY_INDEX, JSONArray(ids + id).toString())
        }
        editor.apply()
    }

    // Removes account [id]: drops its entry and the id from the ordered index, so a later launch
    // no longer loads it. The account's runtime removal is the core's job (`MailcalApp.removeAccount`).
    fun remove(context: Context, id: String) {
        val ids = readIndex(context).filter { it != id }
        prefs(context).edit()
            .remove(accountKey(id))
            .putString(KEY_INDEX, JSONArray(ids).toString())
            .apply()
    }

    // The ordered account ids, or empty if nothing is stored or the value can't be parsed.
    private fun readIndex(context: Context): List<String> {
        val raw = prefs(context).getString(KEY_INDEX, null) ?: return emptyList()
        return runCatching {
            val array = JSONArray(raw)
            (0 until array.length()).map { array.getString(it) }
        }.getOrDefault(emptyList())
    }

    // The EncryptedSharedPreferences instance: AES256_SIV for keys, AES256_GCM for values, under
    // an AES256-GCM master key held in the Android Keystore.
    private fun prefs(context: Context) = EncryptedSharedPreferences.create(
        PREFS_FILE,
        MasterKeys.getOrCreate(MasterKeys.AES256_GCM_SPEC),
        context,
        EncryptedSharedPreferences.PrefKeyEncryptionScheme.AES256_SIV,
        EncryptedSharedPreferences.PrefValueEncryptionScheme.AES256_GCM,
    )
}
