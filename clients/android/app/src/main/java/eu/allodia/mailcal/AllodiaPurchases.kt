// The seam between whichever shop this build sells through and the account service: one pass, and
// the callback that starts one when a store reports a purchase this app did not just ask for.
//
// The rules are `purchasing.md`, the contract that ships beside the Allodia Licence. Which shop it
// is comes from `allodiaBillingProvider`, which each flavour supplies, so nothing here branches on
// the build: the `foss` APK simply has a provider that reports nothing outstanding and sends people
// to a browser.
//
// On Play a pass has one half rather than two: a purchase is acknowledged once it has been
// attached, so there is nothing for this client to do afterwards. What that buys is the safety
// net: a purchase that could not be attached is never acknowledged by anybody, and Play gives the
// money back after three days.
package eu.allodia.mailcal

import android.app.Activity
import android.content.Context
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.launch
import kotlinx.coroutines.withContext
import uniffi.mailcal_bindings.AllodiaOffer
import uniffi.mailcal_bindings.AllodiaPlan
import uniffi.mailcal_bindings.AllodiaPurchaseReport
import uniffi.mailcal_bindings.MailcalApp

// Runs redemption passes and keeps Play's own callback wired to one.
//
// One per app. It owns no UI state: a screen asks it to buy and reads the entitlement afterwards
// like any other.
internal class AllodiaPurchases(
    context: Context,
    private val app: MailcalApp,
    private val scope: CoroutineScope,
) {
    private val billing =
        allodiaBillingProvider(
            context = context.applicationContext,
            app = app,
            // A renewal, a deferred payment clearing, a purchase made on another of the person's
            // devices: all of them arrive here and nowhere else.
            onPurchaseReported = { scope.launch { linkOutstanding() } },
        )

    // What this build's shop will sell, in the order the core says to draw it.
    //
    // Further offers are added by the caller, which knows whether this platform may draw them at
    // all; the ordering is the core's either way.
    suspend fun offers(others: List<AllodiaOffer> = emptyList()): List<AllodiaOffer> =
        app.orderAllodiaOffers(offers = billing.offers() + others)

    // Buys a period and carries the purchase through to the account service.
    //
    // Returns once the purchase has been attached or has been left safely outstanding. A person who
    // closes the app in between loses nothing: Play still reports it, so the next pass picks it up.
    //
    // A pass runs for anything but a dismissed sheet, not only for a sale. Play refuses a second
    // purchase of something already owned, and a purchase nothing has attached yet is exactly what
    // that refusal describes, so the refusal is the moment to carry it over. A pass costs nothing
    // when Play is reporting nothing.
    suspend fun buy(activity: Activity, plan: AllodiaPlan): AllodiaPurchaseOutcome {
        val outcome = billing.buy(activity, plan)
        if (outcome !is AllodiaPurchaseOutcome.Cancelled) linkOutstanding()
        return outcome
    }

    // One pass: hand the account service everything Play is still reporting.
    //
    // `report.finish` is deliberately not acted on here. It names what the service has finished
    // with, which an Apple client turns into a StoreKit `finish()`; on Play the purchase is already
    // acknowledged and there is nothing left for a device to do. On the checkout route nothing is
    // ever outstanding, so this returns before any of that.
    //
    // Safe to call at any time and from anywhere; the core deduplicates and paces the retries.
    suspend fun linkOutstanding(): AllodiaPurchaseReport? {
        val outstanding = billing.outstanding()
        if (outstanding.isEmpty()) return null
        // The pass makes network round trips and the core call blocks on them, so it goes off the
        // main thread exactly as the sign-in and sync passes do.
        return runCatching {
                withContext(Dispatchers.IO) { app.linkAllodiaPurchases(outstanding) }
            }
            .getOrNull()
    }

    fun close() {
        billing.close()
    }
}
