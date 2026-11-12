// One subscription section to build the subscription tests out of, so each test states only the
// part it is about.
package eu.allodia.mailcal

import uniffi.mailcal_bindings.AllodiaBiller
import uniffi.mailcal_bindings.AllodiaCheckoutOffer
import uniffi.mailcal_bindings.AllodiaPrices
import uniffi.mailcal_bindings.AllodiaStorePage
import uniffi.mailcal_bindings.AllodiaSubscriptionCard
import uniffi.mailcal_bindings.AllodiaSubscriptionOutcome
import uniffi.mailcal_bindings.AllodiaSubscriptionState
import uniffi.mailcal_bindings.AllodiaSubscriptionView
import uniffi.mailcal_bindings.AllodiaSwitch
import uniffi.mailcal_bindings.ControlState

internal fun allodiaPrices() = AllodiaPrices(monthlyInCents = 299, yearlyInCents = 2990, currency = "EUR")

internal fun allodiaCheckout(
    monthly: ControlState = FREE,
    yearly: ControlState = FREE,
) = AllodiaCheckoutOffer(prices = allodiaPrices(), monthly = monthly, yearly = yearly)

internal fun subscriptionView(
    entitled: Boolean = false,
    billedBy: AllodiaBiller? = null,
    periodEnd: String? = null,
    renews: Boolean = false,
    grace: Boolean = false,
    duplicateBilling: List<AllodiaBiller> = emptyList(),
    storePages: List<AllodiaStorePage> = emptyList(),
    paidThrough: String? = "2027-09-18T00:00:00+02:00",
    switch: AllodiaSwitch? = null,
    resubscribe: ControlState? = null,
    cancel: ControlState? = null,
    checkout: AllodiaCheckoutOffer? = null,
    refreshing: Boolean = false,
) = AllodiaSubscriptionView(
    entitled = entitled,
    billedBy = billedBy,
    periodEnd = periodEnd,
    renews = renews,
    grace = grace,
    duplicateBilling = duplicateBilling,
    storePages = storePages,
    paidThrough = paidThrough,
    switch = switch,
    resubscribe = resubscribe,
    cancel = cancel,
    checkout = checkout,
    refreshing = refreshing,
)

internal fun subscriptionCard(
    state: AllodiaSubscriptionState,
    outcome: AllodiaSubscriptionOutcome? = null,
) = AllodiaSubscriptionCard(state = state, outcome = outcome, checkoutPage = null)

internal fun loadedSubscription(view: AllodiaSubscriptionView, outcome: AllodiaSubscriptionOutcome? = null) =
    subscriptionCard(AllodiaSubscriptionState.Loaded(view), outcome)
