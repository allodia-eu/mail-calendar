// Buying a subscription through Google Play, the Play Billing half.
//
// The rules are `purchasing.md`, the contract that ships beside the Allodia Licence, and the core
// holds every one of them. What lives here is the part no Rust can do: asking Play for the
// subscription, launching the billing flow, collecting what Play still reports, and acknowledging
// a purchase once the core says it has been granted.
//
// ⚠️ **This client never acknowledges a purchase.** The account service does it, as the last step
// of attaching the purchase, because acknowledging is the thing that must not happen before the
// subscription exists: Play refunds an unacknowledged purchase after three days, which is exactly
// the right outcome for one that could not be attached. Acknowledging here as well would be a
// second place that could get the order wrong, for no gain.
//
// What is handed over is Play's **purchase token**, which is the whole of what the service takes.
// It reads every fact about the subscription back from the Play Developer API with a service
// account that exists nowhere on a device.
package eu.allodia.mailcal

import android.app.Activity
import android.content.Context
import com.android.billingclient.api.BillingClient
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
// One per app. The connection is re-established on demand rather than held open, because Play
// drops it at will and every call has to cope with that anyway.
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
    // first, and the service acknowledges it with Play as the last step of doing so.
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
            BillingClient.BillingResponseCode.OK ->
                AllodiaPurchaseOutcome.Bought(purchases.flatMap(::purchasesFrom))
            BillingClient.BillingResponseCode.USER_CANCELED -> AllodiaPurchaseOutcome.Cancelled
            else -> AllodiaPurchaseOutcome.Unavailable(result.debugMessage)
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

    private suspend fun connect(): Boolean {
        if (client.isReady) return true
        val connected = CompletableDeferred<Boolean>()
        client.startConnection(
            object : com.android.billingclient.api.BillingClientStateListener {
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
