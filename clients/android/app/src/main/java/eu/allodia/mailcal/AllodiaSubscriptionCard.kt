// Settings → Allodia account → Subscription: what is being charged, by whom, until when, and the
// way to start one. Its Apple twin is AllodiaSubscriptionSettings.swift; Windows and Linux do not
// draw this yet. Keep the wording in step.
//
// The rules are `purchasing.md`, the contract beside the Allodia Licence, and the core holds every
// one of them. **What this file draws is what the core answered.** Which offers exist and in what
// order is `orderAllodiaOffers`, and a store's subscription is changed only at that store, so the
// button opens `manageUrl` rather than doing anything itself.
//
// ⚠️ **A price is drawn, never parsed and never assembled.** Play requires the store's own
// formatted price be shown, the store's commission is already inside it, and the core carries no
// locale data to format one with.
package eu.allodia.mailcal

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.material3.Card
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.DisposableEffect
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalConfiguration
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.unit.dp
import java.util.Locale
import uniffi.mailcal_bindings.AllodiaOffer
import uniffi.mailcal_bindings.AllodiaPlan
import uniffi.mailcal_bindings.AllodiaStore
import uniffi.mailcal_bindings.AllodiaStoreSubscription
import uniffi.mailcal_bindings.AllodiaSubscription

/**
 * What the card needs, beyond the read itself.
 *
 * Held by the activity rather than the composable because both of its flows leave the app: a Play
 * sheet is another process and Allodia's checkout is a browser, so state this screen owned would
 * not still be here when the person came back.
 *
 * [accountId] is the Allodia account a store purchase would be tagged with, absent on a device
 * that signed in before the id was recorded. [buying] is the period whose sheet is up, so that
 * button alone spins. [note] is the last thing worth saying about an attempt.
 */
internal data class AllodiaSubscriptionUi(
    val state: AllodiaSubscriptionState = AllodiaSubscriptionState.Checking,
    val accountId: String? = null,
    val buying: AllodiaPlan? = null,
    val note: String? = null,
)

/**
 * Drawn only while somebody is signed in: a subscription belongs to an account, and there is
 * nothing to say about one that does not exist.
 */
@Composable
internal fun AllodiaSubscriptionCard(
    ui: AllodiaSubscriptionUi,
    onRefresh: () -> Unit,
    onBuy: (AllodiaPlan) -> Unit,
    onManageStore: (AllodiaStoreSubscription) -> Unit,
    onSignInAgain: () -> Unit,
    onClosed: () -> Unit,
    onCancel: () -> Unit,
    onResubscribe: () -> Unit,
    onSwitch: (AllodiaPlan) -> Unit,
) {
    val ctx = LocalContext.current
    // The read runs when the card appears rather than when the activity connects: it is a network
    // round trip, and a category nobody opened is not worth one.
    LaunchedEffect(Unit) { onRefresh() }
    // ⚠️ **A flow the shop never answers would leave every button disabled behind a spinner that
    // will not stop**, and leaving the card is then the only way back. Letting go of the wait is
    // not abandoning the purchase: Play reports it on its own listener, which starts a pass of its
    // own, and an unattached purchase is refunded rather than lost.
    DisposableEffect(Unit) { onDispose { onClosed() } }
    Card(modifier = Modifier.fillMaxWidth()) {
        Column(modifier = Modifier.fillMaxWidth().padding(16.dp)) {
            Text(
                L10n.settings_subscription_heading(ctx),
                style = MaterialTheme.typography.titleMedium,
                modifier = Modifier.padding(bottom = 8.dp),
            )
            when (val state = ui.state) {
                AllodiaSubscriptionState.Checking ->
                    Row(
                        verticalAlignment = Alignment.CenterVertically,
                        horizontalArrangement = Arrangement.spacedBy(8.dp),
                    ) {
                        CircularProgressIndicator(modifier = Modifier.size(16.dp))
                        Secondary(L10n.settings_subscription_checking(ctx))
                    }
                // Deliberately quiet. A read that did not arrive costs a sentence, never access:
                // whether a capability is on is `allodiaEntitlement`'s answer, and it is local.
                AllodiaSubscriptionState.Unavailable ->
                    Secondary(L10n.settings_subscription_unavailable(ctx))
                // An offer rather than an error: they are signed in and this one read is asleep,
                // and the ordinary sign-in asks for the full current scope set.
                AllodiaSubscriptionState.NeedsReauth -> {
                    Secondary(L10n.settings_subscription_reauth(ctx))
                    TextButton(onClick = onSignInAgain) {
                        Text(L10n.settings_allodia_reauth_action(ctx))
                    }
                }
                is AllodiaSubscriptionState.Loaded ->
                    Loaded(
                        state.view,
                        ui,
                        onBuy,
                        onManageStore,
                        onSignInAgain,
                        onCancel,
                        onResubscribe,
                        onSwitch,
                    )
            }
            ui.note?.let { note -> Secondary(note, modifier = Modifier.padding(top = 8.dp)) }
        }
    }
}

