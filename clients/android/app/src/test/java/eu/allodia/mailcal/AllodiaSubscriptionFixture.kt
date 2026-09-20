// One subscription answer to build the subscription tests out of, so each test states only the
// part it is about.
package eu.allodia.mailcal

import uniffi.mailcal_bindings.AllodiaBiller
import uniffi.mailcal_bindings.AllodiaOwnStatus
import uniffi.mailcal_bindings.AllodiaOwnSubscription
import uniffi.mailcal_bindings.AllodiaPlan
import uniffi.mailcal_bindings.AllodiaPrices
import uniffi.mailcal_bindings.AllodiaStore
import uniffi.mailcal_bindings.AllodiaStoreStatus
import uniffi.mailcal_bindings.AllodiaStoreSubscription
import uniffi.mailcal_bindings.AllodiaSubscription
import uniffi.mailcal_bindings.AllodiaSubscriptionActions

internal fun subscription(
    entitled: Boolean = false,
    currentPeriodEnd: String? = null,
    own: AllodiaOwnSubscription? = null,
    stores: List<AllodiaStoreSubscription> = emptyList(),
    duplicateBilling: List<AllodiaBiller> = emptyList(),
) = AllodiaSubscription(
    entitled = entitled,
    plan = if (entitled) "pro" else "free",
    currentPeriodEnd = currentPeriodEnd,
    own = own,
    stores = stores,
    duplicateBilling = duplicateBilling,
    // Permissive by default: what a client may offer is the service's answer, so a test about
    // anything else should not have to state it, and the tests that are about it say so.
    actions = AllodiaSubscriptionActions(
        canCancel = true,
        canResubscribe = true,
        canSwitchInterval = true,
        canStartCheckout = !entitled,
    ),
    prices = AllodiaPrices(monthlyInCents = 299, yearlyInCents = 2990, currency = "EUR"),
    checkoutAvailable = true,
)

internal fun storeSubscription(
    source: AllodiaStore = AllodiaStore.GOOGLE,
    status: AllodiaStoreStatus = AllodiaStoreStatus.Active,
    autoRenewing: Boolean = true,
    currentPeriodEnd: String? = null,
    manageUrl: String = "https://play.google.com/store/account/subscriptions",
) = AllodiaStoreSubscription(
    source = source,
    status = status,
    interval = AllodiaPlan.YEARLY,
    productId = "eu.allodia.mailcal.services",
    priceInCents = null,
    currency = null,
    currentPeriodEnd = currentPeriodEnd,
    autoRenewing = autoRenewing,
    environment = "production",
    manageUrl = manageUrl,
)

internal fun ownSubscription(
    status: AllodiaOwnStatus = AllodiaOwnStatus.Active,
    nextPaymentDate: String? = "2027-09-18T00:00:00+02:00",
    interval: AllodiaPlan = AllodiaPlan.YEARLY,
) = AllodiaOwnSubscription(
    status = status,
    interval = interval,
    amountInCents = 2990,
    nextPaymentDate = nextPaymentDate,
    currentPeriodEnd = "2027-09-18T00:00:00+02:00",
    cancelledAt = null,
)

// A service that permits none of the writes, which is what it answers for somebody a store is
// charging: they have a period and a plan, and neither is this API's to change.
internal fun refusing() = AllodiaSubscriptionActions(
    canCancel = false,
    canResubscribe = false,
    canSwitchInterval = false,
    canStartCheckout = false,
)
