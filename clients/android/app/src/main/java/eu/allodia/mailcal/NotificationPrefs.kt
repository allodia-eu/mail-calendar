// The user's "new-mail notifications" on/off choice, a small client-side preference (the core
// still runs the background sync and advances its marks regardless; this only gates whether the
// worker *posts* a notification, so toggling off then on never floods with a backlog).
package eu.allodia.mailcal

import android.content.Context

object NotificationPrefs {
    private const val FILE = "mailcal_prefs"
    private const val KEY_ENABLED = "new_mail_notifications_enabled"

    /// Whether new-mail notifications are on (the default for a fresh install).
    fun enabled(context: Context): Boolean =
        prefs(context).getBoolean(KEY_ENABLED, true)

    /// Persists the user's choice.
    fun setEnabled(context: Context, value: Boolean) {
        prefs(context).edit().putBoolean(KEY_ENABLED, value).apply()
    }

    private fun prefs(context: Context) =
        context.getSharedPreferences(FILE, Context.MODE_PRIVATE)
}
