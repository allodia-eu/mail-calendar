// The words the subscription section puts around the core's card (`allodiaSubscriptionCard`), and
// what buying from Play keeps on the device.
//
// The rules are `purchasing.md`, the contract that ships beside the Allodia Licence, and the core
// holds every one of them: who is billing, whether it renews, which writes exist and what each came
// to. What is here is naming, formatting and the half of buying no Rust can do.
//
// ⚠️ **None of this gates a capability.** `allodiaEntitlement` does, locally and without a network
// call. This is what the screen *says*, so a service that cannot be reached costs somebody a
// sentence, never access they have paid for.
package eu.allodia.mailcal

import android.content.Context
import java.time.LocalDate
import java.time.OffsetDateTime
import java.time.ZoneOffset
import java.time.format.DateTimeFormatter
import java.time.format.FormatStyle
import java.text.NumberFormat
import java.util.Currency
import java.util.Locale
import uniffi.mailcal_bindings.AllodiaBiller
import uniffi.mailcal_bindings.AllodiaCheckoutOffer
import uniffi.mailcal_bindings.AllodiaOffer
import uniffi.mailcal_bindings.AllodiaPlan
import uniffi.mailcal_bindings.AllodiaStore
import uniffi.mailcal_bindings.AllodiaSubscriptionCard
import uniffi.mailcal_bindings.AllodiaSubscriptionOutcome
import uniffi.mailcal_bindings.AllodiaSubscriptionState
import uniffi.mailcal_bindings.AllodiaWriteFailure
import uniffi.mailcal_bindings.ControlState

/** What the section draws before the core has said anything: no section. */
internal val ALLODIA_SUBSCRIPTION_HIDDEN =
    AllodiaSubscriptionCard(state = AllodiaSubscriptionState.Hidden, outcome = null, checkoutPage = null)

/**
 * What buying through this build's shop keeps on the device, beside the core's card.
 *
 * [offers] is what the shop will sell, in the core's order. [accountId] is the Allodia account a
 * store purchase would be tagged with, absent on a device that signed in before the id was
 * recorded. [buying] is the period whose Play sheet is up. [note] is the last thing worth saying
 * about a store purchase. [anythingStuck] says a purchase has waited long enough to be worth saying
 * so: money was taken and nothing granted. [claimedElsewhere] says the last pass found a purchase a
 * **different** Allodia account owns, which nothing here will ever grant.
 */
internal data class AllodiaStoreUi(
    val offers: List<AllodiaOffer> = emptyList(),
    val accountId: String? = null,
    val buying: AllodiaPlan? = null,
    val note: String? = null,
    val anythingStuck: Boolean = false,
    val claimedElsewhere: Boolean = false,
)

// What a biller is called on screen.
//
// **Allodia, never the payment processor behind it.** Naming a processor somebody has never heard
// of, inside a message about being charged twice, is how a correct warning reads as a scam.
internal fun allodiaBillerName(biller: AllodiaBiller): String =
    when (biller) {
        is AllodiaBiller.Allodia -> "Allodia"
        is AllodiaBiller.Apple -> "Apple"
        is AllodiaBiller.Google -> "Google Play"
        // A biller this build cannot name still has to appear, or a "you are being charged twice"
        // warning would name only one of the two.
        is AllodiaBiller.Unknown -> biller.label
    }

/** What a store is called on its page's button. */
internal fun allodiaStoreName(store: AllodiaStore): String =
    when (store) {
        AllodiaStore.APPLE -> "Apple"
        AllodiaStore.GOOGLE -> "Google Play"
        AllodiaStore.ALLODIA -> "Allodia"
    }

// The offers this device may actually go through with, of the ones the shop returned.
//
// ⚠️ **A device that cannot name its account does not buy from a store.** The id a purchase is
// tagged with is the only way to attribute one whose report never arrives, it cannot be added
// afterwards, and a grant stored before the id was recorded has none. Allodia's own checkout needs
// no tag: it creates the subscription against the account the person signs in to on the page.
internal fun allodiaBuyableOffers(offers: List<AllodiaOffer>, accountId: String?): List<AllodiaOffer> =
    if (accountId != null) offers else offers.filter { it.store == AllodiaStore.ALLODIA }

