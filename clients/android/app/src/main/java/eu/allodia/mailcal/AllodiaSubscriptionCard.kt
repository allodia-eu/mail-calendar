// Settings → Allodia account → Subscription: what is being charged, by whom, until when, and the
// way to start one. Its Apple twin is AllodiaSubscriptionSettings.swift; keep the wording in step.
//
// **What this file draws is what the core decided** (`allodiaSubscriptionCard`): which writes
// exist, whether one runs, which store pages to offer and what a write came to. A store's
// subscription is changed only at that store, so its button opens the store's page.
//
// ⚠️ **A store's price is drawn, never parsed and never assembled.** Play requires its own
// formatted price be shown, and the store's commission is already inside it.
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
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.unit.dp
import java.util.Locale
import uniffi.mailcal_bindings.AllodiaIntent
import uniffi.mailcal_bindings.AllodiaPlan
import uniffi.mailcal_bindings.AllodiaPrices
import uniffi.mailcal_bindings.AllodiaStorePage
import uniffi.mailcal_bindings.AllodiaSubscriptionCard
import uniffi.mailcal_bindings.AllodiaSubscriptionState
import uniffi.mailcal_bindings.AllodiaSubscriptionView

/** The section as Settings hands it down: the core's card, the shop's half, and what it may ask. */
internal data class AllodiaSubscriptionUi(
    val card: AllodiaSubscriptionCard = ALLODIA_SUBSCRIPTION_HIDDEN,
    val store: AllodiaStoreUi = AllodiaStoreUi(),
    val actions: AllodiaSubscriptionActions = AllodiaSubscriptionActions(),
)

/** What the section can ask of the activity. */
internal class AllodiaSubscriptionActions(
    // The section came on screen, or left it: the activity reads the subscription again on the
    // first and on every return to the app in between.
    val onShown: () -> Unit = {},
    val onClosed: () -> Unit = {},
    // Fetch what this build's shop sells, for an account nobody is charging. [AllodiaPrices] is
    // Allodia's own list, present only while the core sells its checkout.
    val onOffers: (AllodiaPrices?) -> Unit = {},
    val onBuy: (AllodiaPlan) -> Unit = {},
    val onManageStore: (AllodiaStorePage) -> Unit = {},
    val onSignInAgain: () -> Unit = {},
    // A write to Allodia's own subscription, sent once the person confirmed it.
    val onWrite: (AllodiaIntent) -> Unit = {},
)

/** Drawn while the core draws a section: somebody is signed in and the sign-in holds. */
@Composable
internal fun AllodiaSubscriptionSection(
    card: AllodiaSubscriptionCard,
    store: AllodiaStoreUi,
    actions: AllodiaSubscriptionActions,
) {
    val ctx = LocalContext.current
    // ⚠️ **Leaving lets go of a Play sheet the shop never answers**, which would otherwise leave
    // every button held behind a spinner that will not stop. Not abandoning the purchase: Play
    // reports it on its own listener, which starts a pass of its own.
    DisposableEffect(Unit) {
        actions.onShown()
        onDispose { actions.onClosed() }
    }
    val state = card.state
    Card(modifier = Modifier.fillMaxWidth()) {
        Column(modifier = Modifier.fillMaxWidth().padding(16.dp)) {
            Row(
                verticalAlignment = Alignment.CenterVertically,
                horizontalArrangement = Arrangement.spacedBy(8.dp),
                modifier = Modifier.padding(bottom = 8.dp),
            ) {
                Text(L10n.settings_subscription_heading(ctx), style = MaterialTheme.typography.titleMedium)
                // A read beside the answer, which stays drawn meanwhile.
                if (state is AllodiaSubscriptionState.Loaded && state.view.refreshing) {
                    CircularProgressIndicator(
                        modifier = Modifier.size(16.dp).testTag("subscription-refreshing"),
                    )
                }
            }
            when (state) {
                AllodiaSubscriptionState.Checking ->
                    Row(
                        verticalAlignment = Alignment.CenterVertically,
                        horizontalArrangement = Arrangement.spacedBy(8.dp),
                    ) {
                        CircularProgressIndicator(modifier = Modifier.size(16.dp))
                        Secondary(L10n.settings_subscription_checking(ctx))
                    }
                // Deliberately quiet: a read that did not arrive costs a sentence, never access.
                AllodiaSubscriptionState.Unavailable -> Secondary(L10n.settings_subscription_unavailable(ctx))
                // An offer rather than an error: the ordinary sign-in asks for the full scope set.
                AllodiaSubscriptionState.NeedsReauth -> SignInAgain(actions.onSignInAgain)
                is AllodiaSubscriptionState.Loaded -> Loaded(state.view, store, actions)
                AllodiaSubscriptionState.Hidden -> Unit
            }
            card.outcome?.let { outcome ->
                Secondary(
                    allodiaOutcomeText(ctx, outcome, allodiaReaderLocale()),
                    modifier = Modifier.padding(top = 8.dp),
                )
            }
            store.note?.let { note -> Secondary(note, modifier = Modifier.padding(top = 8.dp)) }
        }
    }
}

