// First run: the Allodia-account recommendation above the address field (docs/onboarding.md).
//
// The rule that fails silently: a build with no registration must lose the card, the sign-in line
// AND the divider together. A lone "or connect directly" heading under nothing is what a wrongly
// gated client looks like, and it renders perfectly.
package eu.allodia.mailcal

import androidx.compose.ui.test.assertIsDisplayed
import androidx.compose.ui.test.junit4.v2.createComposeRule
import androidx.compose.ui.test.onNodeWithText
import androidx.compose.ui.test.performClick
import org.junit.Assert.assertEquals
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import uniffi.mailcal_bindings.AccountChangeRefusal
import uniffi.mailcal_bindings.AllodiaAccount
import uniffi.mailcal_bindings.AllodiaAccountKind
import uniffi.mailcal_bindings.AllodiaAccountOffer
import uniffi.mailcal_bindings.AllodiaCard
import uniffi.mailcal_bindings.AllodiaCardState
import uniffi.mailcal_bindings.AllodiaDevices
import uniffi.mailcal_bindings.AllodiaFailure
import uniffi.mailcal_bindings.AllodiaIntent
import uniffi.mailcal_bindings.AllodiaOnboarding
import uniffi.mailcal_bindings.AllodiaSyncFailure

private const val RECOMMENDED = "Recommended"
private const val TITLE = "Create an Allodia account"
private const val CREATE = "Create an account"
private const val HAVE_ONE = "Already have one? Sign in"
private const val DIVIDER = "Or connect a mail account directly"
private const val CANCEL = "Cancel"
private const val NONE_TITLE = "No mail accounts yet"

@RunWith(RobolectricTestRunner::class)
class OnboardingAllodiaCardTest {
    @get:Rule val compose = createComposeRule()

    private val sent = mutableListOf<AllodiaIntent>()
    private val setUp = mutableListOf<String>()

    private fun card(
        offered: Boolean = true,
        card: AllodiaCard = ALLODIA_SIGNED_OUT,
        offers: List<AllodiaAccountOffer>? = emptyList(),
        firstRun: Boolean = true,
    ) {
        compose.setContent {
            OnboardingAllodiaCard(
                offered = offered,
                card = card,
                offers = offers,
                onChange = { sent += it },
                onSetUp = { setUp += it.email },
                firstRun = firstRun,
            )
        }
        compose.waitForIdle()
    }

    private val signedIn = ALLODIA_SIGNED_OUT.copy(
        state = AllodiaCardState.SignedIn(AllodiaAccount(id = null, email = "alice@allodia.eu", name = null)),
        onboarding = AllodiaOnboarding.SignedIn,
    )

    private fun offer(email: String) = AllodiaAccountOffer(
        id = "abc",
        email = email,
        kind = AllodiaAccountKind.GOOGLE,
        host = null,
        port = null,
        security = null,
        smtpHost = null,
        smtpPort = null,
        smtpSecurity = null,
        caldavBaseUrl = null,
        jmapBaseUrl = null,
    )

    @Test
    fun the_recommendation_comes_with_a_way_back_and_a_divider() {
        card()
        compose.onNodeWithText(RECOMMENDED).assertIsDisplayed()
        compose.onNodeWithText(TITLE).assertIsDisplayed()
        compose.onNodeWithText(HAVE_ONE).assertIsDisplayed()
        compose.onNodeWithText(DIVIDER).assertIsDisplayed()
    }

    /**
     * The failure that renders perfectly. Gating only the card would leave a heading naming a
     * choice nobody was offered.
     */
    @Test
    fun a_build_with_no_registration_loses_the_card_the_line_and_the_divider_together() {
        card(offered = false)
        compose.onNodeWithText(TITLE).assertDoesNotExist()
        compose.onNodeWithText(HAVE_ONE).assertDoesNotExist()
        compose.onNodeWithText(DIVIDER).assertDoesNotExist()
    }

    @Test
    fun both_routes_are_their_own_control() {
        card()
        compose.onNodeWithText(CREATE).performClick()
        compose.onNodeWithText(HAVE_ONE).performClick()
        assertEquals(listOf(allodiaSignIn(create = true), allodiaSignIn()), sent)
    }

    /**
     * A sign-in in flight offers neither button: a second browser hop would discard the first
     * flow's verifier.
     */
    @Test
    fun a_sign_in_in_flight_offers_nothing_to_press_twice() {
        card(card = allodiaSigningIn())
        compose.onNodeWithText(CREATE).assertDoesNotExist()
        compose.onNodeWithText(HAVE_ONE).assertDoesNotExist()
        compose.onNodeWithText(DIVIDER).assertIsDisplayed()
    }

    /**
     * The screen nobody can skip must not be able to strand somebody. A sign-in that goes wrong in
     * the browser, or on the service behind it, leaves this card spinning, and on Android the
     * ordinary way out (coming back to the app) only exists once there is a browser to have been
     * dismissed. Before that there is nothing to come back from.
     */
    @Test
    fun a_browser_hop_that_does_not_come_back_can_be_escaped() {
        card(card = allodiaSigningIn(attempt = 4uL, escape = true))
        compose.onNodeWithText(CANCEL).assertIsDisplayed()

        compose.onNodeWithText(CANCEL).performClick()

        assertEquals(listOf<AllodiaIntent>(AllodiaIntent.CancelSignIn(4uL)), sent)
    }

    /**
     * And not before it has earned it: an ordinary hop is in front of the person within a second,
     * so a button drawn for that one is noise on every sign-in that ever works.
     */
    @Test
    fun an_ordinary_hop_draws_no_way_back_it_has_not_earned() {
        card(card = allodiaSigningIn(escape = false))
        compose.onNodeWithText(CANCEL).assertDoesNotExist()
    }

