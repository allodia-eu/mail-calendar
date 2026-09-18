// What the subscription card has to draw, and the reading of a core answer that decides it.
//
// The rules are `purchasing.md`, the contract that ships beside the Allodia Licence, and the core
// holds every one of them. What is here is the few things a screen can be in, plus the sentences
// that follow from one answer. Its Apple twin is MailcalModel.AllodiaSubscription.swift: keep the
// states and the wording in step.
//
// ⚠️ **None of this gates a capability.** `allodiaEntitlement` does, locally and without a network
// call. This is what the screen *says*, so a service that cannot be reached costs somebody a
// sentence, never access they have paid for.
//
// Everything here is a plain function over plain data, so the sentence a person reads about being
// charged twice is decided in a JVM test rather than by composing a screen.
package eu.allodia.mailcal

import java.time.LocalDate
import java.time.OffsetDateTime
import java.time.ZoneOffset
import java.time.format.DateTimeFormatter
import java.time.format.FormatStyle
import java.util.Locale
import uniffi.mailcal_bindings.AllodiaBiller
import uniffi.mailcal_bindings.AllodiaOffer
import uniffi.mailcal_bindings.AllodiaOwnStatus
import uniffi.mailcal_bindings.AllodiaStore
import uniffi.mailcal_bindings.AllodiaStoreStatus
import uniffi.mailcal_bindings.AllodiaSubscription

// What the subscription section has to draw.
internal sealed interface AllodiaSubscriptionState {
    // The read is in flight. Distinct from [Unavailable]: nothing has been learned yet, and a
    // spinner and a failure are different things to put in front of somebody.
    data object Checking : AllodiaSubscriptionState

    // The read did not come back, or this deployment has no billing configured.
    data object Unavailable : AllodiaSubscriptionState

    // The read could not be made at all, because this device's sign-in predates the permission it
    // needs. An offer rather than an error: they are signed in, one thing is asleep, and the
    // ordinary sign-in asks for the full current scope set. Kept apart from [Unavailable] because
    // the remedies differ: waiting fixes an outage and never fixes this.
    data object NeedsReauth : AllodiaSubscriptionState

    // The service answered.
    data class Loaded(val view: AllodiaSubscriptionView) : AllodiaSubscriptionState
}

// The answer, with the parts a screen needs alongside it.
internal data class AllodiaSubscriptionView(
    val subscription: AllodiaSubscription,
    // What this build's shop will sell, in the core's order. Empty while something is already
    // being charged: the service refuses a second subscription for one plan, so offering one would
    // be drawing a button whose only outcome is a refusal.
    val offers: List<AllodiaOffer>,
    // Whether some purchase has been waiting long enough to be worth saying so. Money was taken
    // and nothing has been granted, and a card that draws nothing here leaves the person with no
    // way to find that out.
    val anythingStuck: Boolean,
)

// What a biller is called on screen.
//
// **Allodia, never the payment processor behind it.** Naming a processor somebody has never heard
// of, inside a message about being charged twice, is how a correct warning reads as a scam. The
// bare company name is right here, where the sentence is about who is taking the money rather than
// about the app.
internal fun allodiaBillerName(biller: AllodiaBiller): String =
    when (biller) {
        is AllodiaBiller.Allodia -> "Allodia"
        is AllodiaBiller.Apple -> "Apple"
        is AllodiaBiller.Google -> "Google Play"
        // A biller this build cannot name still has to appear, or a "you are being charged twice"
        // warning would name only one of the two.
        is AllodiaBiller.Unknown -> biller.label
    }

// Everyone charging for this account right now, in a stable order: Allodia's own billing first,
// then each store in the order the service listed them.
//
// ⚠️ **"Right now" is the whole of it, and reading every store the service listed was a bug.** The
// list keeps a subscription after it ends, so an account that bought at one store and later at
// another carries both, and naming the first named the dead one: a device billed by Google Play
// was told it was billed by Apple, whose sandbox subscription had run out hours earlier. A store
// that has stopped charging still gets its manage button, because somebody may want its receipt;
// it does not get to be the answer to "who is taking my money".
internal fun allodiaBillers(subscription: AllodiaSubscription): List<AllodiaBiller> =
    buildList {
        if (subscription.own != null && subscription.own?.status != AllodiaOwnStatus.PendingFirstPayment) {
            add(AllodiaBiller.Allodia)
        }
        subscription.stores.filter { allodiaStoreIsLive(it.status) }.forEach { store ->
            add(
                when (store.source) {
                    AllodiaStore.APPLE -> AllodiaBiller.Apple
                    AllodiaStore.GOOGLE -> AllodiaBiller.Google
                    // The service never reports its own billing as a store, so this arm is
                    // unreachable rather than meaningful. Saying "Allodia" is still the right
                    // answer if it ever is reached.
                    AllodiaStore.ALLODIA -> AllodiaBiller.Allodia
                }
            )
        }
    }

