// An account's Sign-in section as a person meets it (docs/accounts.md rule 17), drawn from a record
// the core would answer: the line, the ways in through a browser, and the password field, which
// sends its password to the core as a change on the page. A browser sign-in still asks the activity.
package eu.allodia.mailcal

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
import org.junit.Assert.assertEquals
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import uniffi.mailcal_bindings.AccountCapability
import uniffi.mailcal_bindings.AccountEntry
import uniffi.mailcal_bindings.AccountPageIntent
import uniffi.mailcal_bindings.AccountPasswordField
import uniffi.mailcal_bindings.AccountSecretKind
import uniffi.mailcal_bindings.AccountSignIn
import uniffi.mailcal_bindings.AccountSignInLine
import uniffi.mailcal_bindings.AccountSignInRoute
import uniffi.mailcal_bindings.AccountsSnapshot
import uniffi.mailcal_bindings.CapabilityState
import uniffi.mailcal_bindings.ControlState
import uniffi.mailcal_bindings.SignInMethod
import uniffi.mailcal_bindings.AccountKind as CoreAccountKind

@RunWith(RobolectricTestRunner::class)
class SettingsAccountSignInTest {
    @get:Rule val compose = createComposeRule()

    private val state = AccountsUiState()
    private val signIns = mutableListOf<Pair<String, Boolean>>()
    private val consents = mutableListOf<String>()
    private val changes = mutableListOf<AccountPageIntent>()

    private fun account(kind: CoreAccountKind = CoreAccountKind.IMAP) =
        accountEntry(kind, AccountCapability.MAIL to CapabilityState.ON, signIn = SignInMethod.PROVIDER)

    private fun section(
        line: AccountSignInLine,
        routes: List<AccountSignInRoute> = emptyList(),
        password: AccountPasswordField? = null,
        switches: Boolean = false,
    ) = AccountSignIn(line = line, routes = routes, control = FREE, password = password, switches = switches)

    private fun show(entry: AccountEntry, section: AccountSignIn?) {
        state.snapshot = AccountsSnapshot(listOf(entry))
        state.page = entry.id
        state.accountPage = pageOf(entry, signIn = section)
        val ui = AccountsSettingsUi(
            state = state,
            onChange = { changes += it },
            onSignInAgain = { id, _ -> consents += id },
            onAccountSignIn = { id, switching -> signIns += id to switching },
        )
        compose.setContent {
            Column(modifier = Modifier.verticalScroll(rememberScrollState())) {
                AccountsSettings(ui, allodiaSyncCard = {}) { shown, record ->
                    AccountPage(shown, record, ui, syncMode = null, onSetSyncMode = {})
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

        assertEquals(listOf(entry.id to true), signIns)
    }

    @Test
    fun a_microsoft_account_signs_in_again_through_its_consent() {
        val entry = account(CoreAccountKind.MICROSOFT)
        show(entry, section(AccountSignInLine.NEEDS_PERMISSION, routes = listOf(AccountSignInRoute.SignInAgain)))

        compose.onNodeWithText("Needs permission: sign in again to allow it.").assertExists()
        compose.onNodeWithTag("account-sign-in-again").performScrollTo().performClick()

        assertEquals(listOf(entry.id), consents)
        assertEquals(emptyList<Pair<String, Boolean>>(), signIns)
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
                password = AccountPasswordField(
                    AccountSecretKind.PASSWORD,
                    replaces = false,
                    save = ControlState(enabled = false, running = true),
                ),
                switches = true,
            ),
        )

        compose.onNodeWithTag("account-switch-to-password").assertDoesNotExist()
        compose.onNodeWithTag("account-password-save").assertIsNotEnabled()
    }

    @Test
    fun while_the_browser_is_open_the_page_waits_and_holds_its_buttons() {
        val entry = account()
        show(
            entry,
            section(
                AccountSignInLine.PASSWORD,
                routes = listOf(AccountSignInRoute.ToProvider(null)),
                switches = true,
            ),
        )
        compose.onNodeWithTag("account-switch-to-provider").assertIsEnabled()

        state.signIn.begin(entry.id, switching = true)
        state.signIn.opened("handle")
        compose.waitForIdle()

        compose.onNodeWithTag("account-switch-to-provider").assertIsNotEnabled()
        compose.onNodeWithText("Continue in your browser to sign in as alice@example.org.").assertExists()
    }
}
