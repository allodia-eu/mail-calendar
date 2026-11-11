// Settings → Allodia account, drawn from the core's card. Four states and nothing else: signed out
// offers a sign-in and a new account, a sign-in out offers Cancel and neither button, a signed-in
// account names the address (never only the display name, the address is what identifies the
// account), and a sign-in the service ended says so in place of the signed-in state.
//
// Worth pinning because two are silent when wrong: a card that keeps its button while a sign-in is
// out invites a second browser hop, and a card that shows only a name says nothing about WHICH
// account a person is signed in to.
package eu.allodia.mailcal

import android.content.Context
import androidx.compose.ui.test.assertCountEquals
import androidx.compose.ui.test.assertIsDisplayed
import androidx.compose.ui.test.assertIsNotEnabled
import androidx.compose.ui.test.junit4.v2.createComposeRule
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
import uniffi.mailcal_bindings.AccountChangeRefusal
import uniffi.mailcal_bindings.AllodiaAccount
import uniffi.mailcal_bindings.AllodiaCard
import uniffi.mailcal_bindings.AllodiaCardState
import uniffi.mailcal_bindings.AllodiaFailure
import uniffi.mailcal_bindings.AllodiaIntent
import uniffi.mailcal_bindings.AllodiaOnboarding
import uniffi.mailcal_bindings.BrowserSignIn
import uniffi.mailcal_bindings.BrowserSignInStage
import uniffi.mailcal_bindings.ControlState

private const val SIGN_IN = "Sign in"
private const val SIGN_OUT = "Sign out"
private const val CREATE = "Create an account"
private const val MANAGE = "Manage account"
private const val DELETE = "Delete account"
private const val CANCEL = "Cancel"

private val ALICE = AllodiaAccount(id = null, email = "alice@allodia.eu", name = "Alice Ackermann")

/** The core's card with a sign-in out, at [attempt]. */
internal fun allodiaSigningIn(attempt: ULong = 3uL, escape: Boolean = false): AllodiaCard {
    val held = ControlState(enabled = false, running = false)
    return ALLODIA_SIGNED_OUT.copy(
        browser = BrowserSignIn(
            attempt,
            "allodia-account",
            BrowserSignInStage.InBrowser("https://account.example.org/authorize", AllodiaOAuthConfig.REDIRECT_URI),
            ControlState(enabled = true, running = false),
        ),
        signIn = held,
        create = held,
        signOut = held,
        onboarding = AllodiaOnboarding.SigningIn(escape),
    )
}

@RunWith(RobolectricTestRunner::class)
class AllodiaAccountCardTest {
    @get:Rule val compose = createComposeRule()

    private val sent = mutableListOf<AllodiaIntent>()
    private var manages = 0
    private val ctx: Context get() = ApplicationProvider.getApplicationContext()

