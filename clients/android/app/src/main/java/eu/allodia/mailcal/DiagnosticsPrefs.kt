// The user's "include more detail" (DEBUG) diagnostics choice, a small client-side preference,
// like NotificationPrefs. Persisted because a support session routinely spans an app restart:
// the Diagnostics screen's toggle raises the LIVE core's ceiling via `MailcalApp.setLogLevel`,
// and every core constructed afterwards (the foreground connect, a showcase run, and the
// background worker's own headless core) re-applies the choice at boot via `bootLogLevel`, a
// worker that woke on INFO while the user had opted into DEBUG would silently punch a hole in
// exactly the trace the session needs. See docs/logging.md ("DEBUG opt-in").
package eu.allodia.mailcal

import android.content.Context
import uniffi.mailcal_bindings.LogLevel

object DiagnosticsPrefs {
    private const val FILE = "mailcal_prefs"
    private const val KEY_DEBUG = "diagnostic_log_debug_enabled"

    /// Whether the user opted into DEBUG detail (off on a fresh install, INFO is the default
    /// ceiling that keeps the rotating log's window long, per docs/logging.md).
    fun debugEnabled(context: Context): Boolean =
        prefs(context).getBoolean(KEY_DEBUG, false)

    /// Persists the user's choice. The caller applies it to the live core (`setLogLevel`);
    /// this only makes the choice survive to the next boot.
    fun setDebugEnabled(context: Context, value: Boolean) {
        prefs(context).edit().putBoolean(KEY_DEBUG, value).apply()
    }

    /// The log ceiling every newly-constructed core boots with.
    fun bootLogLevel(context: Context): LogLevel = logLevelForDebug(debugEnabled(context))

    private fun prefs(context: Context) =
        context.getSharedPreferences(FILE, Context.MODE_PRIVATE)
}

/// The toggle's level mapping: ON is DEBUG, OFF is back to the INFO default, never ERROR/WARN,
/// which would hide the lifecycle events the log exists to capture.
internal fun logLevelForDebug(enabled: Boolean): LogLevel =
    if (enabled) LogLevel.DEBUG else LogLevel.INFO
