// The manual form's Nextcloud pane (docs/nextcloud-sign-in.md rule 2): the server's address alone,
// a sign-in held until it holds something, what the wait says while the core asks and once the
// browser holds the sign-in, and the offer to sign in anyway, only while the field holds the
// address the core could not place. Also what the pane says of each answer that started nothing.
package eu.allodia.mailcal

import android.content.Context
import androidx.compose.foundation.layout.Column
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.setValue
import androidx.compose.ui.test.assertIsDisplayed
import androidx.compose.ui.test.assertIsEnabled
import androidx.compose.ui.test.assertIsNotEnabled
import androidx.compose.ui.test.junit4.v2.createComposeRule
import androidx.compose.ui.test.onAllNodesWithTag
import androidx.compose.ui.test.onAllNodesWithText
import androidx.compose.ui.test.onNodeWithTag
import androidx.compose.ui.test.onNodeWithText
import androidx.compose.ui.test.performClick
import androidx.test.core.app.ApplicationProvider
import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import uniffi.mailcal_bindings.NextcloudAddressStart

private const val SIGN_IN = "Sign in with Nextcloud"
private const val ADDRESS = "cloud.example.org"
private const val CHECKING = "Checking whether this is a Nextcloud server…"
private const val WAITING = "Continue in your browser. Once you have granted access, come back here."

@RunWith(RobolectricTestRunner::class)
class NextcloudManualPaneTest {
    @get:Rule val compose = createComposeRule()

    private val pressed = mutableListOf<Pair<String, Boolean>>()

    private fun matching(text: String) = compose.onAllNodesWithText(text).fetchSemanticsNodes().size

    private fun pane(address: () -> String, ui: () -> NextcloudSetupUi) {
        compose.setContent {
            Column { NextcloudManualFields(address(), {}, ui(), connecting = false) }
        }
    }

    @Test
    fun thePaneAsksOnlyForTheAddressAndSignsInAtIt() {
        var address by mutableStateOf("")
        pane({ address }) { NextcloudSetupUi(onSignInAt = { at, anyway -> pressed += at to anyway }) }
        compose.onNodeWithText("Server address, such as cloud.example.com").assertIsDisplayed()
        assertEquals("no password on this pane", 0, matching("Password"))
        compose.onNodeWithText(SIGN_IN).assertIsNotEnabled()
        address = ADDRESS
        compose.waitForIdle()
        compose.onNodeWithText(SIGN_IN).assertIsEnabled().performClick()
        assertEquals(listOf(ADDRESS to false), pressed)
    }

    @Test
    fun theWaitSaysTheServerIsAskedUntilTheBrowserHoldsTheSignIn() {
        var checking by mutableStateOf(true)
        pane({ ADDRESS }) { NextcloudSetupUi(signingIn = true, checking = checking) }
        compose.onNodeWithText(CHECKING).assertIsDisplayed()
        checking = false
        compose.waitForIdle()
        compose.onNodeWithText(WAITING).assertIsDisplayed()
        assertEquals(0, matching(CHECKING))
    }

    @Test
    fun anUnsureAnswerOffersTheSignInAnywayWhileTheFieldHoldsThatAddress() {
        var address by mutableStateOf(ADDRESS)
        pane({ address }) {
            NextcloudSetupUi(
                unsureFor = { if (it == ADDRESS) "cloud.example.org" else null },
                onSignInAt = { at, anyway -> pressed += at to anyway },
            )
        }
        compose.onNodeWithText(
            "Could not confirm that cloud.example.org runs Nextcloud. You can still try to sign in: if it does, your browser opens its sign-in page.",
        ).assertIsDisplayed()
        compose.onNodeWithTag("nextcloud-try-anyway").performClick()
        assertEquals(listOf(ADDRESS to true), pressed)
        address = "other.example.org"
        compose.waitForIdle()
        assertEquals(0, compose.onAllNodesWithTag("nextcloud-try-anyway").fetchSemanticsNodes().size)
    }

    @Test
    fun eachAnswerThatStartedNothingIsSaidAndUnsureIsLeftToThePane() {
        val ctx = ApplicationProvider.getApplicationContext<Context>()
        assertEquals(
            "No Nextcloud server answered at cloud.example.org. Check the address and try again.",
            nextcloudRefusalSaid(NextcloudAddressStart.NotNextcloud("cloud.example.org"), ctx),
        )
        assertEquals(
            "The Nextcloud server at cloud.example.org is in maintenance. Try again later.",
            nextcloudRefusalSaid(NextcloudAddressStart.Maintenance("cloud.example.org"), ctx),
        )
        assertEquals(
            "Enter the address of your Nextcloud server, such as cloud.example.com.",
            nextcloudRefusalSaid(NextcloudAddressStart.NotAnAddress, ctx),
        )
        assertNull(nextcloudRefusalSaid(NextcloudAddressStart.Unsure("cloud.example.org"), ctx))
    }
}