@Composable
private fun Loaded(view: AllodiaSubscriptionView, store: AllodiaStoreUi, actions: AllodiaSubscriptionActions) {
    val ctx = LocalContext.current
    if (view.entitled) {
        Active(view)
        StorePages(view, actions.onManageStore)
        // What may be done to Allodia's own subscription (AllodiaSubscriptionWrites.kt).
        AllodiaSubscriptionWrites(view, actions.onWrite)
    } else {
        // Asked when the free half appears and again when Allodia's own list moves; a store's
        // offers cost a round trip to the shop, so not on every redraw.
        val prices = view.checkout?.prices
        LaunchedEffect(prices) { actions.onOffers(prices) }
        Secondary(L10n.settings_subscription_free(ctx))
        if (allodiaNeedsReauthToBuy(store.offers, store.accountId)) SignInAgain(actions.onSignInAgain)
        Offers(view, store, actions.onBuy)
        // On hold and paused grant nothing and are put right only at the store.
        StorePages(view, actions.onManageStore)
    }
    // ⚠️ Money was taken and nothing has been granted. A section that stays silent here leaves
    // somebody with no way to find that out.
    if (store.anythingStuck) Secondary(L10n.settings_subscription_stuck(ctx))
    // ⚠️ What they bought is on a different Allodia account, and no amount of waiting changes that.
    if (store.claimedElsewhere) Secondary(L10n.settings_subscription_claimed_elsewhere(ctx))
}

/** What a paid account says: who is charging and until when. */
@Composable
private fun Active(view: AllodiaSubscriptionView) {
    val ctx = LocalContext.current
    val locale: Locale = allodiaReaderLocale()
    view.billedBy?.let { biller ->
        Text(
            L10n.settings_subscription_billed_by(ctx, allodiaBillerName(biller)),
            style = MaterialTheme.typography.bodyMedium,
        )
    }
    view.periodEnd?.let { allodiaDate(it, locale) }?.let { end ->
        Secondary(
            if (view.renews) {
                L10n.settings_subscription_renews(ctx, end)
            } else {
                L10n.settings_subscription_ends(ctx, end)
            }
        )
    }
    // Not a lapse, and never drawn as one: the charge is being retried and access continues.
    if (view.grace) {
        view.billedBy?.let { biller ->
            Secondary(L10n.settings_subscription_grace(ctx, allodiaBillerName(biller)))
        }
    }
    // Reported, never resolved: cancelling one without asking is a decision about somebody's money.
    if (view.duplicateBilling.isNotEmpty()) {
        Text(
            L10n.settings_subscription_duplicate(
                ctx,
                view.duplicateBilling.joinToString(", ", transform = ::allodiaBillerName),
            ),
            style = MaterialTheme.typography.bodySmall,
            color = MaterialTheme.colorScheme.error,
        )
    }
}

/** One button per store with something left to manage there, each opening that store's page. */
@Composable
private fun StorePages(view: AllodiaSubscriptionView, onManageStore: (AllodiaStorePage) -> Unit) {
    val ctx = LocalContext.current
    view.storePages.forEach { page ->
        TextButton(onClick = { onManageStore(page) }) {
            Text(L10n.settings_subscription_manage(ctx, allodiaStoreName(page.store)))
        }
    }
}

/** The plans that can be bought here, in the core's order. */
@Composable
private fun Offers(view: AllodiaSubscriptionView, store: AllodiaStoreUi, onBuy: (AllodiaPlan) -> Unit) {
    val ctx = LocalContext.current
    allodiaBuyableOffers(store.offers, store.accountId).forEach { offer ->
        val control = allodiaOfferControl(offer, view.checkout, store.buying) ?: return@forEach
        // **The period is on the button, not under it.** It is the thing being chosen.
        val label = when (offer.plan) {
            AllodiaPlan.YEARLY -> L10n.settings_subscription_buy_yearly(ctx, offer.displayPrice)
            AllodiaPlan.MONTHLY -> L10n.settings_subscription_buy_monthly(ctx, offer.displayPrice)
        }
        val tag = "subscription-buy-${offer.plan.name.lowercase(Locale.ROOT)}"
        ControlTextButton(label = label, control = control, tag = tag) { onBuy(offer.plan) }
        // What renewing means, beside the button and not a tap away: both stores make this
        // review-blocking.
        Secondary(L10n.settings_subscription_terms(ctx))
    }
}

@Composable
private fun SignInAgain(onSignInAgain: () -> Unit) {
    val ctx = LocalContext.current
    Secondary(L10n.settings_subscription_reauth(ctx))
    TextButton(onClick = onSignInAgain) { Text(L10n.settings_allodia_reauth_action(ctx)) }
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
