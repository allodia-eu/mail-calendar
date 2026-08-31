// The JMAP "sign in with your provider" offer. Two rules, both negative and both easy to regress.
//
// The button is shown ONLY after the core confirms this specific server advertises discoverable
// OAuth, a server that doesn't is the common case, not an error, and a button that dead-ends
// there is worse than no button at all.
//
// And the manual secret is never *unreachable*. On the detected card it is hidden while sign-in
// is on offer (showing both asks the user to choose between two things that do the same job), but
// it reappears the instant sign-in fails, and "Set up manually" reaches it at any time. On the
// manual tab, which the user chose deliberately, it is always present.
package eu.allodia.mailcal

import androidx.compose.ui.test.assertIsDisplayed
import androidx.compose.ui.test.junit4.createComposeRule
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
import uniffi.mailcal_bindings.SetupRecommendation

private const val TAB_JMAP = "JMAP"
private const val FIELD_EMAIL = "Email"
private const val FIELD_SECRET = "Password or API token"
private const val SIGN_IN = "Sign in with your provider"

@RunWith(RobolectricTestRunner::class)
class AccountSetupJmapSignInTest {
    @get:Rule val compose = createComposeRule()

    private val signInStarts = mutableListOf<Pair<String, String>>()
    private val probed = mutableListOf<Pair<String, String>>()

    /** Renders the JMAP tab with an availability probe that answers [available]. */
    private fun jmapTab(available: Boolean) {
        compose.setContent {
            AccountSetupScreen(
                onConnect = { null },
                onConnectJmap = { null },
                onCheckJmapSignIn = { email, server -> probed += email to server; available },
                onSignInJmap = { email, server -> signInStarts += email to server },
            )
        }
        compose.onNodeWithText(TAB_JMAP).performClick()
        compose.waitForIdle()
    }

    private fun typeEmail(address: String) {
        compose.onNodeWithText(FIELD_EMAIL).performTextInput(address)
        // The probe is debounced, so advance past it rather than racing it.
        compose.mainClock.advanceTimeBy(JMAP_SIGNIN_PROBE_DEBOUNCE_MS + 100)
        compose.waitForIdle()
    }

    @Test
    fun no_sign_in_button_before_the_server_is_known() {
        // Nothing has been typed, so nothing has been probed: the button must not be there.
        jmapTab(available = true)

        compose.onNodeWithText(SIGN_IN).assertDoesNotExist()
        assertTrue("nothing to probe yet", probed.isEmpty())
    }

    @Test
    fun a_server_without_oauth_never_offers_the_button() {
        jmapTab(available = false)
        typeEmail("alice@self-hosted.example")

        // The probe ran and said no, so no button, and the manual field is still there. This is
        // the majority case for self-hosted JMAP servers.
        assertEquals(listOf("alice@self-hosted.example" to ""), probed)
        compose.onNodeWithText(SIGN_IN).assertDoesNotExist()
        compose.onNodeWithText(FIELD_SECRET).assertIsDisplayed()
    }

    @Test
    fun a_server_with_oauth_offers_the_button_alongside_the_secret_field() {
        // The MANUAL tab keeps both: the user reached it by explicitly choosing to enter settings
        // by hand, so the secret field is the whole reason they are here. (The *detected* card
        // hides it until sign-in fails, see AccountSetupDetectTest.)
        jmapTab(available = true)
        typeEmail("alice@fastmail.example")

        compose.onNodeWithText(SIGN_IN).performScrollTo().assertIsDisplayed()
        compose.onNodeWithText(FIELD_SECRET).assertIsDisplayed()
    }

    @Test
    fun tapping_sign_in_passes_the_typed_email_and_server() {
        jmapTab(available = true)
        typeEmail("alice@fastmail.example")
        compose.onNodeWithText(SIGN_IN).performScrollTo().performClick()
        compose.waitForIdle()

        assertEquals(listOf("alice@fastmail.example" to ""), signInStarts)
    }