@Composable
private fun Loaded(
    view: AllodiaSubscriptionView,
    ui: AllodiaSubscriptionUi,
    onBuy: (AllodiaPlan) -> Unit,
    onManageStore: (AllodiaStoreSubscription) -> Unit,
    onSignInAgain: () -> Unit,
    onCancel: () -> Unit,
    onResubscribe: () -> Unit,
    onSwitch: (AllodiaPlan) -> Unit,
) {
    val ctx = LocalContext.current
    if (view.subscription.entitled) {
        Active(view.subscription, onManageStore)
        // What may be done to Allodia's own subscription, which the service decided
        // (AllodiaSubscriptionWrites.kt). A store's is changed at that store and nothing here
        // touches it.
        AllodiaSubscriptionWrites(view.subscription, onCancel, onResubscribe, onSwitch)
    } else {
        Secondary(L10n.settings_subscription_free(ctx))
        if (allodiaNeedsReauthToBuy(view.offers, ui.accountId)) {
            Secondary(L10n.settings_subscription_reauth(ctx))
            TextButton(onClick = onSignInAgain) {
                Text(L10n.settings_allodia_reauth_action(ctx))
            }
        }
        Offers(allodiaBuyableOffers(view.offers, ui.accountId), ui.buying, onBuy)
    }
    // ⚠️ Money was taken and nothing has been granted. A card that stays silent here leaves
    // somebody with no way to find that out.
    if (view.anythingStuck) Secondary(L10n.settings_subscription_stuck(ctx))
    // ⚠️ The other ending that owes somebody an explanation: what they bought is on a different
    // Allodia account, so it is never coming to this one and no amount of waiting changes that.
    if (view.claimedElsewhere) Secondary(L10n.settings_subscription_claimed_elsewhere(ctx))
}

/** What a paid account says: who is charging, until when, and where to change it. */
@Composable
private fun Active(
    subscription: AllodiaSubscription,
    onManageStore: (AllodiaStoreSubscription) -> Unit,
) {
    val ctx = LocalContext.current
    val locale: Locale = allodiaReaderLocale()
    val billers = allodiaBillers(subscription)
    billers.firstOrNull()?.let { first ->
        Text(
            L10n.settings_subscription_billed_by(ctx, allodiaBillerName(first)),
            style = MaterialTheme.typography.bodyMedium,
        )
    }
    subscription.currentPeriodEnd?.let { raw ->
        allodiaDate(raw, locale)?.let { end ->
            Secondary(
                if (allodiaWillRenew(subscription)) {
                    L10n.settings_subscription_renews(ctx, end)
                } else {
                    L10n.settings_subscription_ends(ctx, end)
                }
            )
        }
    }
    // Not a lapse, and never drawn as one: the store is retrying and access continues.
    if (allodiaInGrace(subscription)) {
        billers.firstOrNull()?.let { first ->
            Secondary(L10n.settings_subscription_grace(ctx, allodiaBillerName(first)))
        }
    }
    // Reported, never resolved. Cancelling one of them without asking is a decision about somebody
    // else's money, so each exit is offered and none is taken.
    if (subscription.duplicateBilling.isNotEmpty()) {
        Text(
            L10n.settings_subscription_duplicate(
                ctx,
                subscription.duplicateBilling.joinToString(", ", transform = ::allodiaBillerName),
            ),
            style = MaterialTheme.typography.bodySmall,
            color = MaterialTheme.colorScheme.error,
        )
    }
    // A store's subscription is the store's to change, so this opens its page rather than offering
    // a cancel button that would have nothing to call. Review-blocking on both stores, which is why
    // it is drawn for every store that still has something to do rather than for a recognised one.
    allodiaManageableStores(subscription).forEach { store ->
        val biller = if (store.source == AllodiaStore.APPLE) "Apple" else "Google Play"
        TextButton(onClick = { onManageStore(store) }) {
            Text(L10n.settings_subscription_manage(ctx, biller))
        }
    }
}

/** The plans that can be bought here, in the core's order. */
@Composable
private fun Offers(offers: List<AllodiaOffer>, buying: AllodiaPlan?, onBuy: (AllodiaPlan) -> Unit) {
    val ctx = LocalContext.current
    offers.forEach { offer ->
        TextButton(onClick = { onBuy(offer.plan) }, enabled = buying == null) {
            Row(
                verticalAlignment = Alignment.CenterVertically,
                horizontalArrangement = Arrangement.spacedBy(6.dp),
            ) {
                // The label stays while the spinner runs. A button that becomes a bare spinner
                // stops saying what it is doing, which is the wrong half to drop when what it is
                // doing is charging somebody.
                if (buying == offer.plan) CircularProgressIndicator(modifier = Modifier.size(16.dp))
                // **The period is on the button, not under it.** It is the thing being chosen, so
                // a row of buttons that all read "Subscribe" makes the reader pair each one with a
                // line of small print to find out what it does.
                Text(
                    when (offer.plan) {
                        AllodiaPlan.YEARLY ->
                            L10n.settings_subscription_buy_yearly(ctx, offer.displayPrice)
                        AllodiaPlan.MONTHLY ->
                            L10n.settings_subscription_buy_monthly(ctx, offer.displayPrice)
                    }
                )
            }
        }
        // What renewing means, beside the button and not a tap away: both stores make this
        // review-blocking, and somebody agreeing to a recurring charge is owed it whether or not
        // they are.
        Secondary(L10n.settings_subscription_terms(ctx))
    }
}

// The locale this reader is actually being shown, which is the configuration's rather than the JVM
// default: the app's own language picker writes the configuration.
@Composable
internal fun allodiaReaderLocale(): Locale =
    LocalConfiguration.current.locales.takeIf { !it.isEmpty }?.get(0) ?: Locale.getDefault()

@Composable
private fun Secondary(text: String, modifier: Modifier = Modifier) {
    Text(
        text,
        style = MaterialTheme.typography.bodyMedium,
        color = MaterialTheme.colorScheme.onSurfaceVariant,
        modifier = modifier,
    )
}
