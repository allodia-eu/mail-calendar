// The conditions a background pass ran under, recorded to the attachable log (docs/logging.md,
// docs/background-sync.md).
//
// Why this exists: the worker used to log what it *did*, start, complete, N new messages, but
// nothing about the circumstances it was woken in. So when background mail arrived hours late, the
// log could show a healthy pass that simply happened at the wrong time, and nothing in it said why.
// Answering "is background sync running every 15 minutes?" meant pulling `dumpsys jobscheduler` off
// the device over a cable, which needs the developer's laptop and the user's phone in the same room.
//
// The gap between wakes is what the user actually experiences, and `battery-exempt` is almost always
// the reason it is what it is. Both now ride in one line, so the log the user can already hand us
// answers the question on its own.
//
// Everything here is a duration, a count, or an OS policy flag, never mail content, an address, a
// carrier, or a Wi-Fi network name. The line stays safe to attach to a support request.
package eu.allodia.mailcal

import android.content.Context
import android.net.ConnectivityManager
import android.net.NetworkCapabilities

/// What the OS was doing to us when a pass was woken.
internal data class PassConditions(
    /// Minutes since the previous wake; `null` on the first since install/update. This is the
    /// number that exposes Doze: WorkManager honours the 15-minute period, but the OS defers the
    /// *job*, so a healthy worker can still deliver mail hours late.
    val minutesSinceLastWake: Long?,
    /// Whether the user has allowed unrestricted background use. When false, expect the gap above
    /// to run to hours while the phone is idle, that is Android working as intended, not a bug.
    val batteryExempt: Boolean,
    /// The transport class only, never the network's name or the carrier.
    val transport: String,
    /// WorkManager's retry count. Should always be 0: a failed pass reports success precisely so
    /// backoff never takes over the period (see MailSyncWorker). A non-zero value here means that
    /// invariant has been broken and the period is being replaced by an escalating backoff.
    val runAttempt: Int,
)

internal fun passConditionsLine(c: PassConditions): String = buildString {
    append("conditions: ")
    append(c.minutesSinceLastWake?.let { "+${it}min since last wake" } ?: "first wake since install")
    append(", battery-exempt=${c.batteryExempt}")
    append(", network=${c.transport}")
    if (c.runAttempt > 0) {
        append(", retry-attempt=${c.runAttempt}")
    }
}

/// The transport carrying the active network, or `none` when offline. Deliberately coarse: an SSID
/// or carrier would identify the user's location, which the never-log-content rule forbids.
internal fun activeTransport(context: Context): String {
    val cm = context.getSystemService(ConnectivityManager::class.java) ?: return "unknown"
    val caps = cm.getNetworkCapabilities(cm.activeNetwork) ?: return "none"
    return when {
        caps.hasTransport(NetworkCapabilities.TRANSPORT_WIFI) -> "wifi"
        caps.hasTransport(NetworkCapabilities.TRANSPORT_CELLULAR) -> "cellular"
        caps.hasTransport(NetworkCapabilities.TRANSPORT_ETHERNET) -> "ethernet"
        else -> "other"
    }
}

/// Remembers when the worker was last woken, so the next wake can report the gap. Persisted, because
/// the process is routinely killed between passes, an in-memory field would always read "first".
internal object LastWake {
    private const val FILE = "mailcal_prefs"
    private const val KEY = "last_background_wake_ms"

    fun minutesSince(context: Context, now: Long): Long? {
        val last = context.getSharedPreferences(FILE, Context.MODE_PRIVATE).getLong(KEY, 0L)
        // A clock that moved backwards (travel, NTP correction) would otherwise report a negative
        // gap; clamp rather than lie.
        return if (last == 0L) null else ((now - last) / 60_000L).coerceAtLeast(0)
    }

    fun record(context: Context, now: Long) {
        context.getSharedPreferences(FILE, Context.MODE_PRIVATE)
            .edit()
            .putLong(KEY, now)
            .apply()
    }
}
