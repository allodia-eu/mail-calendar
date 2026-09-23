// Every stored account's config (endpoints and credentials: an IMAP password, a refresh token) as
// one sealed document: a JSON array of `{id, config}` in the order the accounts were added, so the
// switcher keeps add-order. Sealing is the [Sealer]'s job, which in the app is the Android Keystore
// ([KeystoreSealer]); keeping it behind an interface is what lets the JVM suite, which has no
// Keystore, pin the layout and the migration.
//
// One document rather than one entry per account, so the account ids are sealed along with the
// configs rather than readable as preference keys, and a save or a removal is a single write.
package eu.allodia.mailcal

import android.content.SharedPreferences
import java.io.IOException
import java.util.Base64
import org.json.JSONArray
import org.json.JSONObject

internal data class StoredAccount(val id: String, val config: String)

internal interface Sealer {
    fun seal(plaintext: ByteArray, associatedData: ByteArray): ByteArray

    /** Throws [VaultUnreadable] when [sealed] can never be opened again, anything else when it might. */
    fun open(sealed: ByteArray, associatedData: ByteArray): ByteArray
}

/**
 * The vault cannot be opened on this device, now or later: its key is gone, as it is on a device
 * restored from a backup (the Keystore is never backed up), or it no longer matches the data.
 */
internal class VaultUnreadable(reason: String) : Exception(reason)

/** The store this app kept accounts in before the vault, read once and then deleted. */
internal interface LegacyAccounts {
    fun exists(): Boolean

    fun read(): List<StoredAccount>

    fun delete()
}

internal class AccountVault(
    private val prefs: SharedPreferences,
    private val sealer: Sealer,
    private val legacy: LegacyAccounts,
    private val onDiscard: (reason: String) -> Unit,
) {
    fun accounts(): List<StoredAccount> {
        migrate()
        val stored = prefs.getString(KEY_VAULT, null) ?: return emptyList()
        val plaintext = try {
            sealer.open(Base64.getDecoder().decode(stored), ASSOCIATED_DATA)
        } catch (e: VaultUnreadable) {
            // Nothing can open it again, so keeping it only means failing the same way at every
            // launch. The user adds their accounts again.
            onDiscard(e.message ?: "unreadable")
            commit(prefs.edit().remove(KEY_VAULT))
            return emptyList()
        }
        val array = JSONArray(plaintext.toString(Charsets.UTF_8))
        return (0 until array.length()).map { i ->
            val entry = array.getJSONObject(i)
            StoredAccount(entry.getString("id"), entry.getString("config"))
        }
    }

    /** Stores [config] under [id], replacing it in place when the account is already stored. */
    fun save(id: String, config: String) {
        val current = accounts()
        val replaced = current.map { if (it.id == id) StoredAccount(id, config) else it }
        write(if (current.any { it.id == id }) replaced else current + StoredAccount(id, config))
    }

    fun remove(id: String) {
        write(accounts().filterNot { it.id == id })
    }

    // Moves the legacy store's accounts in when the vault has none yet, then deletes it. A legacy
    // store that cannot be read throws and is left in place, so a transient failure costs a launch
    // rather than the user's accounts.
    private fun migrate() {
        if (!legacy.exists()) return
        if (!prefs.contains(KEY_VAULT)) write(legacy.read())
        legacy.delete()
    }

    private fun write(accounts: List<StoredAccount>) {
        val array = JSONArray()
        accounts.forEach { array.put(JSONObject().put("id", it.id).put("config", it.config)) }
        val sealed = sealer.seal(array.toString().toByteArray(Charsets.UTF_8), ASSOCIATED_DATA)
        commit(prefs.edit().putString(KEY_VAULT, Base64.getEncoder().encodeToString(sealed)))
    }

    // `commit`, not `apply`: the caller reports this write to the core as done, and a migration
    // deletes the legacy store only once the vault is on disk.
    private fun commit(editor: SharedPreferences.Editor) {
        if (!editor.commit()) throw IOException("the account vault could not be written")
    }

    private companion object {
        const val KEY_VAULT = "vault"

        // Binds the ciphertext to what it is, so a sealed value cannot be passed off as another.
        val ASSOCIATED_DATA = "mailcal-account-vault/1".toByteArray(Charsets.UTF_8)
    }
}
