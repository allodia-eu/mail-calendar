// The shop this build sells through, and the one thing the two shops have in common.
//
// `purchasing.md` decides which build sells where: Play's payment rules bind the apps Play
// distributes, so the `play` APK sells through Play and the `foss` one, which F-Droid and the
// download we host carry, sells through Allodia's own checkout with no programme to enrol in.
// Each flavour supplies its own `allodiaBillingProvider`, and nothing above this line knows which
// it got.
//
// ⚠️ **The two flows do not have the same shape, and this interface must not pretend they do.**
// Play hands a purchase back to the device, which is then attached to the account. Allodia's
// checkout hands back nothing at all: the person leaves for a browser, the subscription is created
// on the account, and this app finds out by reading it again. That is why
// [AllodiaPurchaseOutcome.SentToCheckout] exists rather than being folded into `Bought`, and why
// `outstanding` is empty on the checkout side rather than unimplemented.
package eu.allodia.mailcal

import android.app.Activity
import uniffi.mailcal_bindings.AllodiaOffer
import uniffi.mailcal_bindings.AllodiaPlan
import uniffi.mailcal_bindings.AllodiaStorePurchase

// How one trip through a purchase ended.
internal sealed interface AllodiaPurchaseOutcome {
    // A shop took the money and handed the purchase to this device. It is still not anything until
    // the account service has attached it.
    data class Bought(val purchases: List<AllodiaStorePurchase>) : AllodiaPurchaseOutcome

    // The person is now in a browser on Allodia's checkout, and **nothing here is waiting for
    // them**. They may pay, close the tab, or never come back, and this app is told none of it.
    // What resolves it is reading the subscription again, which is what any screen drawing this
    // already does when it is next shown.
    data object SentToCheckout : AllodiaPurchaseOutcome

    // The sheet was dismissed. Not a failure, and nothing is said about it.
    data object Cancelled : AllodiaPurchaseOutcome

    // The shop could not sell it. The person sees the other routes rather than an error about
    // this one.
    data class Unavailable(val reason: String) : AllodiaPurchaseOutcome
}

internal interface AllodiaBillingProvider {
    // What this shop will sell, with the price already formatted for the reader.
    suspend fun offers(): List<AllodiaOffer>

    // Start a purchase. What comes back is never a granted subscription.
    suspend fun buy(activity: Activity, plan: AllodiaPlan): AllodiaPurchaseOutcome

    // Every purchase this device is still holding that nothing has attached.
    //
    // Always empty where the shop is a browser: a checkout leaves nothing on the device, so there
    // is nothing here to carry over and no store waiting to be settled with.
    suspend fun outstanding(): List<AllodiaStorePurchase>

    fun close()
}
