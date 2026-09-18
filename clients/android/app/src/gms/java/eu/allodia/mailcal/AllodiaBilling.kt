// Buying a subscription through Google Play, the Play Billing half.
//
// The rules are `purchasing.md`, the contract that ships beside the Allodia Licence, and the core
// holds every one of them. What lives here is the part no Rust can do: asking Play for the
// subscription, launching the billing flow, and collecting what Play still reports.
//
// ⚠️ **This client never acknowledges a purchase.** It is acknowledged once the purchase has been
// attached to the account, and not before: Play refunds an unacknowledged purchase after three
// days, which is exactly the right outcome for one that could not be attached. Acknowledging here
// would throw that away and grant nothing in return.
//
// What is handed over is Play's **purchase token**, which is the whole of what the service takes.
// It reads every fact about the subscription back from the Play Developer API with a service
// account that exists nowhere on a device.
package eu.allodia.mailcal

import android.app.Activity
import android.content.Context
import com.android.billingclient.api.BillingClient
import com.android.billingclient.api.BillingClientStateListener
import com.android.billingclient.api.BillingFlowParams
import com.android.billingclient.api.BillingResult
import com.android.billingclient.api.PendingPurchasesParams
import com.android.billingclient.api.ProductDetails
import com.android.billingclient.api.Purchase
import com.android.billingclient.api.PurchasesUpdatedListener
import com.android.billingclient.api.QueryProductDetailsParams
import com.android.billingclient.api.QueryPurchasesParams
import com.android.billingclient.api.queryProductDetails
import com.android.billingclient.api.queryPurchasesAsync
import kotlinx.coroutines.CompletableDeferred
import uniffi.mailcal_bindings.AllodiaOffer
import uniffi.mailcal_bindings.AllodiaPlan
import uniffi.mailcal_bindings.AllodiaStorePurchase
import uniffi.mailcal_bindings.AllodiaStore
import uniffi.mailcal_bindings.AllodiaStoreProduct

// How one trip through the billing flow ended.
internal sealed interface AllodiaPurchaseOutcome {
    // Play took the money. The purchase still has to be attached to the account before it is
    // anything at all.
    data class Bought(val purchases: List<AllodiaStorePurchase>) : AllodiaPurchaseOutcome

    // The sheet was dismissed. Not a failure, and nothing is said about it.
    data object Cancelled : AllodiaPurchaseOutcome

    // Play could not sell it: no such base plan in this country, billing unavailable, a device
    // with no Play Store. The person sees the other routes rather than an error about this one.
    data class Unavailable(val reason: String) : AllodiaPurchaseOutcome
}

