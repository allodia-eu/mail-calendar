// Buying a subscription from Allodia's own checkout, which is what the `foss` APK sells through.
//
// There is no store on this build and no Google library in it, so there is nothing to query,
// nothing to acknowledge and nothing left outstanding on the device. What there is: two prices the
// account service quotes, and a hosted page to open.
//
// ⚠️ **The page opens in the browser, never in a web view.** It carries the payment-method choice
// and the authorisation for a recurring charge, which is the same reason RFC 8252 keeps an
// authorization request out of one. A Custom Tab is the browser, and is what the Microsoft sign-in
// on this client already uses.
package eu.allodia.mailcal

import android.app.Activity
import android.content.Context
import android.net.Uri
import androidx.browser.customtabs.CustomTabsIntent
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.withContext
import uniffi.mailcal_bindings.AllodiaOffer
import uniffi.mailcal_bindings.AllodiaPlan
import uniffi.mailcal_bindings.AllodiaStore
import uniffi.mailcal_bindings.AllodiaStorePurchase
import uniffi.mailcal_bindings.MailcalApp

internal class WebBillingProvider(
    private val context: Context,
    private val app: MailcalApp,
) : AllodiaBillingProvider {

    // Allodia's own two prices, formatted here.
    //
    // They arrive as minor units and an ISO 4217 code because the core carries no locale data at
    // all, so turning 199 and EUR into something readable is this side's job. That is the same
    // split as a store's formatted string on the other route, arrived at from the other direction.
    //
    // An empty list when the account service cannot be reached or has no billing configured: the
    // screen then has nothing to offer, which is the truth, rather than a price nobody can pay.
    override suspend fun offers(): List<AllodiaOffer> {
        val prices =
            runCatching { withContext(Dispatchers.IO) { app.allodiaSubscription() } }
                .getOrNull()
                ?.prices ?: return emptyList()
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

    // Opens the checkout and returns immediately.
    //
    // **Nothing waits for the result, because there is no result to wait for.** The subscription is
    // created against the account by the checkout itself, so what tells this app it happened is
    // reading the subscription again, not anything handed back here.
    override suspend fun buy(activity: Activity, plan: AllodiaPlan): AllodiaPurchaseOutcome {
        val checkout =
            runCatching { withContext(Dispatchers.IO) { app.startAllodiaCheckout(plan) } }
                .getOrNull() ?: return AllodiaPurchaseOutcome.Unavailable("no checkout")
        CustomTabsIntent.Builder().build().launchUrl(activity, Uri.parse(checkout.checkoutUrl))
        return AllodiaPurchaseOutcome.SentToCheckout
    }

    // Always empty: a checkout leaves nothing on the device to attach.
    override suspend fun outstanding(): List<AllodiaStorePurchase> = emptyList()

    override fun close() = Unit

    private fun formatPrice(minorUnits: Long, currency: String): String =
        java.text.NumberFormat.getCurrencyInstance(context.resources.configuration.locales[0])
            .apply { this.currency = java.util.Currency.getInstance(currency) }
            .format(minorUnits / 100.0)
}
