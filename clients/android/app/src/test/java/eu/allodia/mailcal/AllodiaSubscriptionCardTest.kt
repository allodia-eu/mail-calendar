// Settings → Allodia account → Subscription: what the card puts on screen in each of its states,
// and the two things both stores make review-blocking.
//
// The renewal terms beside the buy button and a way to reach the store's own subscription page are
// requirements rather than preferences: an app that omits either is rejected, and neither omission
// looks like anything on screen.
package eu.allodia.mailcal

import androidx.compose.runtime.mutableStateOf
import androidx.compose.ui.test.assertCountEquals
import androidx.compose.ui.test.assertIsDisplayed
import androidx.compose.ui.test.junit4.createComposeRule
import androidx.compose.ui.test.onAllNodesWithText
import androidx.compose.ui.test.onNodeWithText
import androidx.compose.ui.test.performClick
import org.junit.Assert.assertEquals
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import uniffi.mailcal_bindings.AllodiaOffer
import uniffi.mailcal_bindings.AllodiaPlan
import uniffi.mailcal_bindings.AllodiaStore
import uniffi.mailcal_bindings.AllodiaStoreSubscription

@RunWith(RobolectricTestRunner::class)
class AllodiaSubscriptionCardTest {
    @get:Rule val compose = createComposeRule()

    private var refreshes = 0
    private var bought = mutableListOf<AllodiaPlan>()
    private var managed = mutableListOf<AllodiaStoreSubscription>()
    private var signInsAgain = 0
    private var closes = 0
    private var cancels = 0
    private var restarts = 0
    private var switched = mutableListOf<AllodiaPlan>()

    private fun card(ui: AllodiaSubscriptionUi) {
        compose.setContent {
            AllodiaSubscriptionCard(
                ui = ui,
                onRefresh = { refreshes += 1 },
                onBuy = { bought.add(it) },
                onManageStore = { managed.add(it) },
                onSignInAgain = { signInsAgain += 1 },
                onClosed = { closes += 1 },
                onCancel = { cancels += 1 },
                onResubscribe = { restarts += 1 },
                onSwitch = { switched.add(it) },
            )
        }
        compose.waitForIdle()
    }

    private fun loaded(
        subscription: uniffi.mailcal_bindings.AllodiaSubscription,
        offers: List<AllodiaOffer> = emptyList(),
        anythingStuck: Boolean = false,
        accountId: String? = "an-id",
        buying: AllodiaPlan? = null,
    ) = AllodiaSubscriptionUi(
        state = AllodiaSubscriptionState.Loaded(
            AllodiaSubscriptionView(subscription, offers, anythingStuck)
        ),
        accountId = accountId,
        buying = buying,
    )

    /** The read is the card's own, so opening the category is what starts it. */
    @Test
    fun opening_the_card_asks_for_the_subscription() {
        card(AllodiaSubscriptionUi())
        assertEquals(1, refreshes)
    }

    /**
     * ⚠️ Leaving the card is the way back from a flow the shop never answers, which would
     * otherwise leave every button disabled behind a spinner that does not stop. Letting go of the
     * wait is not abandoning the purchase: Play reports it on its own listener.
     */
    @Test
    fun leaving_the_card_lets_go_of_a_purchase_in_flight() {
        val shown = mutableStateOf(true)
        compose.setContent {
            if (shown.value) {
                AllodiaSubscriptionCard(
                    ui = AllodiaSubscriptionUi(buying = AllodiaPlan.YEARLY),
                    onRefresh = {},
                    onBuy = {},
                    onManageStore = {},
                    onSignInAgain = {},
                    onClosed = { closes += 1 },
                onCancel = { cancels += 1 },
                onResubscribe = { restarts += 1 },
                onSwitch = { switched.add(it) },
                )
            }
        }
        compose.waitForIdle()
        assertEquals(0, closes)
        shown.value = false
        compose.waitForIdle()
        assertEquals(1, closes)
    }

    /**
     * A read that did not arrive costs a sentence, never access.
     *
     * Whether a paid capability is on is answered locally and does not pass through here, so the
     * wording must not read as a lapse or an error somebody has to act on.
     */
    @Test
    fun a_read_that_did_not_arrive_says_so_quietly() {
        card(AllodiaSubscriptionUi(state = AllodiaSubscriptionState.Unavailable))
        compose.onNodeWithText(ctx().getString(R.string.settings_subscription_unavailable))
            .assertIsDisplayed()
    }

    /**
     * ⚠️ A sign-in too old to carry the permission this read needs is an offer, not an outage.
     *
     * The two look identical from the failure alone, and the remedies are opposites: waiting fixes
     * an outage and never fixes this one. Observed on a real device, whose grant predated the
     * permission and which was told its subscription could not be checked "right now".
     */
    @Test
    fun a_sign_in_too_old_to_read_the_subscription_offers_a_fresh_one() {
        card(AllodiaSubscriptionUi(state = AllodiaSubscriptionState.NeedsReauth))
        compose.onNodeWithText(ctx().getString(R.string.settings_subscription_reauth))
            .assertIsDisplayed()
        compose.onNodeWithText(ctx().getString(R.string.settings_allodia_reauth_action))
            .performClick()
        assertEquals(1, signInsAgain)
    }

    /**
     * ⚠️ Both stores reject an app that puts a recurring charge behind a button without saying,
     * beside it, that it renews until cancelled.
     */
    @Test
    fun the_renewal_terms_sit_beside_the_buy_button() {
        card(
            loaded(
                subscription(),
                offers = listOf(
                    AllodiaOffer(AllodiaPlan.YEARLY, AllodiaStore.GOOGLE, "€29.90")
                ),
            )
        )
        compose.onNodeWithText("€29.90", substring = true).assertIsDisplayed()
        compose.onNodeWithText(ctx().getString(R.string.settings_subscription_terms))
            .assertIsDisplayed()
    }

