// The new-mail notifications choice is the core's (`notifications_page`). An install that kept it
// in SharedPreferences hands it over once, on every path that builds a core over the real store and
// before that core runs a pass, so a person who turned notifications off stays off.
package eu.allodia.mailcal

import android.content.Context
import uniffi.mailcal_bindings.PreferenceIntent

internal object NotificationsChoiceImport {
    private const val FILE = "mailcal_prefs"
    private const val KEY = "new_mail_notifications_enabled"

    /**
     * Sends a stored "off" to the core through [setPreference], then forgets the stored key. A
     * stored "on" is the core's default, so it sends nothing. Nothing stored is a no-op.
     */
    fun run(context: Context, setPreference: (PreferenceIntent) -> Unit) {
        val prefs = context.getSharedPreferences(FILE, Context.MODE_PRIVATE)
        if (!prefs.contains(KEY)) return
        if (!prefs.getBoolean(KEY, true)) {
            setPreference(PreferenceIntent.SetNewMailNotifications(on = false))
        }
        prefs.edit().remove(KEY).apply()
    }
}
