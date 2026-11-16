// Settings → Allodia account → Subscription: what the section puts on screen from the core's card,
// and the two things both stores make review-blocking.
//
// The renewal terms beside the buy button and a way to reach the store's own subscription page are
// requirements rather than preferences: an app that omits either is rejected, and neither omission
// looks like anything on screen.
package eu.allodia.mailcal

import androidx.compose.runtime.mutableStateOf
import androidx.compose.ui.test.assertCountEquals
import androidx.compose.ui.test.assertIsDisplayed
import androidx.compose.ui.test.assertIsEnabled
import androidx.compose.ui.test.assertIsNotEnabled
import androidx.compose.ui.test.junit4.v2.createComposeRule
import androidx.compose.ui.test.onAllNodesWithTag
import androidx.compose.ui.test.onAllNodesWithText
import androidx.compose.ui.test.onNodeWithTag
import androidx.compose.ui.test.onNodeWithText
import androidx.compose.ui.test.performClick
import org.junit.Assert.assertEquals
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import uniffi.mailcal_bindings.AllodiaBiller
import uniffi.mailcal_bindings.AllodiaIntent
import uniffi.mailcal_bindings.AllodiaOffer
import uniffi.mailcal_bindings.AllodiaPlan
import uniffi.mailcal_bindings.AllodiaPrices
import uniffi.mailcal_bindings.AllodiaStore
import uniffi.mailcal_bindings.AllodiaStorePage
import uniffi.mailcal_bindings.AllodiaSubscriptionCard
import uniffi.mailcal_bindings.AllodiaSubscriptionOutcome
import uniffi.mailcal_bindings.AllodiaSubscriptionState
import uniffi.mailcal_bindings.AllodiaSwitch
import uniffi.mailcal_bindings.AllodiaWriteFailure
import uniffi.mailcal_bindings.ControlState

private val RUNNING = ControlState(enabled = false, running = true)
private const val PLAY = "https://play.google.com/store/account/subscriptions"

@RunWith(RobolectricTestRunner::class)
class AllodiaSubscriptionCardTest {
    @get:Rule val compose = createComposeRule()

    private var shown = 0
    private var closes = 0
    private val offersAsked = mutableListOf<AllodiaPrices?>()
    private val bought = mutableListOf<AllodiaPlan>()
    private val managed = mutableListOf<AllodiaStorePage>()
    private var signInsAgain = 0
    private val written = mutableListOf<AllodiaIntent>()

    private val actions = AllodiaSubscriptionActions(
        onShown = { shown += 1 },
        onClosed = { closes += 1 },
        onOffers = { offersAsked.add(it) },
        onBuy = { bought.add(it) },
        onManageStore = { managed.add(it) },
        onSignInAgain = { signInsAgain += 1 },
        onWrite = { written.add(it) },
    )

    private val drawn = mutableStateOf<AllodiaSubscriptionCard?>(null)
    private val drawnStore = mutableStateOf(AllodiaStoreUi())

    /** Draws [card], or redraws the section already drawn with it. */
    private fun section(card: AllodiaSubscriptionCard, store: AllodiaStoreUi = AllodiaStoreUi(accountId = "an-id")) {
        val first = drawn.value == null
        drawn.value = card
        drawnStore.value = store
        if (first) {
            compose.setContent { drawn.value?.let { AllodiaSubscriptionSection(it, drawnStore.value, actions) } }
        }
        compose.waitForIdle()
    }

    private fun text(id: Int, vararg args: Any) = ctx().getString(id, *args)

    /** The page asks the core for a read when it opens, and lets go of a Play sheet when it closes. */
    @Test
    fun opening_the_section_asks_for_the_subscription_and_leaving_lets_go() {
        val present = mutableStateOf(true)
        compose.setContent {
            if (present.value) {
                AllodiaSubscriptionSection(subscriptionCard(AllodiaSubscriptionState.Checking), AllodiaStoreUi(), actions)
            }
        }
        compose.waitForIdle()
        assertEquals(1, shown)
        assertEquals(0, closes)
        compose.onNodeWithText(text(R.string.settings_subscription_checking)).assertIsDisplayed()
        present.value = false
        compose.waitForIdle()
        assertEquals(1, closes)
    }

