// The store accounts were kept in before [AccountVault]: EncryptedSharedPreferences over a
// `MasterKeys` key, from androidx.security-crypto, which deprecates the whole library. It is read
// once, to move its accounts into the vault, and then deleted. Suppressed for this file alone, so
// the rest of the module still fails on a deprecation.
@file:Suppress("DEPRECATION")

package eu.allodia.mailcal

import android.content.Context
import androidx.security.crypto.EncryptedSharedPreferences
import androidx.security.crypto.MasterKeys
import java.io.File
import org.json.JSONArray

internal class LegacySecureStore(context: Context) : LegacyAccounts {
    private val appContext = context.applicationContext

    // Checked on disk: opening it through SharedPreferences would cache it in the process, and
    // the cached copy outlives the delete below.
    override fun exists(): Boolean = File(appContext.dataDir, "shared_prefs/$PREFS_FILE.xml").exists()

    // One entry per account under "account:<id>", ordered by the JSON id array under
    // "account-index". An indexed id whose entry is missing is skipped.
    override fun read(): List<StoredAccount> {
        val prefs = EncryptedSharedPreferences.create(
            PREFS_FILE,
            MasterKeys.getOrCreate(MasterKeys.AES256_GCM_SPEC),
            appContext,
            EncryptedSharedPreferences.PrefKeyEncryptionScheme.AES256_SIV,
            EncryptedSharedPreferences.PrefValueEncryptionScheme.AES256_GCM,
        )
        val raw = prefs.getString("account-index", null) ?: return emptyList()
        val ids = runCatching {
            val array = JSONArray(raw)
            (0 until array.length()).map { array.getString(it) }
        }.getOrDefault(emptyList())
        return ids.mapNotNull { id -> prefs.getString("account:$id", null)?.let { StoredAccount(id, it) } }
    }

    // The file also holds the Tink keysets the entries were encrypted under, so this takes them too.
    override fun delete() {
        appContext.deleteSharedPreferences(PREFS_FILE)
    }

    private companion object {
        const val PREFS_FILE = "mailcal-secure"
    }
}