    private fun card(card: AllodiaCard) {
        compose.setContent {
            AllodiaAccountCard(
                AllodiaSettings(available = true, card = card),
                onChange = { sent += it },
                onManage = { manages += 1 },
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
    fun nobody_signed_in_offers_signing_in_and_creating_an_account() {
        card(ALLODIA_SIGNED_OUT)

        compose.onNodeWithText(SIGN_IN).performClick()
        compose.onNodeWithText(CREATE).performClick()

        assertEquals(listOf(allodiaSignIn(), allodiaSignIn(create = true)), sent)
    }

    @Test
    fun a_sign_in_out_offers_its_cancel_and_no_button_to_press_again() {
        card(allodiaSigningIn(attempt = 7uL))

        compose.onNodeWithText("Signing in…").assertIsDisplayed()
        compose.onAllNodesWithText(SIGN_IN).assertCountEquals(0)
        compose.onAllNodesWithText(CREATE).assertCountEquals(0)
        compose.onNodeWithText(CANCEL).performClick()

        assertEquals(listOf<AllodiaIntent>(AllodiaIntent.CancelSignIn(7uL)), sent)
    }

    /**
     * Deletion has to be findable from inside the app, and "Manage account" is not the phrase
     * anyone reaches for when they want out. Both open the same page.
     */
    @Test
    fun a_signed_in_account_names_the_address_and_offers_managing_deleting_and_leaving() {
        card(ALLODIA_SIGNED_OUT.copy(state = AllodiaCardState.SignedIn(ALICE)))

        // Both: the name is what a person recognises, the address is what identifies the account.
        compose.onNodeWithText("Alice Ackermann").assertIsDisplayed()
        compose.onNodeWithText("Signed in as alice@allodia.eu").assertIsDisplayed()
        compose.onNodeWithText(MANAGE).performClick()
        compose.onNodeWithText(DELETE).performClick()
        compose.onNodeWithText(SIGN_OUT).performClick()

        assertEquals("both routes reach the account page", 2, manages)
        assertEquals(listOf<AllodiaIntent>(AllodiaIntent.SignOut), sent)
    }

    // A service that holds no display name must still produce a complete card rather than a blank
    // line above the address.
    @Test
    fun an_account_without_a_name_still_names_its_address() {
        card(ALLODIA_SIGNED_OUT.copy(state = AllodiaCardState.SignedIn(ALICE.copy(name = null))))

        compose.onNodeWithText("Signed in as alice@allodia.eu").assertIsDisplayed()
    }

    @Test
    fun a_sign_out_running_holds_its_button_with_a_spinner() {
        card(
            ALLODIA_SIGNED_OUT.copy(
                state = AllodiaCardState.SignedIn(ALICE),
                signOut = ControlState(enabled = false, running = true),
            ),
        )

        compose.onNodeWithTag("allodia-sign-out").assertIsNotEnabled()
        compose.onNodeWithTag("allodia-sign-out-running", useUnmergedTree = true).assertIsDisplayed()
    }

    /**
     * A grant the service refused is not a signed-in account. "Signed in as" with Manage account
     * over it tells somebody everything is fine while nothing the account holds can be reached.
     */
    @Test
    fun a_sign_in_the_service_ended_says_so_and_offers_to_sign_in_again() {
        card(ALLODIA_SIGNED_OUT.copy(state = AllodiaCardState.Ended(ALICE)))

        compose.onNodeWithText("Your sign-in as alice@allodia.eu has ended").assertIsDisplayed()
        compose.onAllNodesWithText("Signed in as alice@allodia.eu").assertCountEquals(0)
        compose.onAllNodesWithText(MANAGE).assertCountEquals(0)
        compose.onAllNodesWithText(DELETE).assertCountEquals(0)
        compose.onNodeWithText("Sign in again").performClick()
        compose.onNodeWithText(SIGN_OUT).performClick()

        assertEquals(listOf(allodiaSignIn(), AllodiaIntent.SignOut), sent)
    }

    // What Settings → Allodia account reads to leave the subscription section out: an ended
    // sign-in has no subscription to read.
    @Test
    fun only_a_sign_in_the_service_accepts_is_signed_in() {
        assertEquals(ALICE, AllodiaSettings(card = ALLODIA_SIGNED_OUT.copy(state = AllodiaCardState.SignedIn(ALICE))).signedIn)
        assertNull(AllodiaSettings(card = ALLODIA_SIGNED_OUT.copy(state = AllodiaCardState.Ended(ALICE))).signedIn)
        assertNull(AllodiaSettings().signedIn)
    }

    @Test
    fun a_failure_is_said_on_the_card() {
        val refused = AllodiaFailure.SignIn(AccountChangeRefusal.Other("invalid_grant"))
        card(ALLODIA_SIGNED_OUT.copy(failure = refused))

        compose.onNodeWithText("Signing in didn’t work: invalid_grant").assertIsDisplayed()
    }

    @Test
    fun each_failure_is_worded_as_the_card_words_it() {
        fun said(failure: AllodiaFailure) = allodiaFailureSaid(ctx, failure)
        fun signIn(refusal: AccountChangeRefusal) = said(AllodiaFailure.SignIn(refusal))

        // A browser returning from another sign-in is remedied by signing in again.
        assertEquals(L10n.signin_stale(ctx), signIn(AccountChangeRefusal.StaleSignIn))
        assertEquals(L10n.settings_allodia_browser_failed(ctx), signIn(AccountChangeRefusal.BrowserNotOpened))
        assertEquals(L10n.settings_allodia_timeout(ctx), signIn(AccountChangeRefusal.NotFinished))
        // Any other refusal in the words an account's page gives it.
        assertEquals(
            L10n.settings_allodia_failed(ctx, L10n.signin_refused(ctx)),
            signIn(AccountChangeRefusal.SignInRefused),
        )
        assertEquals(
            L10n.settings_allodia_failed(ctx, "keystore locked"),
            said(AllodiaFailure.SignOut("keystore locked")),
        )
    }
}
