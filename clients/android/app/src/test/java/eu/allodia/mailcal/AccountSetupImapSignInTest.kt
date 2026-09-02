// What a mail account's setup surface asks for, once its server has answered.
//
// Three answers, and the middle one is why they are three: a provider whose sign-in exists but
// admits only applications it registered in advance is not the same as one that offers none, and
// showing one bare password form for both leaves somebody wondering why the button their
// colleague has is missing (docs/mail-oauth.md rule 2).
//
// The other rule these cover is negative and easy to regress: the password field is never
// unreachable where a password works, and never *offered* where it does not.
package eu.allodia.mailcal

import androidx.compose.ui.test.assertIsDisplayed
import androidx.compose.ui.test.junit4.createComposeRule
import androidx.compose.ui.test.onAllNodesWithText
import androidx.compose.ui.test.onNodeWithText
import androidx.compose.ui.test.performClick
import androidx.compose.ui.test.performScrollTo
import androidx.compose.ui.test.performTextInput
import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import uniffi.mailcal_bindings.ImapAuthOffer
import uniffi.mailcal_bindings.ImapLoginRequest

private const val FIELD_EMAIL = "Email"
private const val FIELD_MAIL_SERVER = "Mail server"
private const val FIELD_PASSWORD = "Password"
private const val SIGN_IN = "Sign in with your provider"
private const val REGISTRATION_NEEDED =
    "This provider only allows apps it has registered in advance. " +
        "Ask us to add support for it, and use your password in the meantime."

@RunWith(RobolectricTestRunner::class)
class AccountSetupImapSignInTest {
    @get:Rule val compose = createComposeRule()

    private val signInStarts = mutableListOf<ImapLoginRequest>()
    private val asked = mutableListOf<ImapLoginRequest>()

    /** Renders the manual password tab with a pre-flight that answers [offer]. */
    private fun manualTab(offer: ImapAuthOffer) {
        compose.setContent {
            AccountSetupScreen(
                onConnect = { null },
                onConnectJmap = { null },
                onCheckImapAuth = { request -> asked += request; offer },
                onSignInImap = { request -> signInStarts += request },
            )
        }
        compose.waitForIdle()
    }

    /** Fills in enough for the pre-flight to be worth running, then waits past the debounce. */
    private fun typeAccount() {
        compose.onNodeWithText(FIELD_MAIL_SERVER).performTextInput("imap.example.com")
        compose.onNodeWithText(FIELD_EMAIL).performTextInput("alice@example.com")
        compose.mainClock.advanceTimeBy(JMAP_SIGNIN_PROBE_DEBOUNCE_MS + 100)
        compose.waitForIdle()
    }

    /** How many nodes carry [text]; zero is the assertion these tests mostly make. */
    private fun matching(text: String): Int =
        compose.onAllNodesWithText(text).fetchSemanticsNodes().size

    @Test
    fun aServerThatTakesOnlyAPasswordIsNeverOfferedASignIn() {
        // The common case, and the one where a button would dead-end at the provider.
        manualTab(ImapAuthOffer.Password)
        typeAccount()
        compose.onNodeWithText(FIELD_PASSWORD).assertIsDisplayed()
        assertEquals(0, matching(SIGN_IN))
    }

    @Test
    fun aServerThatOffersSignInShowsTheButtonAndKeepsThePasswordField() {
        // Sign-in leads because that is what the server said it prefers, and the password route
        // stays: this pane is where somebody lands when nothing was detected, so a server that
        // later declines the sign-in must still be connectable from here.
        manualTab(
            ImapAuthOffer.SignIn(
                issuer = "https://login.example.com",
                providerLabel = null,
                passwordAlsoWorks = true,
            )
        )
        typeAccount()
        compose.onNodeWithText(SIGN_IN).assertIsDisplayed()
        // Scrolled to rather than merely asserted: the explanation and the button push it down a
        // scrolling form, and "off the bottom" is not the same as "gone".
        compose.onNodeWithText(FIELD_PASSWORD).performScrollTo().assertIsDisplayed()
    }

    @Test
    fun aClosedSignInIsExplainedRatherThanLeftToBeGuessedAt() {
        // No button, because there is no sign-in we can start, and a line saying why. Without it
        // this is indistinguishable from a provider that simply has no OAuth.
        manualTab(ImapAuthOffer.RegistrationNeeded(passwordAlsoWorks = true))
        typeAccount()
        compose.onNodeWithText(REGISTRATION_NEEDED).assertIsDisplayed()
        assertEquals(0, matching(SIGN_IN))
        compose.onNodeWithText(FIELD_PASSWORD).performScrollTo().assertIsDisplayed()
    }

    @Test
    fun theSignInIsStartedForTheAccountThatWasAskedAbout() {
        // A sign-in that registered against a different server from the one the pre-flight probed
        // would offer a button that fails at the provider.
        manualTab(
            ImapAuthOffer.SignIn(
                issuer = "https://login.example.com",
                providerLabel = null,
                passwordAlsoWorks = true,
            )
        )
        typeAccount()
        compose.onNodeWithText(SIGN_IN).performClick()
        compose.waitForIdle()

        assertEquals(1, signInStarts.size)
        assertEquals("alice@example.com", signInStarts[0].email)
        assertEquals("imap.example.com", signInStarts[0].imapHost)
        assertEquals(asked.last().imapHost, signInStarts[0].imapHost)
    }

    @Test
    fun aHalfTypedAccountIsNotWorthADialAtTheProvider() {
        // The pre-flight opens a TLS connection to the mail server. Firing one per keystroke at
        // whatever host the field momentarily spells is what the debounce and this guard exist
        // for; an address with no server answers nothing useful.
        manualTab(ImapAuthOffer.Password)
        compose.onNodeWithText(FIELD_EMAIL).performTextInput("alice@example.com")
        compose.mainClock.advanceTimeBy(JMAP_SIGNIN_PROBE_DEBOUNCE_MS + 100)
        compose.waitForIdle()
        assertTrue("no server typed yet", asked.isEmpty())
    }
}
