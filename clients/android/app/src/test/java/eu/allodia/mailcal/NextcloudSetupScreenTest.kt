// What the setup surfaces draw for a Nextcloud server (docs/nextcloud-sign-in.md rule 2): the
// found card leads with Nextcloud's own sign-in and keeps the password behind "Use a password
// instead"; the manual form asks once the typing settles and draws the offer above its fields;
// the separate-server step names it and says where it waits.
package eu.allodia.mailcal

import androidx.compose.foundation.layout.Column
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.setValue
import androidx.compose.ui.test.assertIsDisplayed
import androidx.compose.ui.test.assertIsEnabled
import androidx.compose.ui.test.junit4.v2.createComposeRule
import androidx.compose.ui.test.onAllNodesWithText
import androidx.compose.ui.test.onNodeWithTag
import androidx.compose.ui.test.onNodeWithText
import androidx.compose.ui.test.performClick
import androidx.compose.ui.test.performScrollTo
import org.junit.Assert.assertEquals
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import uniffi.mailcal_bindings.AccountCapability
import uniffi.mailcal_bindings.SetupChoice

private const val CLOUD = "https://cloud.example.org/remote.php/dav"
private const val SIGN_IN = "Sign in with Nextcloud"
private const val PASSWORD = "Password"

@RunWith(RobolectricTestRunner::class)
class NextcloudSetupScreenTest {
    @get:Rule val compose = createComposeRule()

    private val started = mutableListOf<Pair<String, List<AccountCapability>>>()
    private val asked = mutableListOf<String>()

    private fun ui(offered: Boolean = true, signingIn: Boolean = false) = NextcloudSetupUi(
        signingIn = signingIn,
        offers = { offered && it == CLOUD },
        onAsk = { asked += it },
        onSignIn = { server, uses -> started += server to uses },
    )

    private val offer = UseOffer(
        listOf(
            SetupChoice(AccountCapability.CALENDAR, on = true, serverFound = true),
            SetupChoice(AccountCapability.CONTACTS, on = true, serverFound = true),
        ),
        caldavUrl = CLOUD,
        carddavUrl = CLOUD,
    )

    private fun matching(text: String) = compose.onAllNodesWithText(text).fetchSemanticsNodes().size

    @Test
    fun theFoundCardLeadsWithTheSignInAndKeepsThePasswordBehind() {
        compose.setContent {
            DavFoundCard(
                email = "alice@cloud.example.org",
                offer = offer,
                nextcloud = ui(),
                connecting = false,
                externalError = null,
                externalFailure = null,
                onConnect = { null },
                onManual = {},
            )
        }
        compose.onNodeWithText(SIGN_IN).performScrollTo().assertIsDisplayed()
        assertEquals("no field before it is asked for", 0, matching(PASSWORD))
        compose.onNodeWithText(SIGN_IN).performScrollTo().performClick()
        assertEquals(listOf(CLOUD to listOf(AccountCapability.CALENDAR, AccountCapability.CONTACTS)), started)
        compose.onNodeWithTag("nextcloud-password-instead").performScrollTo().performClick()
        compose.onNodeWithText(PASSWORD).assertIsDisplayed()
    }

    @Test
    fun aCardForAnotherServerAsksForThePasswordAsAlways() {
        compose.setContent {
            DavFoundCard(
                email = "alice@cloud.example.org",
                offer = offer,
                connecting = false,
                externalError = null,
                externalFailure = null,
                onConnect = { null },
                onManual = {},
            )
        }
        assertEquals(0, matching(SIGN_IN))
        compose.onNodeWithText(PASSWORD).assertIsDisplayed()
    }

    @Test
    fun theManualFormAsksOnceTheTypingSettlesAndOffersAboveItsFields() {
        var answered by mutableStateOf(false)
        compose.setContent {
            Column { DavManualFields(
                login = "alice",
                onLogin = {},
                caldav = CLOUD,
                onCaldav = {},
                carddav = "",
                onCarddav = {},
                password = "",
                onPassword = {},
                caldavMissing = false,
                nextcloud = ui(offered = answered),
            ) }
        }
        compose.mainClock.advanceTimeBy(1_000)
        compose.waitForIdle()
        assertEquals(listOf(CLOUD), asked)
        assertEquals(0, matching(SIGN_IN))
        answered = true
        compose.waitForIdle()
        compose.onNodeWithText(SIGN_IN).assertIsEnabled()
        compose.onNodeWithText(SIGN_IN).performClick()
        compose.waitForIdle()
        assertEquals(listOf(CLOUD to listOf(AccountCapability.CALENDAR, AccountCapability.CONTACTS)), started)
        compose.onNodeWithText(PASSWORD).assertIsDisplayed()
    }

    @Test
    fun theSeparateStepNamesNextcloudAndSaysWhereItWaits() {
        val form = SeparateServerForm("alice@imap", listOf(AccountCapability.CALENDAR), UnreachedSet.CALENDAR, "cloud.example.org", "alice")
        var step by mutableStateOf<SeparateSignInStep?>(null)
        compose.setContent {
            SeparateServerStepScreen(
                form,
                SeparateControls.of(busy = false, offersSignIn = false, step = step, nextcloud = true),
                null,
                { _, _ -> },
                {},
                onSignIn = {},
                onShown = {},
                onLeave = {},
            )
        }
        compose.onNodeWithText(SIGN_IN).assertIsDisplayed()
        compose.onNodeWithText(
            "This server runs Nextcloud. Signing in through your browser gives this app a password of its own, which you can revoke in Nextcloud.",
        ).assertIsDisplayed()
        step = SeparateSignInStep.BROWSER
        compose.waitForIdle()
        compose.onNodeWithText("Continue in your browser. Once you have granted access, come back here.").assertIsDisplayed()
    }
}