    /** A sign-in that did not work is said under the offer it returns to, in the Settings card's words. */
    @Test
    fun a_sign_in_that_did_not_work_says_so_under_the_offer() {
        card(card = ALLODIA_SIGNED_OUT.copy(failure = AllodiaFailure.SignIn(AccountChangeRefusal.NotFinished)))
        compose.onNodeWithText(HAVE_ONE).assertIsDisplayed()
        compose.onNodeWithText("Sign-in timed out. Please try again.").assertIsDisplayed()
    }

    /**
     * The pass that follows a sign-in is a bounded network call, not a wait on somebody in another
     * application, there is nothing there to escape from.
     */
    @Test
    fun the_pass_after_a_sign_in_offers_no_way_back() {
        card(card = signedIn.copy(onboarding = AllodiaOnboarding.Checking))
        compose.onNodeWithText("Checking your other devices…").assertIsDisplayed()
        compose.onNodeWithText(CANCEL).assertDoesNotExist()
    }

    /**
     * The whole reason to sign in here: the screen that follows is not an empty address field.
     * The offer only fills that field, detection still decides the route.
     */
    @Test
    fun an_account_from_another_device_replaces_the_card_with_a_shortcut() {
        card(card = signedIn, offers = listOf(offer("someone@gmail.com")))
        compose.onNodeWithText(TITLE).assertDoesNotExist()
        compose.onNodeWithText("someone@gmail.com").assertIsDisplayed()

        compose.onNodeWithText("Set up").performClick()

        assertEquals(listOf("someone@gmail.com"), setUp)
    }

    /**
     * Signed in with nothing to bring over is this person's first device. The card must not go on
     * recommending what they already did -- and it must not go quiet either: a divider over an
     * address field with the card gone reads as the sign-in having failed.
     */
    @Test
    fun signing_in_on_a_first_device_says_what_was_found() {
        card(card = signedIn)
        compose.onNodeWithText(TITLE).assertDoesNotExist()
        compose.onNodeWithText(HAVE_ONE).assertDoesNotExist()
        compose.onNodeWithText(NONE_TITLE).assertIsDisplayed()
        compose.onNodeWithText(DIVIDER).assertIsDisplayed()
    }

    /** And an account that did come over is not "no mail accounts yet". */
    @Test
    fun an_offer_is_not_reported_as_an_empty_account() {
        card(card = signedIn, offers = listOf(offer("someone@gmail.com")))
        compose.onNodeWithText(NONE_TITLE).assertDoesNotExist()
    }

    /**
     * The **card** is asked once: somebody who signed in has decided. The **offers** are not a
     * pitch, they are accounts they already have, and gating them with the card left the second
     * of three linked accounts reachable only from a Settings page, while the "Add account…"
     * button beside it asked them to type an address they could have picked from a list.
     */
    @Test
    fun a_later_add_offers_what_is_left_without_pitching_the_card_again() {
        card(card = signedIn, offers = listOf(offer("carol@example.test")), firstRun = false)
        compose.onNodeWithText("carol@example.test").assertIsDisplayed()
        compose.onNodeWithText(DIVIDER).assertIsDisplayed()
        compose.onNodeWithText(TITLE).assertDoesNotExist()
        compose.onNodeWithText(HAVE_ONE).assertDoesNotExist()
    }

    /**
     * Nothing left to offer is the ordinary second add, and draws nothing at all, not even the
     * empty-answer message, which is a first-run sentence about an account with no mail accounts.
     * A divider over nothing is the shape the contract's own rule forbids.
     */
    @Test
    fun a_later_add_with_an_empty_answer_is_the_direct_route_alone() {
        card(card = signedIn, offers = emptyList(), firstRun = false)
        compose.onNodeWithText(DIVIDER).assertDoesNotExist()
        compose.onNodeWithText(NONE_TITLE).assertDoesNotExist()
    }

    /** And the same when no pass has answered, `setContent` runs once, so this is its own case. */
    @Test
    fun a_later_add_with_no_answer_yet_is_the_direct_route_alone() {
        card(card = signedIn, offers = null, firstRun = false)
        compose.onNodeWithText(DIVIDER).assertDoesNotExist()
        compose.onNodeWithText(NONE_TITLE).assertDoesNotExist()
    }

    /**
     * "We have not looked" and "there is nothing" are different answers, and only the second may
     * be put on screen. A pass that failed -- the service down, the network gone -- leaves no
     * report, and saying "no mail accounts yet, add your first" states a result nobody has.
     */
    @Test
    fun a_pass_that_has_not_answered_is_not_an_empty_account() {
        card(card = signedIn, offers = null)
        compose.onNodeWithText(NONE_TITLE).assertDoesNotExist()
        compose.onNodeWithText(DIVIDER).assertIsDisplayed()
    }

    /**
     * What the activity hands the card from the core's record: an answer only an empty pass gives
     * may read as "no mail accounts yet", and a record holding only questions, or only a failure,
     * is no answer about offers at all.
     */
    @Test
    fun only_a_pass_that_found_nothing_is_an_empty_answer() {
        fun devices(nothingFound: Boolean, offers: List<AllodiaAccountOffer> = emptyList()) =
            AllodiaDevices(
                checking = false,
                nothingFound = nothingFound,
                offers = offers,
                questions = emptyList(),
                failure = if (nothingFound) null else AllodiaSyncFailure.UNAVAILABLE,
            )
        val found = listOf(offer("someone@gmail.com"))
        assertEquals(null, onboardingOffers(null))
        assertEquals(emptyList<AllodiaAccountOffer>(), onboardingOffers(devices(nothingFound = true)))
        assertEquals(null, onboardingOffers(devices(nothingFound = false)))
        assertEquals(found, onboardingOffers(devices(nothingFound = false, offers = found)))
    }
}
