// An account's Sign-in section as a person meets it (docs/accounts.md rule 17), drawn from a record
// the core would answer: the line, the ways in through a browser, the browser sign-in the core
// runs with its waiting line and Cancel, and the password field, which sends its password to the
// core as a change on the page. What a sign-in came to is the core's notice, worded here.
package eu.allodia.mailcal

import android.content.Context
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.verticalScroll
import androidx.compose.ui.Modifier
import androidx.compose.ui.test.assertIsEnabled
import androidx.compose.ui.test.assertIsNotEnabled
import androidx.compose.ui.test.junit4.v2.createComposeRule
import androidx.compose.ui.test.onNodeWithTag
import androidx.compose.ui.test.onNodeWithText
import androidx.compose.ui.test.performClick
import androidx.compose.ui.test.performScrollTo
import androidx.compose.ui.test.performTextInput
import androidx.test.core.app.ApplicationProvider
import org.junit.Assert.assertEquals
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import uniffi.mailcal_bindings.AccountCapability
import uniffi.mailcal_bindings.AccountChangeNotice
import uniffi.mailcal_bindings.AccountChangeRefusal
import uniffi.mailcal_bindings.AccountEntry
import uniffi.mailcal_bindings.AccountPageIntent
import uniffi.mailcal_bindings.AccountPasswordField
import uniffi.mailcal_bindings.AccountSecretKind
import uniffi.mailcal_bindings.AccountSignIn
import uniffi.mailcal_bindings.AccountSignInLine
import uniffi.mailcal_bindings.AccountSignInRoute
import uniffi.mailcal_bindings.AccountsSnapshot
import uniffi.mailcal_bindings.BrowserSignIn
import uniffi.mailcal_bindings.BrowserSignInStage
import uniffi.mailcal_bindings.CapabilityState
import uniffi.mailcal_bindings.ControlState
import uniffi.mailcal_bindings.SignInMethod
import uniffi.mailcal_bindings.AccountKind as CoreAccountKind

@RunWith(RobolectricTestRunner::class)
class SettingsAccountSignInTest {
    @get:Rule val compose = createComposeRule()

    private val state = AccountsUiState()
    private val signIns = mutableListOf<Pair<String, List<AccountCapability>>>()
    private val changes = mutableListOf<AccountPageIntent>()

    private val running = ControlState(enabled = false, running = true)

    private fun ctx() = ApplicationProvider.getApplicationContext<Context>()

    private fun account(kind: CoreAccountKind = CoreAccountKind.IMAP) =
        accountEntry(kind, AccountCapability.MAIL to CapabilityState.ON, signIn = SignInMethod.PROVIDER)

    private fun section(
        line: AccountSignInLine,
        routes: List<AccountSignInRoute> = emptyList(),
        control: ControlState = FREE,
        password: AccountPasswordField? = null,
        switches: Boolean = false,
        browser: BrowserSignIn? = null,
    ) = AccountSignIn(line, routes, control, password, switches, browser)

    private fun inBrowser(entry: AccountEntry, redirect: String? = "eu.allodia.mailcal://imap-oauth") =
        BrowserSignIn(4uL, entry.id, BrowserSignInStage.InBrowser("https://auth.example.org/authorize", redirect), FREE)

    private fun show(entry: AccountEntry, section: AccountSignIn?) {
        state.snapshot = AccountsSnapshot(listOf(entry))
        state.page = entry.id
        state.accountPage = pageOf(entry, signIn = section)
        val ui = AccountsSettingsUi(
            state = state,
            onChange = { changes += it },
            onSignIn = { id, adding -> signIns += id to adding },
        )
        compose.setContent {
            Column(modifier = Modifier.verticalScroll(rememberScrollState())) {
                AccountsSettings(ui, allodiaSyncCard = {}) { shown, record ->
                    AccountPage(shown, record, ui)
                }
            }
        }
        compose.waitForIdle()
    }

