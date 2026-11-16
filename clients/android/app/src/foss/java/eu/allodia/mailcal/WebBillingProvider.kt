// Buying a subscription from Allodia's own checkout, which is what the `foss` APK sells through.
//
// There is no store on this build and no Google library in it, so there is nothing to query,
// nothing to acknowledge and nothing left outstanding on the device. The checkout is the core's:
// its card prices it and names the payment page, which the activity opens in the browser.
package eu.allodia.mailcal

import android.app.Activity
import android.content.Context
import uniffi.mailcal_bindings.AllodiaIntent
import uniffi.mailcal_bindings.AllodiaOffer
import uniffi.mailcal_bindings.AllodiaPlan
import uniffi.mailcal_bindings.AllodiaPrices
import uniffi.mailcal_bindings.AllodiaStore
import uniffi.mailcal_bindings.AllodiaStorePurchase
import uniffi.mailcal_bindings.Intent
import uniffi.mailcal_bindings.MailcalApp

internal class WebBillingProvider(
    private val context: Context,
    private val app: MailcalApp,
) : AllodiaBillingProvider {

    // Allodia's own two prices, formatted here: they arrive as minor units and an ISO 4217 code
    // because the core carries no locale data at all. Nothing while the core does not sell the
    // checkout, which is the truth rather than a price nobody can pay.
    override suspend fun offers(checkout: AllodiaPrices?): List<AllodiaOffer> {
        val prices = checkout ?: return emptyList()
        return listOf(
            AllodiaOffer(
                plan = AllodiaPlan.YEARLY,
                store = AllodiaStore.ALLODIA,
                displayPrice = formatPrice(prices.yearlyInCents, prices.currency),
            ),
            AllodiaOffer(
                plan = AllodiaPlan.MONTHLY,
                store = AllodiaStore.ALLODIA,
                displayPrice = formatPrice(prices.monthlyInCents, prices.currency),
            ),
        )
    }

    // Asks the core for the checkout and returns at once: the card names the payment page, and
    // the subscription is created against the account by the checkout itself.
    override suspend fun buy(activity: Activity, plan: AllodiaPlan): AllodiaPurchaseOutcome {
        app.dispatch(Intent.Allodia(AllodiaIntent.StartCheckout(plan)))
        return AllodiaPurchaseOutcome.SentToCheckout
    }

    // Always empty: a checkout leaves nothing on the device to attach.
    override suspend fun outstanding(): List<AllodiaStorePurchase> = emptyList()

    override fun close() = Unit

    private fun formatPrice(minorUnits: Long, currency: String): String =
        allodiaMinorUnits(minorUnits, currency, context.resources.configuration.locales[0])
}