    /** A read that did not arrive costs a sentence, never access, so it never reads as a lapse. */
    @Test
    fun a_read_that_did_not_arrive_says_so_quietly() {
        section(subscriptionCard(AllodiaSubscriptionState.Unavailable))
        compose.onNodeWithText(text(R.string.settings_subscription_unavailable)).assertIsDisplayed()
    }

    /** ⚠️ A sign-in too old to carry the permission the read needs is an offer, not an outage. */
    @Test
    fun a_sign_in_too_old_to_read_the_subscription_offers_a_fresh_one() {
        section(subscriptionCard(AllodiaSubscriptionState.NeedsReauth))
        compose.onNodeWithText(text(R.string.settings_subscription_reauth)).assertIsDisplayed()
        compose.onNodeWithText(text(R.string.settings_allodia_reauth_action)).performClick()
        assertEquals(1, signInsAgain)
    }

    /** A read beside an answer keeps the answer and says it is running. */
    @Test
    fun a_read_beside_an_answer_keeps_it_drawn() {
        section(loadedSubscription(subscriptionView(entitled = true, billedBy = AllodiaBiller.Allodia, refreshing = true)))
        compose.onNodeWithTag("subscription-refreshing").assertIsDisplayed()
        compose.onNodeWithText(text(R.string.settings_subscription_billed_by, "Allodia")).assertIsDisplayed()
    }

    /**
     * The free half asks this build's shop what it sells, handing it Allodia's own list while the
     * core sells the checkout, and ⚠️ puts the renewal terms beside the buy button.
     */
    @Test
    fun the_free_half_asks_for_offers_and_puts_the_terms_beside_them() {
        val store = AllodiaStoreUi(
            offers = listOf(AllodiaOffer(AllodiaPlan.YEARLY, AllodiaStore.GOOGLE, "€29.90")),
            accountId = "an-id",
        )
        section(loadedSubscription(subscriptionView(checkout = allodiaCheckout())), store)
        assertEquals(listOf<AllodiaPrices?>(allodiaPrices()), offersAsked)
        compose.onNodeWithText("€29.90", substring = true).assertIsDisplayed()
        compose.onNodeWithText(text(R.string.settings_subscription_terms)).assertIsDisplayed()
    }

    /** The period is on the button rather than in the small print under it. */
    @Test
    fun each_button_names_the_period_it_buys() {
        val store = AllodiaStoreUi(
            offers = listOf(
                AllodiaOffer(AllodiaPlan.YEARLY, AllodiaStore.GOOGLE, "€29.90"),
                AllodiaOffer(AllodiaPlan.MONTHLY, AllodiaStore.GOOGLE, "€2.99"),
            ),
            accountId = "an-id",
        )
        section(loadedSubscription(subscriptionView()), store)
        compose.onNodeWithText(text(R.string.settings_subscription_buy_monthly, "€2.99")).performClick()
        assertEquals(listOf(AllodiaPlan.MONTHLY), bought)
    }

    /** A Play sheet that is up holds both buttons and spins on the period being bought. */
    @Test
    fun a_store_purchase_in_flight_spins_on_its_own_button() {
        val store = AllodiaStoreUi(
            offers = listOf(
                AllodiaOffer(AllodiaPlan.YEARLY, AllodiaStore.GOOGLE, "€29.90"),
                AllodiaOffer(AllodiaPlan.MONTHLY, AllodiaStore.GOOGLE, "€2.99"),
            ),
            accountId = "an-id",
            buying = AllodiaPlan.YEARLY,
        )
        section(loadedSubscription(subscriptionView()), store)
        compose.onNodeWithTag("subscription-buy-yearly-running", useUnmergedTree = true).assertIsDisplayed()
        compose.onAllNodesWithTag("subscription-buy-monthly-running", useUnmergedTree = true).assertCountEquals(0)
        compose.onNodeWithTag("subscription-buy-monthly").assertIsNotEnabled()
    }