    @Test
    fun the_page_says_how_it_signs_in_and_offers_only_what_the_core_gives() {
        show(account(), section(AccountSignInLine.PASSWORD))

        compose.onNodeWithText("Sign-in").assertExists()
        compose.onNodeWithText("Signs in with a password.").assertExists()
        compose.onNodeWithTag("account-switch-to-provider").assertDoesNotExist()
        compose.onNodeWithTag("account-sign-in-cancel").assertDoesNotExist()
        compose.onNodeWithText("Its mail, settings and linked accounts stay as they are.").assertDoesNotExist()
    }

    @Test
    fun a_calendar_account_without_a_section_draws_none() {
        show(account(CoreAccountKind.DAV), null)

        compose.onNodeWithText("Sign-in").assertDoesNotExist()
    }

    @Test
    fun a_password_account_switches_to_the_provider_its_server_names() {
        val entry = account()
        show(
            entry,
            section(
                AccountSignInLine.PASSWORD,
                routes = listOf(AccountSignInRoute.ToProvider("Fastmail")),
                switches = true,
            ),
        )

        compose.onNodeWithText("Sign in with Fastmail instead").performScrollTo().performClick()
        compose.onNodeWithText("Its mail, settings and linked accounts stay as they are.").assertExists()

        assertEquals(listOf(entry.id to emptyList<AccountCapability>()), signIns)
    }

    @Test
    fun a_microsoft_account_signs_in_again_through_the_core() {
        val entry = account(CoreAccountKind.MICROSOFT)
        show(entry, section(AccountSignInLine.NEEDS_PERMISSION, routes = listOf(AccountSignInRoute.SignInAgain)))

        compose.onNodeWithText("Needs permission: sign in again to allow it.").assertExists()
        compose.onNodeWithTag("account-sign-in-again").performScrollTo().performClick()

        assertEquals(listOf(entry.id to emptyList<AccountCapability>()), signIns)
    }

    @Test
    fun while_the_browser_is_open_the_page_waits_and_offers_cancel() {
        val entry = account()
        val browser = inBrowser(entry)
        show(
            entry,
            section(
                AccountSignInLine.PASSWORD,
                routes = listOf(AccountSignInRoute.ToProvider(null)),
                switches = true,
                browser = browser,
            ),
        )

        compose.onNodeWithText("Continue in your browser to sign in as alice@example.org.").assertExists()
        compose.onNodeWithTag("account-sign-in-cancel").performScrollTo().assertIsEnabled().performClick()
        assertEquals(listOf(AccountPageIntent.CancelSignIn(entry.id, browser.attempt)), changes)

        // A route pressed while the browser waits starts it again (docs/provider-oauth.md rule 13).
        compose.onNodeWithTag("account-switch-to-provider").performScrollTo().assertIsEnabled().performClick()
        assertEquals(listOf(entry.id to emptyList<AccountCapability>()), signIns)
    }

    @Test
    fun a_nextcloud_sign_in_says_it_waits_on_the_server() {
        val entry = account(CoreAccountKind.DAV)
        show(
            entry,
            section(
                AccountSignInLine.NEXTCLOUD,
                routes = listOf(AccountSignInRoute.SignInAgain),
                browser = inBrowser(entry, redirect = null),
            ),
        )

        compose.onNodeWithText(L10n.nextcloud_sign_in_waiting(ctx())).assertExists()
        compose.onNodeWithTag("account-sign-in-cancel").assertExists()
    }

    @Test
    fun a_sign_in_the_core_starts_or_completes_spins_its_route_and_offers_no_cancel() {
        val entry = account(CoreAccountKind.GOOGLE)
        val completing = BrowserSignIn(4uL, entry.id, BrowserSignInStage.Completing, ControlState(enabled = false, running = false))
        show(
            entry,
            section(
                AccountSignInLine.PROVIDER,
                routes = listOf(AccountSignInRoute.SignInAgain),
                control = running,
                browser = completing,
            ),
        )

        compose.onNodeWithTag("account-sign-in-again").assertIsNotEnabled()
        compose.onNodeWithTag("account-sign-in-again-running", useUnmergedTree = true).assertExists()
        compose.onNodeWithTag("account-sign-in-cancel").assertDoesNotExist()
        compose.onNodeWithText("Continue in your browser", substring = true).assertDoesNotExist()
    }