    /**
     * The period is on the button rather than in the small print under it: a row of buttons that
     * all read "Subscribe" makes the reader pair each one with a line of text to find out what it
     * does.
     */
    @Test
    fun each_button_names_the_period_it_buys() {
        card(
            loaded(
                subscription(),
                offers = listOf(
                    AllodiaOffer(AllodiaPlan.YEARLY, AllodiaStore.GOOGLE, "€29.90"),
                    AllodiaOffer(AllodiaPlan.MONTHLY, AllodiaStore.GOOGLE, "€2.99"),
                ),
            )
        )
        compose.onNodeWithText(
            ctx().getString(R.string.settings_subscription_buy_monthly, "€2.99")
        ).performClick()
        assertEquals(listOf(AllodiaPlan.MONTHLY), bought)
    }

    /**
     * ⚠️ A device that cannot name its account is asked to sign in again rather than sold a
     * purchase nothing can attribute.
     */
    @Test
    fun a_device_with_no_account_id_is_asked_to_sign_in_rather_than_sold_anything() {
        card(
            loaded(
                subscription(),
                offers = listOf(
                    AllodiaOffer(AllodiaPlan.YEARLY, AllodiaStore.GOOGLE, "€29.90")
                ),
                accountId = null,
            )
        )
        compose.onNodeWithText(ctx().getString(R.string.settings_subscription_reauth))
            .assertIsDisplayed()
        compose.onNodeWithText(ctx().getString(R.string.settings_allodia_reauth_action))
            .performClick()
        assertEquals(1, signInsAgain)
        assertEquals(emptyList<AllodiaPlan>(), bought)
    }

    /**
     * ⚠️ A store's subscription is changed only at that store, so the card has to offer the way
     * there. Review-blocking on both stores, and drawn for every store subscription rather than
     * for a recognised one.
     */
    @Test
    fun a_paid_account_can_reach_the_store_that_bills_it() {
        val store = storeSubscription(currentPeriodEnd = "2027-09-18T00:00:00+02:00")
        card(loaded(subscription(entitled = true, stores = listOf(store))))
        compose.onNodeWithText(
            ctx().getString(R.string.settings_subscription_manage, "Google Play")
        ).performClick()
        assertEquals(listOf(store), managed)
    }

    /**
     * ⚠️ Cancelling asks first, and the way out never says "Cancel".
     *
     * On a dialog headed "Cancel your subscription?" that word is the thing being asked about, so a
     * dismiss button carrying it is a coin toss. The confirmation also has to name the date, or it
     * reads as "it stops now", which is the one thing cancelling does not do.
     */
    @Test
    fun cancelling_asks_first_and_says_what_is_kept() {
        card(loaded(subscription(entitled = true, own = ownSubscription())))
        compose.onNodeWithText(ctx().getString(R.string.settings_subscription_cancel)).performClick()
        assertEquals("nothing may happen before the question is answered", 0, cancels)

        compose.onNodeWithText(ctx().getString(R.string.settings_subscription_cancel_title))
            .assertIsDisplayed()
        compose.onNodeWithText("2027", substring = true).assertIsDisplayed()
        compose.onNodeWithText(ctx().getString(R.string.settings_subscription_cancel_keep))
            .performClick()
        assertEquals("dismissing cancels nothing", 0, cancels)
    }

    /**
     * ⚠️ The switch confirmation quotes no price.
     *
     * What this subscriber is charged is not today's list price, because a price change never
     * reaches somebody who already subscribed. The amount comes back from the switch itself, so
     * saying one beforehand would be telling a long-standing subscriber a number they will not pay.
     */
    @Test
    fun switching_period_asks_without_quoting_a_price() {
        card(
            loaded(
                subscription(
                    entitled = true,
                    own = ownSubscription(interval = AllodiaPlan.MONTHLY),
                )
            )
        )
        compose.onNodeWithText(ctx().getString(R.string.settings_subscription_switch_yearly))
            .performClick()
        compose.onNodeWithText(ctx().getString(R.string.settings_subscription_switch_title))
            .assertIsDisplayed()
        compose.onAllNodesWithText("€", substring = true).assertCountEquals(0)

        compose.onNodeWithText(ctx().getString(R.string.action_update)).performClick()
        assertEquals(listOf(AllodiaPlan.YEARLY), switched)
    }

    /**
     * A store's subscription is the store's to change, so none of the three writes is offered for
     * one: the service refuses them, and the card draws what the service permits.
     */
    @Test
    fun a_store_billed_account_is_offered_none_of_the_writes() {
        card(
            loaded(
                subscription(entitled = true, stores = listOf(storeSubscription()))
                    .copy(actions = refusing())
            )
        )
        compose.onAllNodesWithText(ctx().getString(R.string.settings_subscription_cancel))
            .assertCountEquals(0)
        compose.onAllNodesWithText(ctx().getString(R.string.settings_subscription_switch_yearly))
            .assertCountEquals(0)
    }

    /**
     * ⚠️ Money was taken and nothing was granted. A card that stays silent leaves somebody with no
     * way to find that out.
     */
    @Test
    fun a_purchase_that_has_not_gone_through_is_said_out_loud() {
        card(loaded(subscription(), anythingStuck = true))
        compose.onNodeWithText(ctx().getString(R.string.settings_subscription_stuck))
            .assertIsDisplayed()
    }

    private fun ctx() = org.robolectric.RuntimeEnvironment.getApplication()
}