    /**
     * Allodia's checkout is drawn from the card: its button runs while the core's checkout runs,
     * and is gone once the card stops selling it, whatever the shop last priced.
     */
    @Test
    fun the_checkout_follows_the_card() {
        val store = AllodiaStoreUi(
            offers = listOf(
                AllodiaOffer(AllodiaPlan.YEARLY, AllodiaStore.ALLODIA, "€29.90"),
                AllodiaOffer(AllodiaPlan.MONTHLY, AllodiaStore.ALLODIA, "€2.99"),
            ),
        )
        section(loadedSubscription(subscriptionView(checkout = allodiaCheckout(monthly = HELD, yearly = RUNNING))), store)
        compose.onNodeWithTag("subscription-buy-yearly-running", useUnmergedTree = true).assertIsDisplayed()
        compose.onNodeWithTag("subscription-buy-monthly").assertIsNotEnabled()

        section(loadedSubscription(subscriptionView(checkout = null)), store)
        compose.onAllNodesWithTag("subscription-buy-yearly").assertCountEquals(0)
        compose.onAllNodesWithTag("subscription-buy-monthly").assertCountEquals(0)
    }

    /** ⚠️ A device that cannot name its account is asked to sign in again rather than sold anything. */
    @Test
    fun a_device_with_no_account_id_is_asked_to_sign_in_rather_than_sold_anything() {
        val store = AllodiaStoreUi(offers = listOf(AllodiaOffer(AllodiaPlan.YEARLY, AllodiaStore.GOOGLE, "€29.90")))
        section(loadedSubscription(subscriptionView()), store)
        compose.onNodeWithText(text(R.string.settings_subscription_reauth)).assertIsDisplayed()
        compose.onAllNodesWithTag("subscription-buy-yearly").assertCountEquals(0)
        compose.onNodeWithText(text(R.string.settings_allodia_reauth_action)).performClick()
        assertEquals(1, signInsAgain)
    }

    /** ⚠️ A store's subscription is changed only at that store, so the section offers the way there. */
    @Test
    fun a_paid_account_can_reach_the_store_that_bills_it() {
        val page = AllodiaStorePage(AllodiaStore.GOOGLE, PLAY)
        section(loadedSubscription(subscriptionView(entitled = true, billedBy = AllodiaBiller.Google, storePages = listOf(page))))
        compose.onNodeWithText(text(R.string.settings_subscription_manage, "Google Play")).performClick()
        assertEquals(listOf(page), managed)
    }

    /** ⚠️ On hold and paused grant nothing and are put right only at the store: the page stays. */
    @Test
    fun the_free_half_still_offers_a_store_with_something_to_fix() {
        val page = AllodiaStorePage(AllodiaStore.GOOGLE, PLAY)
        section(loadedSubscription(subscriptionView(storePages = listOf(page))))
        compose.onNodeWithText(text(R.string.settings_subscription_free)).assertIsDisplayed()
        compose.onNodeWithText(text(R.string.settings_subscription_manage, "Google Play")).performClick()
        assertEquals(listOf(page), managed)
    }

    /** ⚠️ Cancelling asks first, names the date, and the way out never says "Cancel". */
    @Test
    fun cancelling_asks_first_and_says_what_is_kept() {
        section(loadedSubscription(subscriptionView(entitled = true, cancel = FREE)))
        compose.onNodeWithText(text(R.string.settings_subscription_cancel)).performClick()
        assertEquals("nothing may happen before the question is answered", emptyList<AllodiaIntent>(), written)
        compose.onNodeWithText(text(R.string.settings_subscription_cancel_title)).assertIsDisplayed()
        compose.onNodeWithText("2027", substring = true).assertIsDisplayed()
        compose.onNodeWithText(text(R.string.settings_subscription_cancel_keep)).performClick()
        assertEquals("dismissing cancels nothing", emptyList<AllodiaIntent>(), written)

        compose.onNodeWithText(text(R.string.settings_subscription_cancel)).performClick()
        compose.onNodeWithTag("subscription-confirm").performClick()
        assertEquals(listOf<AllodiaIntent>(AllodiaIntent.CancelSubscription), written)
    }

