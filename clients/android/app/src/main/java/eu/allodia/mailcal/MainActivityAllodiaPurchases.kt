// Buying a subscription, driven from the activity: the one read the card draws, the purchase that
// starts a subscription, and the page a store's own subscription is changed on.
//
// The rules are `purchasing.md`, the contract beside the Allodia Licence. Which shop this build
// sells through is `allodiaBillingProvider`, one file per flavour, so nothing here branches on the
// build. Its Apple twin is MailcalModel.AllodiaSubscription.swift.
//
// ⚠️ **None of this gates a capability.** `allodiaEntitlement` does, locally and without a network
// call. A service that cannot be reached costs somebody a sentence, never access they paid for.
package eu.allodia.mailcal

import android.net.Uri
import androidx.browser.customtabs.CustomTabsIntent
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.SupervisorJob
import kotlinx.coroutines.cancel
import kotlinx.coroutines.launch
import kotlinx.coroutines.withContext
import uniffi.mailcal_bindings.AllodiaGrantHealth
import uniffi.mailcal_bindings.AllodiaPlan
import uniffi.mailcal_bindings.AllodiaStoreSubscription
import uniffi.mailcal_bindings.MailcalApp

// The scope every purchase and every read of this card runs in. Its own rather than a composable's:
// both flows leave the app, for Play's sheet or for a browser, and a scope belonging to a screen
// that was recomposed in the meantime would drop the pass that records what was bought.
internal fun allodiaPurchaseScope(): CoroutineScope =
    CoroutineScope(SupervisorJob() + Dispatchers.Main.immediate)

// Opens this build's shop, once, at connect.
//
// A build carrying no Allodia registration has no account to attach a purchase to and no card that
// offers one, so it opens no connection to Play either: absent, rather than present and
// unreachable, which is the posture the whole category already takes.
internal fun MainActivity.startAllodiaPurchases(instance: MailcalApp) {
    if (!allodiaSignInOffered() || allodiaPurchases != null) return
    allodiaPurchases =
        AllodiaPurchases(context = this, app = instance, scope = allodiaPurchaseScope)
}

internal fun MainActivity.closeAllodiaPurchases() {
    allodiaPurchases?.close()
    allodiaPurchases = null
    allodiaPurchaseScope.cancel()
}

// Everything the subscription card draws, in one pass.
//
// The redemption pass runs first, so a purchase made on another device, or one this device could
// not attach last time, is attached before the state it produces is read back.
internal fun MainActivity.refreshAllodiaSubscription() {
    val instance = app ?: return
    val purchases = allodiaPurchases ?: return
    allodiaPurchaseScope.launch {
        val stuck = purchases.linkOutstanding()?.anythingStuck == true
        // The read is a network round trip and the core call blocks on it, so it goes off the main
        // thread exactly as the sign-in and sync passes do.
        val answer =
            runCatching { withContext(Dispatchers.IO) { instance.allodiaSubscription() } }
                .getOrNull()
        if (answer == null) {
            // ⚠️ **The core's typed answer decides what a failed read may say, never the
            // failure's text.** A sign-in that predates the permission this read needs is an
            // offer with a remedy, and drawing it as an outage tells somebody to wait for
            // something that will never arrive.
            val health = instance.allodiaGrantHealth()
            logUiWarn("allodia: the subscription could not be read (grant is $health)")
            allodiaSubscription = allodiaSubscription.copy(
                state =
                    if (health == AllodiaGrantHealth.NEEDS_REAUTH) {
                        AllodiaSubscriptionState.NeedsReauth
                    } else {
                        AllodiaSubscriptionState.Unavailable
                    },
                accountId = instance.allodiaAccount()?.id,
            )
            return@launch
        }
        // Offers cost a round trip to the shop, so they are fetched only when they can be drawn.
        val offers = if (answer.entitled) emptyList() else purchases.offers()
        allodiaSubscription = allodiaSubscription.copy(
            state = AllodiaSubscriptionState.Loaded(
                AllodiaSubscriptionView(
                    subscription = answer,
                    offers = offers,
                    anythingStuck = stuck,
                )
            ),
            accountId = instance.allodiaAccount()?.id,
        )
    }
}

// Buys a period and carries the purchase through to the account service.
//
// Returns the card to an ordinary state once the purchase has been attached or has been left
// safely outstanding. Somebody who closes the app in between loses nothing: Play still reports the
// purchase, so the next pass picks it up.
internal fun MainActivity.buyAllodiaSubscription(plan: AllodiaPlan) {
    val purchases = allodiaPurchases ?: return
    if (allodiaSubscription.buying != null) return
    allodiaSubscription = allodiaSubscription.copy(buying = plan, note = null)
    allodiaPurchaseScope.launch {
        val outcome = purchases.buy(this@buyAllodiaSubscription, plan)
        allodiaSubscription = allodiaSubscription.copy(buying = null, note = noteFor(outcome, plan))
        // Re-read rather than assume: what was bought is granted only once the account service has
        // attached it, and this read is what says whether it has. On the checkout route there is
        // nothing to have been granted yet, and the read is what will find it when there is.
        if (outcome !is AllodiaPurchaseOutcome.Cancelled) refreshAllodiaSubscription()
    }
}

// ⚠️ **Every ending is logged, the quiet ones most of all.** A dismissed sheet says nothing to the
// person, deliberately, so a purchase that never had a chance and one somebody thought better of
// look identical on screen, and the log is the only place they can be told apart.
private fun MainActivity.noteFor(outcome: AllodiaPurchaseOutcome, plan: AllodiaPlan): String? =
    when (outcome) {
        is AllodiaPurchaseOutcome.Bought -> {
            logUiInfo("allodia: the shop took a $plan purchase")
            null
        }
        is AllodiaPurchaseOutcome.SentToCheckout -> {
            logUiInfo("allodia: a $plan checkout was opened in the browser")
            null
        }
        is AllodiaPurchaseOutcome.Cancelled -> {
            logUiInfo("allodia: a $plan purchase ended without one")
            null
        }
        // The shop's messages name products and storefronts, never an address or a secret, so
        // showing and logging one stays inside the never-log-content rule.
        is AllodiaPurchaseOutcome.Unavailable -> {
            logUiWarn("allodia: a $plan purchase did not complete (${outcome.reason})")
            L10n.settings_subscription_failed(this, outcome.reason)
        }
    }

// The card has gone, so nothing is waiting on a sheet any more.
//
// ⚠️ **Letting go of the wait is not abandoning the purchase.** Play reports it on its own
// listener, which starts a pass of its own, and a purchase nothing attached is refunded after three
// days rather than lost. What this buys is the way back from a flow the shop never answers, which
// would otherwise leave every button disabled behind a spinner that does not stop.
internal fun MainActivity.forgetAllodiaPurchaseInFlight() {
    if (allodiaSubscription.buying == null) return
    logUiInfo("allodia: the subscription card was closed while a purchase was in flight")
    allodiaSubscription = allodiaSubscription.copy(buying = null)
}

// Opens the store's own subscription page, which is the only thing that can change a subscription
// the store is billing.
internal fun MainActivity.manageAllodiaStoreSubscription(store: AllodiaStoreSubscription) {
    logUiInfo("allodia: opening the store's subscription page")
    CustomTabsIntent.Builder().build().launchUrl(this, Uri.parse(store.manageUrl))
}
