// An account's page for a calendar or address book its sign-in does not reach (docs/accounts.md
// rule 18): the action under exactly the uses the core names, the form it opens, and the question
// a switch to the provider asks before the browser opens.
package eu.allodia.mailcal

import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.verticalScroll
import androidx.compose.ui.Modifier
import androidx.compose.ui.test.junit4.v2.createComposeRule
import androidx.compose.ui.test.onNodeWithTag
import androidx.compose.ui.test.onNodeWithText
import androidx.compose.ui.test.performClick
import androidx.compose.ui.test.performScrollTo
import org.junit.Assert.assertEquals
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.RuntimeEnvironment
import uniffi.mailcal_bindings.AccountCapability.CALENDAR
import uniffi.mailcal_bindings.AccountCapability.CONTACTS
import uniffi.mailcal_bindings.AccountCapability.MAIL
import uniffi.mailcal_bindings.AccountEntry
import uniffi.mailcal_bindings.AccountsSnapshot
import uniffi.mailcal_bindings.CapabilityState
import uniffi.mailcal_bindings.SeparateServer
import uniffi.mailcal_bindings.UseProblem
import uniffi.mailcal_bindings.AccountKind as CoreAccountKind

@RunWith(RobolectricTestRunner::class)
class SettingsSeparateServerTest {
    @get:Rule val compose = createComposeRule()

    private fun ctx() = RuntimeEnvironment.getApplication()

    private val state = AccountsUiState()
    private val opened = mutableListOf<String>()
    private val choices = mutableListOf<UnreachedChoice>()

    private fun mailbox(server: SeparateServer?): AccountEntry {
        val base = accountEntry(
            CoreAccountKind.IMAP,
            MAIL to CapabilityState.ON,
            CALENDAR to CapabilityState.ON,
            CONTACTS to CapabilityState.ON,
        )
        val problem = UseProblem.NEEDS_OWN_SIGN_IN.takeIf { server != null }
        return base.copy(
            uses = base.uses.map { if (it.capability == CALENDAR) it.copy(problem = problem) else it },
            separateServer = server,
        )
    }

    private fun show(entry: AccountEntry) {
        state.snapshot = AccountsSnapshot(listOf(entry))
        state.page = entry.id
        val ui = AccountsSettingsUi(
            state = state,
            onSeparateSignIn = { opened += it },
            onUnreachedChoice = { choices += it },
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

    private val calendar = SeparateServer(listOf(CALENDAR), "dav.example.org", "alice")

    @Test
    fun the_action_sits_under_exactly_the_use_the_core_names() {
        val entry = mailbox(calendar)
        show(entry)

        compose.onNodeWithTag("account-use-separate-CALENDAR").performScrollTo().performClick()
        compose.onNodeWithTag("account-use-separate-CONTACTS").assertDoesNotExist()

        assertEquals(listOf(entry.id), opened)
    }

    @Test
    fun a_use_that_connected_offers_nothing() {
        show(mailbox(null))

        compose.onNodeWithTag("account-use-separate-CALENDAR").assertDoesNotExist()
    }

    @Test
    fun the_form_opened_names_the_server_and_its_login() {
        val entry = mailbox(calendar)
        state.separate.open(UnreachedRules.form(entry)!!)
        show(entry)

        compose.onNodeWithText(L10n.separate_server_title_calendar(ctx())).assertExists()
        compose.onNodeWithText(L10n.separate_server_body(ctx(), "dav.example.org")).assertExists()
        compose.onNodeWithText("alice").assertExists()

        compose.onNodeWithTag("separate-server-cancel").performClick()
        compose.waitForIdle()
        assertEquals(null, state.separate.form)
    }

    @Test
    fun a_switch_that_leaves_the_calendar_behind_asks_on_the_page() {
        val entry = mailbox(null)
        state.signIn.begin(entry.id, switching = true)
        state.signIn.ask(UnreachedQuestion(entry.id, UnreachedSet.CALENDAR, "handle", "https://auth", "redirect"))
        show(entry)

        compose.onNodeWithText(L10n.signin_unreached_title_calendar(ctx())).assertExists()
        compose.onNodeWithTag("signin-unreached-keep").performClick()

        assertEquals(listOf(UnreachedChoice.KEEP), choices)
    }
}
