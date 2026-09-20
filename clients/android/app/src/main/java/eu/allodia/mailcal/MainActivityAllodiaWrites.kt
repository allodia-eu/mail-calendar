// The three writes against the subscription Allodia bills directly, driven from the activity:
// stop the recurring charge, start it again, and move between the two periods.
//
// ⚠️ **Every one of these is a blocking network call**, so each goes off the main thread exactly as
// the read next door does, and each re-reads afterwards rather than guessing what it changed. The
// service is the one that knows what a write did: a resubscribe may come back already running or
// with a page to open, and only it can say which.
//
// What a client may offer at all is `actions`, read where the buttons are drawn. Nothing here
// second-guesses it.
package eu.allodia.mailcal

import android.net.Uri
import androidx.browser.customtabs.CustomTabsIntent
import java.util.Locale
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.launch
import kotlinx.coroutines.withContext
import uniffi.mailcal_bindings.AllodiaIntervalChange
import uniffi.mailcal_bindings.AllodiaPlan

// Stop the recurring charge. Everything already paid for runs to the date the service names, which
// is what the card then says: a cancellation that reported nothing would read as "it stopped now".
internal fun MainActivity.cancelAllodiaSubscription() {
    runAllodiaWrite("cancel") { instance ->
        val cancellation = instance.cancelAllodiaSubscription()
        L10n.settings_subscription_cancelled(this, allodiaWrittenDate(cancellation.endDate))
    }
}

// Start a cancelled subscription again.
//
// Two endings, and the service decides which: restarted on the authorisation already held, with
// nothing to pay and nothing to re-enter, or a page to open in a browser. A client draws both,
// which is why `reactivated` is read rather than assumed from the absence of a URL.
internal fun MainActivity.resubscribeToAllodia() {
    runAllodiaWrite("resubscribe") { instance ->
        val checkout = instance.resubscribeToAllodia()
        val url = checkout.checkoutUrl
        when {
            checkout.reactivated -> L10n.settings_subscription_resubscribed(this)
            url != null -> {
                // ⚠️ The browser, never a web view: this page carries the payment method and the
                // authorisation for a recurring charge, which is the same reason RFC 8252 keeps an
                // authorization request out of one.
                mainHandler.post {
                    CustomTabsIntent.Builder().build().launchUrl(this, Uri.parse(url))
                }
                null
            }
            // The service refused without saying so in a way this build can draw. The re-read
            // below is what will show whatever it did decide.
            else -> null
        }
    }
}

// Move between the monthly and the yearly plan.
//
// **Nothing moves today.** The period already paid for runs on untouched, and only the charge after
// it changes, which is what the confirmation said and what this then reports with the service's own
// figures rather than today's list price.
internal fun MainActivity.switchAllodiaInterval(plan: AllodiaPlan) {
    runAllodiaWrite("switch to $plan") { instance ->
        allodiaSwitchNote(instance.switchAllodiaInterval(plan))
    }
}

private fun MainActivity.allodiaSwitchNote(change: AllodiaIntervalChange): String {
    val date = change.nextPaymentDate?.let { allodiaWrittenDate(it) }.orEmpty()
    val amount =
        allodiaMinorUnits(
            change.amountInCents,
            allodiaWriteCurrency(),
            resources.configuration.locales[0] ?: Locale.getDefault(),
        )
    return L10n.settings_subscription_switched(this, date, amount)
}

// One write: off the main thread, its answer into the card's note, then a re-read.
//
// The note is cleared first, because the last write's sentence beside this one's outcome reads as
// one statement about the wrong thing.
private fun MainActivity.runAllodiaWrite(what: String, write: (uniffi.mailcal_bindings.MailcalApp) -> String?) {
    val instance = app ?: return
    allodiaSubscription = allodiaSubscription.copy(note = null)
    allodiaPurchaseScope.launch {
        val note =
            runCatching { withContext(Dispatchers.IO) { write(instance) } }
                .onSuccess { logUiInfo("allodia: $what was accepted") }
                .onFailure { failure ->
                    // The service's refusals name plans and dates, never an address or a secret.
                    logUiWarn("allodia: $what did not go through (${failure.message})")
                }
                .getOrElse { failure -> allodiaRefusalText(allodiaWriteFailure(failure)) }
        allodiaSubscription = allodiaSubscription.copy(note = note)
        // What the write actually did is the next read's answer, never this one's: the service
        // recomputes every biller, and a screen that edited its own copy would disagree with it.
        refreshAllodiaSubscription()
    }
}

// A refusal's own words. Never the exception's text, which is a generated field name
// (AllodiaSubscriptionModel.kt).
private fun MainActivity.allodiaRefusalText(failure: AllodiaWriteFailure): String =
    when (failure) {
        AllodiaWriteFailure.ALREADY_CANCELLED ->
            L10n.settings_subscription_refused_already_cancelled(this)
        AllodiaWriteFailure.ALREADY_ACTIVE ->
            L10n.settings_subscription_refused_already_active(this)
        AllodiaWriteFailure.ALREADY_ON_INTERVAL ->
            L10n.settings_subscription_refused_already_on_interval(this)
        AllodiaWriteFailure.NOT_SWITCHABLE ->
            L10n.settings_subscription_refused_not_switchable(this)
        AllodiaWriteFailure.NOT_FOUND -> L10n.settings_subscription_refused_not_found(this)
        AllodiaWriteFailure.UNEXPLAINED -> L10n.settings_subscription_write_failed(this)
    }

// A date from a write, formatted for the reader, or the wire when this build cannot read it.
private fun MainActivity.allodiaWrittenDate(raw: String): String =
    allodiaDate(raw, resources.configuration.locales[0] ?: Locale.getDefault()) ?: raw

// The currency the account is billed in, which the service sends with today's prices. It is the
// only place a client can learn it, and a switch reports an amount in minor units alone.
private fun MainActivity.allodiaWriteCurrency(): String =
    (allodiaSubscription.state as? AllodiaSubscriptionState.Loaded)
        ?.view
        ?.subscription
        ?.prices
        ?.currency
        .orEmpty()
