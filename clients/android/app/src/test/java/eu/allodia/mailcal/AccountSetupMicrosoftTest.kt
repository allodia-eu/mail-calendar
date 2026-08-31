// Regression: the email-first flow's Microsoft found-card must not be a silent dead-end. A
// declined/failed sign-in (e.g. Microsoft returning access_denied) comes back as an external
// error, and the card has to show it so the user can retry or fall back to manual setup, the
// bug where the card rendered but did nothing after a failed sign-in.
package eu.allodia.mailcal

import androidx.compose.ui.test.assertIsDisplayed
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
class AccountSetupMicrosoftTest {
    @get:Rule val compose = createComposeRule()

    private fun flow(externalError: String?, onSignIn: (String?) -> Unit = {}) {
        compose.setContent {
            AccountSetupFlow(
                externalError = externalError,
                onCancel = null,
                signingIn = false,
                connecting = false,
                // A Microsoft-hosted domain routes straight to the Microsoft found-card.
                detect = { SetupRecommendation.Microsoft(it) },
                onSignInMicrosoft = onSignIn,
                onConnect = { null },
                onConnectJmap = { null },
            )
        }
        compose.onNodeWithText("Email").performTextInput("someone@outlook.com")
        compose.onNodeWithText("Continue").performClick()
        compose.waitForIdle()
    }

    @Test
    fun microsoftCardSurfacesASignInError() {
        flow(externalError = "Microsoft sign-in failed: access_denied")
        // The card is shown with the sign-in button AND the error, not a silent dead-end.
        compose.onNodeWithText("Sign in with Microsoft").assertIsDisplayed()
        compose.onNodeWithText("access_denied", substring = true).assertIsDisplayed()
    }

    @Test
    fun microsoftSignInPassesTheDetectedEmailAsTheHint() {
        var hint: String? = "unset"
        flow(externalError = null, onSignIn = { hint = it })
        compose.onNodeWithText("Sign in with Microsoft").performClick()
        // The detected address is threaded through as the login hint, so Microsoft targets it.
        assert(hint == "someone@outlook.com") { "expected the detected email as the hint, got $hint" }
    }
}
