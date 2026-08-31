// The expired-sign-in banner's one decision: is there a sign-in this app can re-run, or must the
// user go to Settings? It reads the core's `account_provider`, and JMAP is the family where the
// answer is not decided by the protocol, an account connected by SIGNING IN can be re-authorized
// in place, while one holding a pasted password/API token has no browser flow at all.
//
// Worth pinning because getting it wrong is silent in both directions: a missing button leaves a
// user with no remedy but removing and re-adding the account (which is the bug this closes), and a
// button offered to a secret account dead-ends in an error it can never recover from.
package eu.allodia.mailcal

import androidx.compose.ui.test.assertIsDisplayed
import androidx.compose.ui.test.junit4.createComposeRule
import androidx.compose.ui.test.onNodeWithText
import androidx.compose.ui.test.performClick
import androidx.test.core.app.ApplicationProvider
import org.junit.Assert.assertEquals
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import uniffi.mailcal_bindings.AccountProvider

private const val SIGN_IN_AGAIN = "Sign in again"

@RunWith(RobolectricTestRunner::class)
class SignInExpiredBannerTest {
    @get:Rule val compose = createComposeRule()

    private val signedIn = mutableListOf<ExpiredSignIn>()

    private fun banner(provider: AccountProvider?) {
        val ctx = ApplicationProvider.getApplicationContext<android.content.Context>()
        val account = ExpiredSignIn(
            id = "alice@jmap:api.example.com",
            email = "alice@example.com",
            provider = provider,
        )
        compose.setContent {
            SignInExpiredBanner(listOf(account), onSignIn = { signedIn += it }, ctx = ctx)
        }
        compose.waitForIdle()
    }

    @Test
    fun a_jmap_account_that_signed_in_is_offered_its_sign_in_again() {
        banner(AccountProvider.JMAP_OAUTH)

        compose.onNodeWithText(SIGN_IN_AGAIN).assertIsDisplayed()
        compose.onNodeWithText(SIGN_IN_AGAIN).performClick()

        // The id, not the address: the core re-authorises THIS account's persisted grant.
        assertEquals(listOf("alice@jmap:api.example.com"), signedIn.map { it.id })
    }

    @Test
    fun a_jmap_account_holding_a_pasted_secret_is_pointed_at_settings() {
        banner(AccountProvider.JMAP)

        // No browser flow exists for a stored password/API token, so no button is offered, and
        // the wording has to change with it, or it would name an action that isn't there.
        compose.onNodeWithText(SIGN_IN_AGAIN).assertDoesNotExist()
        compose.onNodeWithText("Settings", substring = true).assertIsDisplayed()
    }

    @Test
    fun an_account_of_an_unknown_family_is_pointed_at_settings() {
        banner(null)

        compose.onNodeWithText(SIGN_IN_AGAIN).assertDoesNotExist()
    }
}