// Whether a store subscription is one somebody is still on: renewing, being retried, or cancelled
// and running out the period already paid for.
//
// The rest grant nothing, and `AllodiaStoreStatus` is deliberately read arm by arm rather than by
// exclusion: a status this build does not know is **never read as permission**, which is the rule
// the core states about the same enum.
private fun allodiaStoreIsLive(status: AllodiaStoreStatus): Boolean =
    when (status) {
        AllodiaStoreStatus.Active, AllodiaStoreStatus.Grace, AllodiaStoreStatus.Cancelled -> true
        AllodiaStoreStatus.OnHold,
        AllodiaStoreStatus.Paused,
        AllodiaStoreStatus.Expired,
        AllodiaStoreStatus.Revoked -> false
        else -> false
    }

// Whether anything will charge again, which decides between "renews on" and "runs until".
//
// A store subscription says so itself; Allodia's own says so by still having a next payment.
// Deliberately not a reading of `entitled`, which answers a different question.
internal fun allodiaWillRenew(subscription: AllodiaSubscription): Boolean {
    if (subscription.stores.any { it.autoRenewing }) return true
    val own = subscription.own ?: return false
    return own.status == AllodiaOwnStatus.Active && !own.nextPaymentDate.isNullOrEmpty()
}

// Whether a charge has failed and is being retried. **Access continues**, so this is never drawn
// as a lapse.
internal fun allodiaInGrace(subscription: AllodiaSubscription): Boolean =
    subscription.stores.any { it.status == AllodiaStoreStatus.Grace } ||
        subscription.own?.status == AllodiaOwnStatus.PastDue

// The offers this device may actually go through with, of the ones the shop returned.
//
// ⚠️ **A device that cannot name its account does not buy from a store.** The id a purchase is
// tagged with is the only way to attribute one whose report never arrives, it cannot be added
// afterwards, and a grant stored before the id was recorded has none. Allodia's own checkout needs
// no tag: it creates the subscription against the account the person signs in to on the page, so
// the `foss` build keeps its offers either way.
internal fun allodiaBuyableOffers(offers: List<AllodiaOffer>, accountId: String?): List<AllodiaOffer> =
    if (accountId != null) offers else offers.filter { it.store == AllodiaStore.ALLODIA }

// Whether the card owes somebody the "sign in again" prompt: a store had something to sell and
// this device cannot be the one to buy it.
internal fun allodiaNeedsReauthToBuy(offers: List<AllodiaOffer>, accountId: String?): Boolean =
    accountId == null && offers.any { it.store != AllodiaStore.ALLODIA }

// An ISO 8601 date from the account service as a plain calendar date.
//
// Day precision on purpose: a renewal is a date somebody's bank statement will agree with, and an
// hour and minute in this sentence would invite a comparison with a clock that means nothing here.
//
// ⚠️ The account service is not the sync engine and does not send the engine's `Z`-suffixed
// instants: an offset like `+02:00`, and a plain day for a period that ends on one, both arrive.
// A shape this build cannot read still has the right day in its leading ten characters, so the
// sentence stays true and only stops being localised.
internal fun allodiaDate(raw: String, locale: Locale): String? {
    if (raw.isEmpty()) return null
    val day = allodiaDay(raw) ?: return raw.take(10)
    return day.format(DateTimeFormatter.ofLocalizedDate(FormatStyle.LONG).withLocale(locale))
}

private fun allodiaDay(raw: String): LocalDate? =
    runCatching { OffsetDateTime.parse(raw).withOffsetSameInstant(ZoneOffset.UTC).toLocalDate() }
        .recoverCatching { LocalDate.parse(raw.take(10)) }
        .getOrNull()
