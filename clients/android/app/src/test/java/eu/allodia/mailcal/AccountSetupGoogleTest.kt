// The Google Early Access gate is a security contract: Google hard-blocks anyone not on the app's
// OAuth test-user allow-list, so the "Sign in with Google" button must stay disabled until the
// user confirms (checkbox) they've signed up, otherwise we'd open a browser flow that just dead-
// ends on Google's block screen. Mirrors AccountSetupMicrosoftTest's shape.
package eu.allodia.mailcal

import androidx.compose.ui.test.assertIsDisplayed
import androidx.compose.ui.test.assertIsEnabled
import androidx.compose.ui.test.assertIsNotEnabled
import androidx.compose.ui.test.isToggleable
import androidx.compose.ui.test.junit4.v2.createComposeRule
import androidx.compose.ui.test.onNodeWithText
import androidx.compose.ui.test.performClick
import androidx.compose.ui.test.performTextInput
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import uniffi.mailcal_bindings.SetupRecommendation

@RunWith(RobolectricTestRunner::class)
class AccountSetupGoogleTest {
    @get:Rule val compose = createComposeRule()

    // Drive the email-first flow to the Google found-card (a Google-hosted domain routes straight
    // there), leaving the Early Access checkbox unchecked.
    private fun flow(onSignIn: (String?) -> Unit = {}) {
        compose.setContent {
            AccountSetupFlow(
                externalError = null,
                onCancel = null,
                signingIn = false,
                signingInGoogle = false,
                connecting = false,
                detect = { SetupRecommendation.Google(it) },
                onSignInMicrosoft = {},
                onSignInGoogle = onSignIn,
                onConnect = { null },
                onConnectJmap = { null },
            )
        }
        compose.onNodeWithText("Email").performTextInput("someone@gmail.com")
        compose.onNodeWithText("Continue").performClick()
        compose.waitForIdle()
    }

    @Test
    fun googleSignInIsGatedOnTheEarlyAccessConfirmation() {
        flow()
        // The button is shown but disabled before the Early Access checkbox is ticked.
        compose.onNodeWithText("Sign in with Google").assertIsDisplayed()
        compose.onNodeWithText("Sign in with Google").assertIsNotEnabled()
        // The only toggleable on the card is the mandatory Early Access checkbox.
        compose.onNode(isToggleable()).performClick()
        compose.waitForIdle()
        // Confirmed, the button is now enabled.
        compose.onNodeWithText("Sign in with Google").assertIsEnabled()
    }

    @Test
    fun googleSignInPassesTheDetectedEmailAsTheHint() {
        var hint: String? = "unset"
        flow(onSignIn = { hint = it })
        // Confirm Early Access, then sign in.
        compose.onNode(isToggleable()).performClick()
        compose.onNodeWithText("Sign in with Google").performClick()
        // The detected address is threaded through as the login hint, so Google targets it.
        assert(hint == "someone@gmail.com") { "expected the detected email as the hint, got $hint" }
    }
}
