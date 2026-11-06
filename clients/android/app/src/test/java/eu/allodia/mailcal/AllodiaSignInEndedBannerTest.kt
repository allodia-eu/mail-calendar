// The banner a refused Allodia sign-in puts beside the mail accounts' expired ones. It names the
// address, offers the one remedy and nothing else, and is gone the moment the core stops naming a
// refusal. A sign-in that does not complete is a toast, as the mail banner's is, never a line here.
package eu.allodia.mailcal

import android.content.Context
import androidx.compose.ui.test.assertIsDisplayed
import androidx.compose.ui.test.junit4.v2.createComposeRule
import androidx.compose.ui.test.onNodeWithText
import androidx.compose.ui.test.performClick
import androidx.test.core.app.ApplicationProvider
import org.junit.Assert.assertEquals
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner

private const val PROMPT =
    "The sign-in to your Allodia account alice@allodia.eu has ended. " +
        "Sign in again to keep your accounts in step."

@RunWith(RobolectricTestRunner::class)
class AllodiaSignInEndedBannerTest {
    @get:Rule val compose = createComposeRule()

    private var signIns = 0

    private fun banner(email: String?) {
        val ctx = ApplicationProvider.getApplicationContext<Context>()
        compose.setContent {
            AllodiaSignInEndedBanner(email, onSignIn = { signIns += 1 }, ctx = ctx)
        }
        compose.waitForIdle()
    }

    @Test
    fun a_refused_sign_in_names_the_address_and_signs_in_again() {
        banner("alice@allodia.eu")

        compose.onNodeWithText(PROMPT).assertIsDisplayed()
        compose.onNodeWithText("Sign in again").performClick()

        assertEquals(1, signIns)
    }

    @Test
    fun nothing_is_drawn_once_the_core_names_no_refusal() {
        banner(null)

        compose.onNodeWithText(PROMPT, substring = true).assertDoesNotExist()
        compose.onNodeWithText("Sign in again").assertDoesNotExist()
    }
}