    @Test
    fun a_switch_that_leaves_the_contacts_behind_asks_and_cancel_sends_no_answer() {
        val entry = account()
        val asking = BrowserSignIn(9uL, entry.id, BrowserSignInStage.Asking(listOf(AccountCapability.CONTACTS)), FREE)
        show(entry, section(AccountSignInLine.PASSWORD, routes = listOf(AccountSignInRoute.ToProvider(null)), browser = asking))

        compose.onNodeWithText(L10n.signin_unreached_title_contacts(ctx())).assertExists()
        compose.onNodeWithTag("signin-unreached-cancel").performClick()

        assertEquals(listOf(AccountPageIntent.AnswerUnreached(entry.id, 9uL, null)), changes)
    }

    @Test
    fun a_password_in_place_of_signing_in_is_a_change_on_the_page() {
        val entry = account()
        show(
            entry,
            section(
                AccountSignInLine.PROVIDER,
                routes = listOf(AccountSignInRoute.SignInAgain),
                password = AccountPasswordField(AccountSecretKind.PASSWORD, replaces = false, save = FREE),
                switches = true,
            ),
        )

        compose.onNodeWithTag("account-password-save").assertDoesNotExist()
        compose.onNodeWithTag("account-switch-to-password").performScrollTo().performClick()
        compose.waitForIdle()
        compose.onNodeWithTag("account-password-save").assertIsNotEnabled()
        compose.onNodeWithText("Password").performTextInput("hunter2")
        compose.onNodeWithTag("account-password-save").performScrollTo().performClick()

        val sent = changes.single() as AccountPageIntent.UsePassword
        assertEquals(entry.id, sent.account)
        assertEquals("hunter2", sent.password.text)
        compose.onNodeWithText("•••••••", substring = true).assertDoesNotExist()
    }

    @Test
    fun a_refused_password_is_typed_again_at_once() {
        val entry = account(CoreAccountKind.JMAP)
        show(
            entry,
            section(
                AccountSignInLine.REFUSED,
                password = AccountPasswordField(AccountSecretKind.PASSWORD_OR_TOKEN, replaces = true, save = FREE),
            ),
        )

        compose.onNodeWithText("The sign-in for alice@example.org no longer works. Sign in again to keep syncing.")
            .assertExists()
        compose.onNodeWithText("Password or API token").performTextInput("token")
        compose.onNodeWithTag("account-password-save").performScrollTo().performClick()

        assertEquals("token", (changes.single() as AccountPageIntent.UsePassword).password.text)
    }

    @Test
    fun a_password_being_tried_keeps_its_field_open_and_held() {
        show(
            account(),
            section(
                AccountSignInLine.PROVIDER,
                password = AccountPasswordField(AccountSecretKind.PASSWORD, replaces = false, save = running),
                switches = true,
            ),
        )

        compose.onNodeWithTag("account-switch-to-password").assertDoesNotExist()
        compose.onNodeWithTag("account-password-save").assertIsNotEnabled()
    }

    @Test
    fun what_a_sign_in_came_to_is_worded_from_the_core_s_notice() {
        val ctx = ctx()

        val timedOut = changeNotice(ctx, AccountChangeNotice.SignInFailed(true, AccountChangeRefusal.NotFinished))
        assertEquals(
            AccountNotice(NoticeKind.FAILURE, L10n.settings_account_switch_failed(ctx), L10n.account_signin_timeout(ctx)),
            timedOut,
        )
        val notOpened = changeNotice(ctx, AccountChangeNotice.SignInFailed(false, AccountChangeRefusal.BrowserNotOpened))
        assertEquals(
            AccountNotice(
                NoticeKind.FAILURE,
                L10n.settings_account_signin_failed(ctx),
                L10n.account_signin_browser_failed(ctx),
            ),
            notOpened,
        )
        val withheld = changeNotice(ctx, AccountChangeNotice.SignedInWithheld(listOf(AccountCapability.CALENDAR)))
        assertEquals(NoticeKind.WARNING, withheld.kind)
        assertEquals(L10n.setup_withheld_title(ctx), withheld.title)
        assertEquals(L10n.setup_withheld_detail(ctx, useName(ctx, AccountCapability.CALENDAR)), withheld.message)
    }
}