    /** ⚠️ The switch confirmation quotes no price: today's list is not what a subscriber pays. */
    @Test
    fun switching_period_asks_without_quoting_a_price() {
        section(loadedSubscription(subscriptionView(entitled = true, switch = AllodiaSwitch(AllodiaPlan.YEARLY, FREE))))
        compose.onNodeWithText(text(R.string.settings_subscription_switch_yearly)).performClick()
        compose.onNodeWithText(text(R.string.settings_subscription_switch_title)).assertIsDisplayed()
        compose.onAllNodesWithText("€", substring = true).assertCountEquals(0)
        compose.onNodeWithText(text(R.string.action_update)).performClick()
        assertEquals(listOf<AllodiaIntent>(AllodiaIntent.SwitchInterval(AllodiaPlan.YEARLY)), written)
    }

    /** A running write spins on its own button and holds every other one, as the core says. */
    @Test
    fun a_running_write_spins_on_its_button_and_holds_the_rest() {
        section(
            loadedSubscription(
                subscriptionView(
                    entitled = true,
                    switch = AllodiaSwitch(AllodiaPlan.MONTHLY, HELD),
                    resubscribe = HELD,
                    cancel = RUNNING,
                )
            )
        )
        compose.onNodeWithTag("subscription-cancel-running", useUnmergedTree = true).assertIsDisplayed()
        compose.onNodeWithTag("subscription-cancel").assertIsNotEnabled()
        compose.onNodeWithTag("subscription-switch").assertIsNotEnabled()
        compose.onNodeWithTag("subscription-resubscribe").assertIsNotEnabled()
        compose.onAllNodesWithTag("subscription-switch-running", useUnmergedTree = true).assertCountEquals(0)
    }

    /** What the card does not offer is not drawn: a store's subscription is that store's to change. */
    @Test
    fun a_store_billed_account_is_offered_none_of_the_writes() {
        section(loadedSubscription(subscriptionView(entitled = true, billedBy = AllodiaBiller.Google)))
        compose.onAllNodesWithTag("subscription-cancel").assertCountEquals(0)
        compose.onAllNodesWithTag("subscription-switch").assertCountEquals(0)
        compose.onAllNodesWithTag("subscription-resubscribe").assertCountEquals(0)
    }

    /** A restart the card offers is one press, asked nothing. */
    @Test
    fun restarting_sends_the_write() {
        section(loadedSubscription(subscriptionView(entitled = true, resubscribe = FREE)))
        compose.onNodeWithTag("subscription-resubscribe").assertIsEnabled().performClick()
        assertEquals(listOf<AllodiaIntent>(AllodiaIntent.Resubscribe), written)
    }

    /** ⚠️ What a write came to is said with the date it keeps, or the refusal's own sentence. */
    @Test
    fun what_a_write_came_to_is_said_under_the_section() {
        val paid = subscriptionView(entitled = true)
        section(loadedSubscription(paid, AllodiaSubscriptionOutcome.Cancelled("2027-09-18T00:00:00+02:00")))
        compose.onNodeWithText("2027", substring = true).assertIsDisplayed()

        section(loadedSubscription(paid, AllodiaSubscriptionOutcome.Failed(AllodiaWriteFailure.NOT_SWITCHABLE)))
        compose.onNodeWithText(text(R.string.settings_subscription_refused_not_switchable)).assertIsDisplayed()

        section(loadedSubscription(paid, AllodiaSubscriptionOutcome.Failed(AllodiaWriteFailure.UNEXPLAINED)))
        compose.onNodeWithText(text(R.string.settings_subscription_write_failed)).assertIsDisplayed()

        section(loadedSubscription(paid, AllodiaSubscriptionOutcome.Switched("2027-09-18", 2990, "EUR")))
        compose.onNodeWithText("29.90", substring = true).assertIsDisplayed()
    }

    /** ⚠️ A purchase owned by a different Allodia account, and one not yet through, are said out loud. */
    @Test
    fun a_purchase_that_has_not_reached_this_account_is_said_out_loud() {
        section(loadedSubscription(subscriptionView()), AllodiaStoreUi(claimedElsewhere = true, anythingStuck = true))
        compose.onNodeWithText(text(R.string.settings_subscription_claimed_elsewhere)).assertIsDisplayed()
        compose.onNodeWithText(text(R.string.settings_subscription_stuck)).assertIsDisplayed()
    }

    private fun ctx() = org.robolectric.RuntimeEnvironment.getApplication()
}
