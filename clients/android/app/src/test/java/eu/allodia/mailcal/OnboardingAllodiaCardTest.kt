// First run: the Allodia-account recommendation above the address field (docs/onboarding.md).
//
// The rule that fails silently: a build with no registration must lose the card, the sign-in line
// AND the divider together. A lone "or connect directly" heading under nothing is what a wrongly
// gated client looks like, and it renders perfectly.
package eu.allodia.mailcal

import androidx.compose.ui.test.assertIsDisplayed
import androidx.compose.ui.test.junit4.createComposeRule
import androidx.compose.ui.test.onNodeWithText
import androidx.compose.ui.test.performClick
import org.junit.Assert.assertEquals
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import uniffi.mailcal_bindings.AllodiaAccountKind
import uniffi.mailcal_bindings.AllodiaAccountOffer

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

    private var creates = 0
    private var signIns = 0
    private var cancels = 0
    private val setUp = mutableListOf<String>()

    private fun card(
        offered: Boolean = true,
        signingIn: Boolean = false,
        signedIn: Boolean = false,
        checking: Boolean = false,
        offers: List<AllodiaAccountOffer>? = emptyList(),
        escapable: Boolean = false,
        firstRun: Boolean = true,
    ) {
        compose.setContent {
            OnboardingAllodiaCard(
                offered = offered,
                signingIn = signingIn,
                signedIn = signedIn,
                offers = offers,
                checking = checking,
                onCreate = { creates += 1 },
                onSignIn = { signIns += 1 },
                onSetUp = { setUp += it.email },
                escapable = escapable,
                onCancelSignIn = { cancels += 1 },
                firstRun = firstRun,
            )
        }
        compose.waitForIdle()
    }

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
        assertEquals(1, creates)
        assertEquals(1, signIns)
    }

    /**
     * A sign-in in flight offers neither button: a second browser hop would discard the first
     * flow's verifier.
     */
    @Test
    fun a_sign_in_in_flight_offers_nothing_to_press_twice() {
        card(signingIn = true)
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
        card(signingIn = true, escapable = true)
        compose.onNodeWithText(CANCEL).assertIsDisplayed()

        compose.onNodeWithText(CANCEL).performClick()

        assertEquals(1, cancels)
    }

    /**
     * And not before it has earned it: an ordinary hop is in front of the person within a second,
     * so a button drawn for that one is noise on every sign-in that ever works.
     */
    @Test
    fun an_ordinary_hop_draws_no_way_back_it_has_not_earned() {
        card(signingIn = true)
        compose.onNodeWithText(CANCEL).assertDoesNotExist()
    }

    /**
     * The pass that follows a sign-in is a bounded network call, not a wait on somebody in another
     * application, there is nothing there to escape from.
     */
    @Test
    fun the_pass_after_a_sign_in_offers_no_way_back() {
        card(signedIn = true, checking = true, escapable = true)
        compose.onNodeWithText(CANCEL).assertDoesNotExist()
    }

    /**
     * The whole reason to sign in here: the screen that follows is not an empty address field.
     * The offer only fills that field, detection still decides the route.
     */
    @Test
    fun an_account_from_another_device_replaces_the_card_with_a_shortcut() {
        card(signedIn = true, offers = listOf(offer("someone@gmail.com")))
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
        card(signedIn = true)
        compose.onNodeWithText(TITLE).assertDoesNotExist()
        compose.onNodeWithText(HAVE_ONE).assertDoesNotExist()
        compose.onNodeWithText(NONE_TITLE).assertIsDisplayed()
        compose.onNodeWithText(DIVIDER).assertIsDisplayed()
    }

    /** And an account that did come over is not "no mail accounts yet". */
    @Test
    fun an_offer_is_not_reported_as_an_empty_account() {
        card(signedIn = true, offers = listOf(offer("someone@gmail.com")))
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
        card(signedIn = true, offers = listOf(offer("carol@example.test")), firstRun = false)
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
        card(signedIn = true, offers = emptyList(), firstRun = false)
        compose.onNodeWithText(DIVIDER).assertDoesNotExist()
        compose.onNodeWithText(NONE_TITLE).assertDoesNotExist()
    }

    /** And the same when no pass has answered, `setContent` runs once, so this is its own case. */
    @Test
    fun a_later_add_with_no_answer_yet_is_the_direct_route_alone() {
        card(signedIn = true, offers = null, firstRun = false)
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
        card(signedIn = true, offers = null)
        compose.onNodeWithText(NONE_TITLE).assertDoesNotExist()
        compose.onNodeWithText(DIVIDER).assertIsDisplayed()
    }
}