// Google Play, as this app uses it.
//
// One per app. Play drops the connection at will, and since Billing 8 the library re-establishes
// it: `enableAutoServiceReconnection` below means a call made while disconnected reconnects rather
// than failing, so what is left here is the first connection and nothing else.
internal class AllodiaBilling(
    context: Context,
    private val catalogue: List<AllodiaStoreProduct>,
    // Called when Play reports a purchase this app did not just ask for: a renewal, a deferred
    // payment clearing, a purchase made on another device. **Required, not an optimisation**: it
    // is the only place those arrive.
    //
    // It carries nothing, because a pass asks Play for the whole outstanding set anyway. Handing
    // over what arrived would create a second path to the same work, and two paths to one pass is
    // two chances for them to disagree about what is outstanding.
    private val onPurchaseReported: () -> Unit,
) {
    private val listener = PurchasesUpdatedListener { result, purchases ->
        // A flow this app is awaiting takes the result and nothing else: its caller attaches as
        // soon as it returns, so notifying as well would run the same pass twice for one purchase.
        val awaited = pendingFlow
        pendingFlow = null
        if (awaited != null) {
            awaited.complete(result to purchases.orEmpty())
        } else if (result.responseCode == BillingClient.BillingResponseCode.OK) {
            onPurchaseReported()
        }
    }

    private val client: BillingClient =
        BillingClient.newBuilder(context)
            .setListener(listener)
            // The library retries a call made while the service is disconnected, which is every
            // call here: Play drops the connection whenever it updates itself. Without it each of
            // those surfaces as a failure a person would read as "the purchase did not work".
            .enableAutoServiceReconnection()
            // Required since Billing 5: a client built without it throws on the first call. The
            // params have to declare at least one product type, and this app sells only
            // auto-renewing subscriptions, which are never pending, so what is declared here
            // changes nothing about what arrives.
            .enablePendingPurchases(
                PendingPurchasesParams.newBuilder().enableOneTimeProducts().build()
            )
            .build()

    // Written by a coroutine and read by Play's own callback thread, so the write has to be
    // visible across both.
    @Volatile
    private var pendingFlow: CompletableDeferred<Pair<BillingResult, List<Purchase>>>? = null

    // Everything Play will sell here, with Play's own localised price.
    //
    // The price string is **drawn, never parsed**: it is already localised and Play's commission is
    // already in it. A base plan Play does not offer in this country is left out rather than
    // raised, so a person sees the plans that are available rather than an error about one that
    // is not.
    suspend fun offers(): List<AllodiaOffer> {
        val details = productDetails() ?: return emptyList()
        return catalogue.mapNotNull { wanted ->
            val basePlan = wanted.basePlan ?: return@mapNotNull null
            val offer =
                details.subscriptionOfferDetails
                    ?.firstOrNull { it.basePlanId == basePlan }
                    ?: return@mapNotNull null
            val price =
                offer.pricingPhases.pricingPhaseList.lastOrNull()?.formattedPrice
                    ?: return@mapNotNull null
            AllodiaOffer(plan = wanted.plan, store = AllodiaStore.GOOGLE, displayPrice = price)
        }
    }

    // Launches the billing flow for one period and waits for Play to report the result.
    //
    // What comes back is **not** a granted subscription. The core attaches it to the account
    // first, and only an attached purchase is acknowledged with Play.
    suspend fun buy(activity: Activity, plan: AllodiaPlan): AllodiaPurchaseOutcome {
        val details = productDetails() ?: return AllodiaPurchaseOutcome.Unavailable("no products")
        val basePlan =
            catalogue.firstOrNull { it.plan == plan }?.basePlan
                ?: return AllodiaPurchaseOutcome.Unavailable("no base plan")
        // Play identifies which offer to buy by an opaque token, not by the base plan id, and the
        // token is only valid for the ProductDetails object it came from. Holding one across a
        // refetch is how a flow fails with a developer error nobody can reproduce.
        val token =
            details.subscriptionOfferDetails
                ?.firstOrNull { it.basePlanId == basePlan }
                ?.offerToken
                ?: return AllodiaPurchaseOutcome.Unavailable("no offer")

        val awaited = CompletableDeferred<Pair<BillingResult, List<Purchase>>>()
        pendingFlow = awaited
        val params =
            BillingFlowParams.newBuilder()
                .setProductDetailsParamsList(
                    listOf(
                        BillingFlowParams.ProductDetailsParams.newBuilder()
                            .setProductDetails(details)
                            .setOfferToken(token)
                            .build()
                    )
                )
                .build()
        val launched = client.launchBillingFlow(activity, params)
        if (launched.responseCode != BillingClient.BillingResponseCode.OK) {
            pendingFlow = null
            return AllodiaPurchaseOutcome.Unavailable(launched.debugMessage)
        }

        val (result, purchases) = awaited.await()
        return when (result.responseCode) {
            BillingClient.BillingResponseCode.OK -> bought(purchases)
            BillingClient.BillingResponseCode.USER_CANCELED -> AllodiaPurchaseOutcome.Cancelled
            else -> AllodiaPurchaseOutcome.Unavailable(reasonFrom(result))
        }
    }

    // An `OK` result that carries nothing this build can attach is not a purchase, and saying it
    // is would tell somebody they had bought something when nothing was paid for and nothing
    // crosses to the core. `purchasesFrom` drops a payment that has not cleared and a purchase of
    // another product, so an empty list here is one of those rather than a sale.
    private fun bought(purchases: List<Purchase>): AllodiaPurchaseOutcome {
        val attachable = purchases.flatMap(::purchasesFrom)
        return if (attachable.isEmpty()) {
            AllodiaPurchaseOutcome.Unavailable("nothing to attach")
        } else {
            AllodiaPurchaseOutcome.Bought(attachable)
        }
    }

    // Every purchase Play is still reporting, which is every live subscription plus every one
    // nothing has acknowledged yet.
    //
    // Handed to the core whole: the store is the authority on what is outstanding, so a pass is
    // told the set rather than the difference.
    suspend fun outstanding(): List<AllodiaStorePurchase> {
        if (!connect()) return emptyList()
        val params =
            QueryPurchasesParams.newBuilder().setProductType(BillingClient.ProductType.SUBS).build()
        val result = client.queryPurchasesAsync(params)
        if (result.billingResult.responseCode != BillingClient.BillingResponseCode.OK) {
            return emptyList()
        }
        return result.purchasesList.flatMap(::purchasesFrom)
    }

    fun close() {
        client.endConnection()
    }

    private suspend fun productDetails(): ProductDetails? {
        if (!connect()) return null
        val subscriptionId = catalogue.firstOrNull()?.productId ?: return null
        val params =
            QueryProductDetailsParams.newBuilder()
                .setProductList(
                    listOf(
                        QueryProductDetailsParams.Product.newBuilder()
                            .setProductId(subscriptionId)
                            .setProductType(BillingClient.ProductType.SUBS)
                            .build()
                    )
                )
                .build()
        val result = client.queryProductDetails(params)
        if (result.billingResult.responseCode != BillingClient.BillingResponseCode.OK) return null
        return result.productDetailsList?.firstOrNull()
    }

    // The first connection. Transient drops after it are the library's to retry.
    //
    // ⚠️ **A device with no Play Store answers here, and it is not an error.** Billing 9 reports a
    // Play Store that is absent or blocked as `BILLING_UNAVAILABLE` rather than the generic
    // `ERROR` that Billing 8 used, so every non-OK answer is treated the same way: false, and the
    // caller offers the routes that do not need Play.
    private suspend fun connect(): Boolean {
        if (client.isReady) return true
        val connected = CompletableDeferred<Boolean>()
        client.startConnection(
            object : BillingClientStateListener {
                override fun onBillingSetupFinished(result: BillingResult) {
                    connected.complete(
                        result.responseCode == BillingClient.BillingResponseCode.OK
                    )
                }

                override fun onBillingServiceDisconnected() {
                    connected.complete(false)
                }
            }
        )
        return connected.await()
    }

    // Why a flow ended the way it did, for a diagnostic and never for a person to read.
    //
    // Billing 9 adds a sub-response code to the result the purchase listener receives, which is the
    // difference between "the card was declined" and "this offer is not for you". Folded in here so
    // the reason is not lost; what anybody is actually told is a client's to compose, because
    // `debugMessage` is Play's own text and was not written for a reader.
    private fun reasonFrom(result: BillingResult): String =
        when (result.onPurchasesUpdatedSubResponseCode) {
            BillingClient.OnPurchasesUpdatedSubResponseCode
                .PAYMENT_DECLINED_DUE_TO_INSUFFICIENT_FUNDS ->
                "insufficient funds: ${result.debugMessage}"
            BillingClient.OnPurchasesUpdatedSubResponseCode.USER_INELIGIBLE ->
                "not eligible for this offer: ${result.debugMessage}"
            else -> result.debugMessage
        }

    // What the core carries to the account service, one entry per purchase.
    //
    // A Play purchase can name more than one product; ours names one, but reading the list rather
    // than the first entry is what keeps that true rather than assumed.
    private fun purchasesFrom(purchase: Purchase): List<AllodiaStorePurchase> =
        allodiaPurchasesFrom(
            catalogue = catalogue,
            productIds = purchase.products,
            purchaseToken = purchase.purchaseToken,
            purchaseTimeMillis = purchase.purchaseTime,
            isPurchased = purchase.purchaseState == Purchase.PurchaseState.PURCHASED,
        )
}