    @Test
    fun the_offer_is_withdrawn_when_the_address_changes_to_another_server() {
        // A stale "offered" surviving an edit would show a sign-in button for a server that was
        // never probed, and start a flow against the wrong host.
        var available = true
        compose.setContent {
            AccountSetupScreen(
                onConnect = { null },
                onConnectJmap = { null },
                onCheckJmapSignIn = { _, _ -> available },
                onSignInJmap = { _, _ -> },
            )
        }
        compose.onNodeWithText(TAB_JMAP).performClick()
        compose.waitForIdle()
        typeEmail("alice@fastmail.example")
        compose.onNodeWithText(SIGN_IN).performScrollTo().assertIsDisplayed()

        available = false
        compose.onNodeWithText(FIELD_EMAIL).performTextInput("x")
        compose.mainClock.advanceTimeBy(JMAP_SIGNIN_PROBE_DEBOUNCE_MS + 100)
        compose.waitForIdle()

        compose.onNodeWithText(SIGN_IN).assertDoesNotExist()
    }

    @Test
    fun a_form_with_no_probe_wired_shows_only_the_manual_path() {
        // The default (no `onCheckJmapSignIn`) must never render the button, that is what keeps
        // every other caller, preview and test on the plain form.
        compose.setContent {
            AccountSetupScreen(onConnect = { null }, onConnectJmap = { null })
        }
        compose.onNodeWithText(TAB_JMAP).performClick()
        compose.waitForIdle()
        compose.onNodeWithText(FIELD_EMAIL).performTextInput("alice@fastmail.example")
        compose.mainClock.advanceTimeBy(JMAP_SIGNIN_PROBE_DEBOUNCE_MS + 100)
        compose.waitForIdle()

        compose.onNodeWithText(SIGN_IN).assertDoesNotExist()
        compose.onNodeWithText(FIELD_SECRET).assertIsDisplayed()
    }
}

// The detected-JMAP card, driven through the real detection flow with a scripted result. This is
// where the "one obvious action" rule lives: while sign-in is on offer the secret field is out of
// the way, and the moment sign-in fails it must come back, otherwise a user on a server whose
// OAuth we could not complete is left with a screen that can do nothing at all.
@RunWith(RobolectricTestRunner::class)
class DetectedJmapSignInTest {
    @get:Rule val compose = createComposeRule()

    private val jmapRecommendation = SetupRecommendation.Jmap(
        email = "alice@fastmail.example",
        serverUrl = "https://api.fastmail.example",
        isTrusted = true,
        source = "https://api.fastmail.example/.well-known/jmap",
    )

    /** Runs detection to the JMAP card, with sign-in [available] and an optional prior failure. */
    private fun detectedCard(available: Boolean, externalError: String? = null) {
        compose.setContent {
            AccountSetupFlow(
                externalError = externalError,
                onCancel = null,
                signingIn = false,
                connecting = false,
                detect = { jmapRecommendation },
                onSignInMicrosoft = {},
                onConnect = { null },
                onConnectJmap = { null },
                onCheckJmapSignIn = { _, _ -> available },
                onSignInJmap = { _, _ -> },
            )
        }
        compose.onNodeWithText(FIELD_EMAIL).performTextInput("alice@fastmail.example")
        compose.waitForIdle()
        compose.onNodeWithText("Continue").performClick()
        compose.waitForIdle()
    }

    @Test
    fun sign_in_replaces_the_secret_field_rather_than_sitting_above_it() {
        detectedCard(available = true)

        compose.onNodeWithText(SIGN_IN).assertIsDisplayed()
        // The point of the change: no second way to do the same thing competing for attention.
        compose.onNodeWithText(FIELD_SECRET).assertDoesNotExist()
        // …but the escape hatch to the manual form is still one tap away.
        compose.onNodeWithText("Set up manually").assertIsDisplayed()
    }

    @Test
    fun a_failed_sign_in_brings_the_secret_field_back() {
        // Without this the user is stranded: a sign-in that cannot complete, and no other way to
        // connect on the screen they are looking at.
        detectedCard(available = true, externalError = "Signing in didn't work.")

        compose.onNodeWithText(FIELD_SECRET).performScrollTo().assertIsDisplayed()
        compose.onNodeWithText(SIGN_IN).assertIsDisplayed()
    }

    @Test
    fun a_server_without_sign_in_shows_the_secret_field_as_before() {
        detectedCard(available = false)

        compose.onNodeWithText(FIELD_SECRET).assertIsDisplayed()
        compose.onNodeWithText(SIGN_IN).assertDoesNotExist()
    }
}
