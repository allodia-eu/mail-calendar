// The seam between Google Play and the account service: one redemption pass, and the callback that
// starts one when Play reports a purchase this app did not just ask for.
//
// The rules are `purchasing.md`, the contract that ships beside the Allodia Licence. On this
// platform a pass has one half rather than two: the service acknowledges a Play purchase itself
// when it attaches it, so there is nothing for this client to do afterwards. What that buys is the
// safety net: a purchase the service could not attach is never acknowledged by anybody, and Play
// gives the money back after three days.
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
import uniffi.mailcal_bindings.AllodiaStore
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
        AllodiaBilling(
            context = context.applicationContext,
            catalogue = app.allodiaStoreProducts(store = AllodiaStore.GOOGLE),
            // A renewal, a deferred payment clearing, a purchase made on another of the person's
            // devices: all of them arrive here and nowhere else.
            onPurchaseReported = { scope.launch { linkOutstanding() } },
        )

    // What Play will sell, in the order the core says to draw it.
    //
    // Offers from Allodia's own checkout are added by the caller, which knows whether this platform
    // may draw them at all; the ordering is the core's either way.
    suspend fun offers(others: List<AllodiaOffer> = emptyList()): List<AllodiaOffer> =
        app.orderAllodiaOffers(offers = billing.offers() + others)

    // Buys a period and carries the purchase through to the account service.
    //
    // Returns once the purchase has been attached or has been left safely outstanding. A person who
    // closes the app in between loses nothing: Play still reports it, so the next pass picks it up.
    suspend fun buy(activity: Activity, plan: AllodiaPlan): AllodiaPurchaseOutcome {
        val outcome = billing.buy(activity, plan)
        if (outcome is AllodiaPurchaseOutcome.Bought) linkOutstanding()
        return outcome
    }

    // One pass: hand the account service everything Play is still reporting.
    //
    // `report.finish` is deliberately not acted on here. It names what the service has finished
    // with, which an Apple client turns into a StoreKit `finish()`; on Play the service has already
    // acknowledged the purchase and there is nothing left for a device to do.
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
