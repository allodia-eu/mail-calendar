// The subscription section, driven from the activity: asking the core to read it again, the
// payment page a checkout names, and the half of buying the core cannot do, Play's.
//
// The read, Allodia's checkout and the three writes are the core's (`AllodiaIntent`), and the
// section is drawn from its card. Which shop this build sells through is `allodiaBillingProvider`,
// one file per flavour, so nothing here branches on the build.
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
import uniffi.mailcal_bindings.AllodiaIntent
import uniffi.mailcal_bindings.AllodiaPlan
import uniffi.mailcal_bindings.AllodiaPrices
import uniffi.mailcal_bindings.AllodiaPurchaseReport
import uniffi.mailcal_bindings.AllodiaStorePage
import uniffi.mailcal_bindings.Intent
import uniffi.mailcal_bindings.MailcalApp

// The scope every purchase runs in. Its own rather than a composable's: Play's sheet leaves the
// app, and a scope belonging to a screen recomposed in the meantime would drop the pass that
// records what was bought.
internal fun allodiaPurchaseScope(): CoroutineScope =
    CoroutineScope(SupervisorJob() + Dispatchers.Main.immediate)

// Opens this build's shop, once, at connect.
//
// A build carrying no Allodia registration has no account to attach a purchase to and no section
// that offers one, so it opens no connection to Play either.
internal fun MainActivity.startAllodiaPurchases(instance: MailcalApp) {
    if (!allodiaSignInOffered() || allodiaPurchases != null) return
    allodiaPurchases =
        AllodiaPurchases(context = this, app = instance, scope = allodiaPurchaseScope) { report ->
            allodiaLinked(report)
        }
}

internal fun MainActivity.closeAllodiaPurchases() {
    allodiaPurchases?.close()
    allodiaPurchases = null
    allodiaPurchaseScope.cancel()
}

/** What the section has to draw what it can at once: the actions the activity answers. */
internal fun MainActivity.allodiaSubscriptionActions(onSignInAgain: () -> Unit) =
    AllodiaSubscriptionActions(
        onShown = {
            allodiaSubscriptionOnScreen = true
            refreshAllodiaSubscription()
        },
        onClosed = {
            allodiaSubscriptionOnScreen = false
            forgetAllodiaPurchaseInFlight()
        },
        onOffers = { loadAllodiaOffers(it) },
        onBuy = { buyAllodiaSubscription(it) },
        onManageStore = { manageAllodiaStoreSubscription(it) },
        onSignInAgain = onSignInAgain,
        onWrite = { changeAllodia(it) },
    )

// Reads the subscription again, after carrying over anything Play still holds, so a purchase made
// on another device or one this device could not attach last time is in what is read back.
internal fun MainActivity.refreshAllodiaSubscription() {
    val instance = app ?: return
    val purchases = allodiaPurchases
    if (purchases == null) {
        instance.dispatch(REFRESH_SUBSCRIPTION)
    } else {
        // The pass ends in [allodiaLinked], which asks for the read.
        allodiaPurchaseScope.launch { purchases.linkOutstanding() }
    }
}

private val REFRESH_SUBSCRIPTION = Intent.Allodia(AllodiaIntent.RefreshSubscription)

// A pass ended: what it found is said, and the subscription is read again whenever it attached
// something or the section is on screen.
private fun MainActivity.allodiaLinked(report: AllodiaPurchaseReport?) {
    allodiaStore = allodiaStore.copy(
        anythingStuck = report?.anythingStuck == true,
        // A list rather than a flag, because the core names which purchases; a person cannot act
        // on a store transaction id, so only whether it happened reaches the screen.
        claimedElsewhere = report?.claimedElsewhere.orEmpty().isNotEmpty(),
    )
    if (report != null || allodiaSubscriptionOnScreen) app?.dispatch(REFRESH_SUBSCRIPTION)
}

// What this build's shop sells, for an account nobody is charging.
private fun MainActivity.loadAllodiaOffers(prices: AllodiaPrices?) {
    val instance = app ?: return
    val purchases = allodiaPurchases ?: return
    allodiaPurchaseScope.launch {
        val offers = purchases.offers(prices)
        val accountId = withContext(Dispatchers.IO) { instance.allodiaAccount()?.id }
        allodiaStore = allodiaStore.copy(offers = offers, accountId = accountId)
    }
}

/** The payment page the core's card names, opened once for each id. */
internal fun MainActivity.openAllodiaCheckoutPage() {
    val page = allodiaSubscription.checkoutPage?.takeIf { it.id > allodiaCheckoutOpened } ?: return
    allodiaCheckoutOpened = page.id
    // ⚠️ The browser, never a web view: this page carries the payment method and the
    // authorisation for a recurring charge, the reason RFC 8252 keeps an authorization request out
    // of one.
    logUiInfo("allodia: opening the payment page in the browser")
    runCatching { CustomTabsIntent.Builder().build().launchUrl(this, Uri.parse(page.url)) }
        .onFailure { logUiWarn("allodia: the payment page could not be opened") }
}

// Buys a period. On Play the purchase is carried through to the account service before this
// returns, or left safely outstanding; on Allodia's checkout the core opens the payment page.
private fun MainActivity.buyAllodiaSubscription(plan: AllodiaPlan) {
    val purchases = allodiaPurchases ?: return
    if (allodiaStore.buying != null) return
    allodiaStore = allodiaStore.copy(buying = plan, note = null)
    allodiaPurchaseScope.launch {
        val outcome = purchases.buy(this@buyAllodiaSubscription, plan)
        allodiaStore = allodiaStore.copy(buying = null, note = noteFor(outcome, plan))
    }
}

// ⚠️ **Every ending is logged, the quiet ones most of all.** A dismissed sheet says nothing to the
// person, so a purchase that never had a chance and one somebody thought better of look identical
// on screen, and the log is the only place they can be told apart.
private fun MainActivity.noteFor(outcome: AllodiaPurchaseOutcome, plan: AllodiaPlan): String? =
    when (outcome) {
        is AllodiaPurchaseOutcome.Bought -> {
            logUiInfo("allodia: the shop took a $plan purchase")
            null
        }
        is AllodiaPurchaseOutcome.SentToCheckout -> {
            logUiInfo("allodia: a $plan checkout was asked for")
            null
        }
        is AllodiaPurchaseOutcome.Cancelled -> {
            logUiInfo("allodia: a $plan purchase ended without one")
            null
        }
        // The shop's messages name products and storefronts, never an address or a secret.
        is AllodiaPurchaseOutcome.Unavailable -> {
            logUiWarn("allodia: a $plan purchase did not complete (${outcome.reason})")
            L10n.settings_subscription_failed(this, outcome.reason)
        }
    }

// The section has gone, so nothing is waiting on a sheet any more.
//
// ⚠️ **Letting go of the wait is not abandoning the purchase.** Play reports it on its own
// listener, which starts a pass of its own, and a purchase nothing attached is refunded after three
// days rather than lost.
private fun MainActivity.forgetAllodiaPurchaseInFlight() {
    if (allodiaStore.buying != null) {
        logUiInfo("allodia: the subscription section was closed while a purchase was in flight")
    }
    // The note answers an attempt somebody has just made; read a week later it is about nothing.
    allodiaStore = allodiaStore.copy(buying = null, note = null)
}

// Opens the store's own subscription page, the only thing that can change what that store bills.
private fun MainActivity.manageAllodiaStoreSubscription(page: AllodiaStorePage) {
    logUiInfo("allodia: opening the store's subscription page")
    CustomTabsIntent.Builder().build().launchUrl(this, Uri.parse(page.url))
}
