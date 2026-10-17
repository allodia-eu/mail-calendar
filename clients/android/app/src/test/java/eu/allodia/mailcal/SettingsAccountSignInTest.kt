// An IMAP or JMAP account's Sign-in section as a person meets it (docs/accounts.md rule 17): how it
// signs in now, and only once its server has answered, the way back in or the other way in. Each
// control asks the activity; nothing here changes the account by itself.
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
import uniffi.mailcal_bindings.AccountSignInOptions
import uniffi.mailcal_bindings.AccountsSnapshot
import uniffi.mailcal_bindings.CapabilityState
import uniffi.mailcal_bindings.SignInMethod
import uniffi.mailcal_bindings.AccountKind as CoreAccountKind

@RunWith(RobolectricTestRunner::class)
class SettingsAccountSignInTest {
    @get:Rule val compose = createComposeRule()

    private val state = AccountsUiState()
    private val asked = mutableListOf<String>()
    private val signIns = mutableListOf<Pair<String, Boolean>>()
    private val passwords = mutableListOf<Pair<String, String>>()

    private fun account(signIn: SignInMethod, kind: CoreAccountKind = CoreAccountKind.IMAP) =
        accountEntry(kind, AccountCapability.MAIL to CapabilityState.ON, signIn = signIn)

    private fun show(entry: AccountEntry, options: AccountSignInOptions? = null) {
        state.snapshot = AccountsSnapshot(listOf(entry))
        state.page = entry.id
        options?.let { state.signIn.answered(entry.id, it) }
        val ui = AccountsSettingsUi(
            state = state,
            onAskSignInOptions = { asked += it },
            onAccountSignIn = { id, switching -> signIns += id to switching },
            onSwitchToPassword = { id, password -> passwords += id to password },
        )
        compose.setContent {
            Column(modifier = Modifier.verticalScroll(rememberScrollState())) {
                AccountsSettings(ui, allodiaSyncCard = {}) { page ->
                    AccountPage(page, ui, syncMode = null, onSetSyncMode = {}) {}
                }
            }
        }
        compose.waitForIdle()
    }

    @Test
    fun the_page_says_how_it_signs_in_and_asks_the_server_before_offering_anything() {
        val entry = account(SignInMethod.PASSWORD)
        show(entry)

        compose.onNodeWithText("Sign-in").assertExists()
        compose.onNodeWithText("Signs in with a password.").assertExists()
        assertEquals(listOf(entry.id), asked)
        compose.onNodeWithTag("account-switch-to-provider").assertDoesNotExist()
        compose.onNodeWithText("Its mail, settings and linked accounts stay as they are.")
            .assertDoesNotExist()
    }

    @Test
    fun a_password_account_switches_to_the_provider_its_server_names() {
        val entry = account(SignInMethod.PASSWORD)
        show(entry, AccountSignInOptions(signIn = true, providerLabel = "Fastmail", password = true))

        compose.onNodeWithText("Sign in with Fastmail instead").performScrollTo().performClick()
        compose.onNodeWithText("Its mail, settings and linked accounts stay as they are.").assertExists()
        // A password account is not offered a password.
        compose.onNodeWithTag("account-switch-to-password").assertDoesNotExist()

        assertEquals(listOf(entry.id to true), signIns)
    }

    @Test
    fun a_signed_in_account_signs_in_again_without_switching() {
        val entry = account(SignInMethod.PROVIDER, CoreAccountKind.JMAP)
        show(entry, AccountSignInOptions(signIn = true, providerLabel = null, password = false))

        compose.onNodeWithText("Signs in through your provider.").assertExists()
        compose.onNodeWithText("Sign in again").performScrollTo().performClick()
        compose.onNodeWithText("Its mail, settings and linked accounts stay as they are.")
            .assertDoesNotExist()

        assertEquals(listOf(entry.id to false), signIns)
    }

    @Test
    fun a_password_in_place_of_signing_in_starts_from_an_empty_field() {
        val entry = account(SignInMethod.PROVIDER)
        show(entry, AccountSignInOptions(signIn = true, providerLabel = null, password = true))

        compose.onNodeWithTag("account-switch-password-save").assertDoesNotExist()
        compose.onNodeWithTag("account-switch-to-password").performScrollTo().performClick()
        compose.waitForIdle()
        compose.onNodeWithTag("account-switch-password-save").assertIsNotEnabled()
        compose.onNodeWithText("Password").performTextInput("hunter2")
        compose.onNodeWithTag("account-switch-password-save").performScrollTo().performClick()

        assertEquals(listOf(entry.id to "hunter2"), passwords)
    }

    @Test
    fun a_switched_password_leaves_the_field() {
        val entry = account(SignInMethod.PROVIDER)
        val options = AccountSignInOptions(signIn = true, providerLabel = null, password = true)
        show(entry, options)
        compose.onNodeWithTag("account-switch-to-password").performScrollTo().performClick()
        compose.onNodeWithText("Password").performTextInput("hunter2")
        compose.onNodeWithText("•••••••", substring = true).assertExists()

        state.signIn.beginPassword(entry.id)
        state.signIn.finished(switched = true)
        // Asked again, the server still offers the switch: it starts closed and empty.
        state.signIn.answered(entry.id, options)
        compose.waitForIdle()

        compose.onNodeWithText("•••••••", substring = true).assertDoesNotExist()
        compose.onNodeWithTag("account-switch-password-save").assertDoesNotExist()
        compose.onNodeWithTag("account-switch-to-password").assertExists()
    }

    @Test
    fun while_the_browser_is_open_the_page_waits_and_holds_its_buttons() {
        val entry = account(SignInMethod.PASSWORD)
        show(entry, AccountSignInOptions(signIn = true, providerLabel = null, password = true))
        compose.onNodeWithTag("account-switch-to-provider").assertIsEnabled()

        state.signIn.begin(entry.id, switching = true)
        state.signIn.opened("handle")
        compose.waitForIdle()

        compose.onNodeWithTag("account-switch-to-provider").assertIsNotEnabled()
        compose.onNodeWithText("Continue in your browser to sign in as alice@example.org.").assertExists()
    }

    @Test
    fun an_account_without_mail_has_no_sign_in_to_switch() {
        val entry = accountEntry(
            CoreAccountKind.DAV,
            AccountCapability.MAIL to CapabilityState.OFF,
            AccountCapability.CALENDAR to CapabilityState.ON,
            id = "alice@dav:cloud.example",
        )
        show(entry)

        compose.onNodeWithText("Sign-in").assertDoesNotExist()
        assertEquals(emptyList<String>(), asked)
    }
}
