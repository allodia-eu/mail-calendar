// Flying a Nextcloud sign-in for whichever surface asked (docs/nextcloud-sign-in.md rule 5): the
// surface's own start off the main thread, the server's login page in a Custom Tab, and the poll
// on the flight's worker. What a surface does with the grant is its own; what they share is that
// the server hands a grant out once, so one found for an attempt nobody waits on any more is
// withdrawn rather than dropped.
package eu.allodia.mailcal

import android.content.Context
import android.net.Uri
import androidx.browser.customtabs.CustomTabsIntent
import java.lang.ref.WeakReference
import kotlin.concurrent.thread
import uniffi.mailcal_bindings.MailcalApp
import uniffi.mailcal_bindings.MailcalException
import uniffi.mailcal_bindings.NextcloudSignInStart
import uniffi.mailcal_bindings.nextcloudPollIntervalMs

/** The flights in the air, woken together when the app is in front again. Main thread only. */
internal object NextcloudFlights {
    private val flying = mutableListOf<NextcloudFlight>()

    // The core the last flight flew with. A grant exists only after a flight, so whatever gives one
    // up can withdraw it here without a core of its own.
    private var core: WeakReference<MailcalApp>? = null

    fun flewWith(instance: MailcalApp) {
        core = WeakReference(instance)
    }

    /** Withdraws [grant], which became no account. Returns at once. */
    fun withdraw(grant: String?) {
        grant?.let { core?.get()?.abandonNextcloudSignIn(it) }
    }

    fun add(flight: NextcloudFlight) {
        flying += flight
    }

    fun remove(flight: NextcloudFlight) {
        flying -= flight
    }

    fun wakeAll() = flying.toList().forEach { it.wake() }
}

/** What a sign-in that ended without a grant says; null for a cancelled one, which says nothing. */
internal fun nextcloudSaid(end: NextcloudEnd, ctx: Context): String? = when (end) {
    is NextcloudEnd.Granted, NextcloudEnd.Cancelled -> null
    NextcloudEnd.Expired -> L10n.account_signin_timeout(ctx)
    is NextcloudEnd.Failed -> when (val error = end.error) {
        null -> L10n.account_signin_browser_failed(ctx)
        is MailcalException.Connect -> L10n.setup_imap_signin_failed(ctx)
        else -> coreErrorSaid(error, ctx) ?: L10n.setup_imap_signin_failed(ctx)
    }
}

/**
 * Flies one sign-in. [begin] is the surface's own start, run off the main thread; the login page
 * opens unless [opening] says the surface has moved on; [ended] hears how it ended, on the main
 * thread. Null without a core.
 */
internal fun MainActivity.flyNextcloud(
    name: String,
    begin: (MailcalApp) -> NextcloudSignInStart,
    opening: () -> Boolean,
    ended: (NextcloudEnd) -> Unit,
): NextcloudFlight? {
    val instance = app ?: return null
    val flight = NextcloudFlight()
    NextcloudFlights.flewWith(instance)
    NextcloudFlights.add(flight)
    val interval = nextcloudPollIntervalMs().toLong()
    thread(name = name) {
        val end = runCatching { begin(instance) }.fold(
            onSuccess = { start ->
                mainHandler.post { openNextcloudLogin(flight, start.loginUrl, opening) }
                flight.await(interval) { instance.pollNextcloudSignIn(start.pending) }
            },
            onFailure = { NextcloudEnd.Failed(it) },
        )
        val reported = if (end is NextcloudEnd.Granted && flight.isStopped) {
            instance.abandonNextcloudSignIn(end.grant)
            NextcloudEnd.Cancelled
        } else {
            end
        }
        logUiInfo("nextcloud: the sign-in ended (${reported::class.simpleName})")
        mainHandler.post {
            NextcloudFlights.remove(flight)
            ended(reported)
        }
    }
    return flight
}

private fun MainActivity.openNextcloudLogin(flight: NextcloudFlight, loginUrl: String, opening: () -> Boolean) {
    if (flight.isStopped || !opening()) {
        flight.stop()
        return
    }
    logUiInfo("nextcloud: opening the login page; polling for the grant")
    runCatching { CustomTabsIntent.Builder().build().launchUrl(this, Uri.parse(loginUrl)) }.onFailure {
        logUiWarn("nextcloud: the browser did not open: ${it.message}")
        flight.stop(NextcloudEnd.Failed(null))
    }
}

/** Withdraws [grant], which became no account. Returns at once. */
internal fun MainActivity.abandonNextcloud(grant: String?) {
    grant?.let { app?.abandonNextcloudSignIn(it) }
}

/**
 * The app is in front again: every flight asks now, and the surfaces stop saying they wait, as a
 * provider sign-in's do; a grant still to come is taken when it comes.
 */
internal fun MainActivity.resumeNextcloudSignIns() {
    NextcloudFlights.wakeAll()
    setupFlow.nextcloud.resumed()
}
