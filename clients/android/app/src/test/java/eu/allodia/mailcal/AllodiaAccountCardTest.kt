// Settings → Accounts, the Allodia-account card. Three states and nothing else: signed out offers
// a sign-in, a sign-in in flight offers neither button, and a signed-in account names the address
// (never only the display name, the address is what identifies the account).
//
// Worth pinning because two of the three are silent when wrong: a card that keeps its button while
// a sign-in is running invites a second browser hop that discards the first flow's verifier, and a
// card that shows only a name says nothing about WHICH account a person is signed in to.
package eu.allodia.mailcal

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
import uniffi.mailcal_bindings.AllodiaAccount

private const val SIGN_IN = "Sign in"
private const val SIGN_OUT = "Sign out"
private const val CREATE = "Create an account"
private const val MANAGE = "Manage account"
private const val DELETE = "Delete account"

@RunWith(RobolectricTestRunner::class)
class AllodiaAccountCardTest {
    @get:Rule val compose = createComposeRule()

    private var signIns = 0
    private var creates = 0
    private var manages = 0
    private var signOuts = 0

    private fun card(state: AllodiaSettings) {
        compose.setContent {
            AllodiaAccountCard(
                state,
                onSignIn = { signIns += 1 },
                onCreate = { creates += 1 },
                onManage = { manages += 1 },
                onSignOut = { signOuts += 1 },
            )
        }
        compose.waitForIdle()
    }

    /**
     * Creating is its own control, not a link inside the sign-in page.
     *
     * Someone with no account who presses "Sign in" lands on a form asking for a password they
     * have never set, which reads as the app being broken rather than as the wrong button.
     */
    @Test
    fun nobody_signed_in_can_also_create_an_account() {
        card(AllodiaSettings(available = true))

        compose.onNodeWithText(CREATE).assertIsDisplayed()
        compose.onNodeWithText(CREATE).performClick()

        assertEquals(1, creates)
        assertEquals("creating is not signing in", 0, signIns)
    }

    /**
     * Deletion has to be findable from inside the app, and "Manage account" is not the phrase
     * anyone reaches for when they want out. Both open the same page.
     */
    @Test
    fun a_signed_in_account_can_be_managed_and_deleted_from_here() {
        card(
            AllodiaSettings(
                available = true,
                account = AllodiaAccount("someone@example.com", null),
            ),
        )

        // No performScrollTo: this test hosts the card alone, with no scrolling parent.
        compose.onNodeWithText(MANAGE).performClick()
        compose.onNodeWithText(DELETE).performClick()

        assertEquals("both routes reach the account page", 2, manages)
        assertEquals(0, signOuts)
    }

    @Test
    fun nobody_signed_in_offers_a_sign_in() {
        card(AllodiaSettings(available = true))

        compose.onNodeWithText(SIGN_IN).assertIsDisplayed()
        compose.onNodeWithText(SIGN_IN).performClick()

        assertEquals(1, signIns)
    }

    @Test
    fun a_sign_in_in_flight_offers_no_button_to_press_again() {
        card(AllodiaSettings(available = true, signingIn = true))

        compose.onNodeWithText("Signing in…").assertIsDisplayed()
        compose.onAllNodesWithText(SIGN_IN).assertCountEquals(0)
    }

    @Test
    fun a_signed_in_account_names_the_address_and_offers_a_way_out() {
        card(
            AllodiaSettings(
                available = true,
                account = AllodiaAccount(email = "alice@allodia.eu", name = "Alice Ackermann"),
            ),
        )

        // Both: the name is what a person recognises, the address is what identifies the account.
        compose.onNodeWithText("Alice Ackermann").assertIsDisplayed()
        compose.onNodeWithText("Signed in as alice@allodia.eu").assertIsDisplayed()
        compose.onNodeWithText(SIGN_OUT).performClick()

        assertEquals(1, signOuts)
    }

    // A service that holds no display name must still produce a complete card rather than a blank
    // line above the address.
    @Test
    fun an_account_without_a_name_still_names_its_address() {
        card(
            AllodiaSettings(
                available = true,
                account = AllodiaAccount(email = "alice@allodia.eu", name = null),
            ),
        )

        compose.onNodeWithText("Signed in as alice@allodia.eu").assertIsDisplayed()
    }

    @Test
    fun a_failure_says_what_the_service_said() {
        card(AllodiaSettings(available = true, failure = "invalid_grant"))

        compose.onNodeWithText("Signing in didn’t work: invalid_grant").assertIsDisplayed()
    }
}