// Whether the section owes somebody the "sign in again" prompt: a store had something to sell and
// this device cannot be the one to buy it.
internal fun allodiaNeedsReauthToBuy(offers: List<AllodiaOffer>, accountId: String?): Boolean =
    accountId == null && offers.any { it.store != AllodiaStore.ALLODIA }

/**
 * The control an offer's button is drawn with. Allodia's checkout is the core's write, so its state
 * is the card's, and an offer the card no longer sells has none; a store's purchase is this
 * device's, held while any Play sheet is up.
 */
internal fun allodiaOfferControl(
    offer: AllodiaOffer,
    checkout: AllodiaCheckoutOffer?,
    buying: AllodiaPlan?,
): ControlState? =
    if (offer.store == AllodiaStore.ALLODIA) {
        checkout?.let { if (offer.plan == AllodiaPlan.YEARLY) it.yearly else it.monthly }
    } else {
        ControlState(enabled = buying == null, running = buying == offer.plan)
    }

// An ISO 8601 date from the account service as a plain calendar date.
//
// Day precision on purpose: a renewal is a date somebody's bank statement will agree with.
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

// Minor units and an ISO 4217 code as something a reader can price.
//
// ⚠️ **Only for what Allodia bills directly.** A store's price is the store's own formatted string,
// drawn and never recomputed; this is the other route, where the service sends 199 and EUR because
// the core carries no locale data at all.
//
// A currency this JVM cannot name formats as a plain number rather than throwing: an amount with no
// symbol is still the right amount, and a screen that crashed over a code would say nothing at all.
internal fun allodiaMinorUnits(minorUnits: Long, currency: String, locale: Locale): String {
    val format = NumberFormat.getCurrencyInstance(locale)
    runCatching { Currency.getInstance(currency) }
        .onSuccess { format.currency = it }
        .onFailure { return NumberFormat.getNumberInstance(locale).format(minorUnits / 100.0) }
    return format.format(minorUnits / 100.0)
}

private fun allodiaDay(raw: String): LocalDate? =
    runCatching { OffsetDateTime.parse(raw).withOffsetSameInstant(ZoneOffset.UTC).toLocalDate() }
        .recoverCatching { LocalDate.parse(raw.take(10)) }
        .getOrNull()

/** What the last write came to, as the sentence under the section. */
internal fun allodiaOutcomeText(ctx: Context, outcome: AllodiaSubscriptionOutcome, locale: Locale): String =
    when (outcome) {
        // ⚠️ The date is the whole reassurance; one this build cannot read is still drawn, since a
        // sentence with no date at all is not true.
        is AllodiaSubscriptionOutcome.Cancelled ->
            L10n.settings_subscription_cancelled(ctx, allodiaDate(outcome.endDate, locale) ?: outcome.endDate)
        AllodiaSubscriptionOutcome.Restarted -> L10n.settings_subscription_resubscribed(ctx)
        // The service's own figures, never today's list price.
        is AllodiaSubscriptionOutcome.Switched ->
            L10n.settings_subscription_switched(
                ctx,
                outcome.nextPaymentDate?.let { allodiaDate(it, locale) }.orEmpty(),
                allodiaMinorUnits(outcome.amountInCents, outcome.currency, locale),
            )
        is AllodiaSubscriptionOutcome.Failed -> allodiaWriteFailureText(ctx, outcome.failure)
    }

// Why a write did not happen. "You have already cancelled" and "that cannot change while a charge
// is being retried" are different things to say, and only the code tells them apart.
internal fun allodiaWriteFailureText(ctx: Context, failure: AllodiaWriteFailure): String =
    when (failure) {
        AllodiaWriteFailure.ALREADY_CANCELLED -> L10n.settings_subscription_refused_already_cancelled(ctx)
        AllodiaWriteFailure.ALREADY_ACTIVE -> L10n.settings_subscription_refused_already_active(ctx)
        AllodiaWriteFailure.ALREADY_ON_INTERVAL -> L10n.settings_subscription_refused_already_on_interval(ctx)
        AllodiaWriteFailure.NOT_SWITCHABLE -> L10n.settings_subscription_refused_not_switchable(ctx)
        AllodiaWriteFailure.NOT_FOUND -> L10n.settings_subscription_refused_not_found(ctx)
        AllodiaWriteFailure.UNEXPLAINED -> L10n.settings_subscription_write_failed(ctx)
    }
