// Where this device remembers what it has synced with the Allodia account service. Its Apple,
// Windows and Linux twins keep the same blob in each platform's own ordinary preferences:
// UserDefaults, ApplicationDataContainer and the GSettings-free config file respectively.
//
// SharedPreferences and not the Keystore: nothing in the blob is secret (a record id, a version, a
// fingerprint, a flag), and a Keystore prompt in front of a pass nobody started would be a prompt
// nobody is there to answer.
package eu.allodia.mailcal

import android.content.Context
import uniffi.mailcal_bindings.SyncStateException
import uniffi.mailcal_bindings.SyncStateStore

/**
 * The blob, in one preference, written whole.
 *
 * [file] separates a dev-account launch's bookkeeping from a real one's, the same way the engine
 * store is separated: the two hold different accounts, and a pass that mixed them would offer a
 * harness mailbox to the developer's own phone.
 */
internal class SharedPrefsSyncStateStore(
    context: Context,
    private val file: String,
) : SyncStateStore {
    private val appContext = context.applicationContext

    override fun load(): String? =
        prefs().getString(KEY, null)

    override fun save(blob: String) {
        // `commit`, not `apply`: the core has already written to the service by the time it calls
        // this, and treats a refusal as something to report. `apply` returns before the write
        // happens and can only report success.
        if (!prefs().edit().putString(KEY, blob).commit()) {
            throw SyncStateException.Store("Android refused to store the sync state")
        }
    }

    private fun prefs() = appContext.getSharedPreferences(file, Context.MODE_PRIVATE)

    private companion object {
        const val KEY = "allodia_sync_state"
    }
}

/**
 * Which preferences file this launch's bookkeeping belongs in, or null for a launch that must not
 * sync at all.
 *
 * A dev-account launch connects a canned harness account. Sending that to the person's real Allodia
 * account would put `admin@localhost` on their phone, so those launches get their own file, and
 * the caller does not run a pass in them either.
 */
internal fun syncStateFile(dataSubdir: String?): String =
    if (dataSubdir == null) "mailcal_prefs" else "mailcal_prefs_$dataSubdir"
