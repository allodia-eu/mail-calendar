// An account's page for a calendar or address book its sign-in does not reach (docs/accounts.md
// rule 18): the action under exactly the uses the core names, the form it opens and the sign-in it
// offers, and the question a switch to the provider asks before the browser opens.
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
import uniffi.mailcal_bindings.AccountPageIntent
import uniffi.mailcal_bindings.AccountSignIn
import uniffi.mailcal_bindings.AccountSignInLine
import uniffi.mailcal_bindings.AccountsSnapshot
import uniffi.mailcal_bindings.BrowserSignIn
import uniffi.mailcal_bindings.BrowserSignInStage
import uniffi.mailcal_bindings.CapabilityState
import uniffi.mailcal_bindings.AccountSeparateServer
import uniffi.mailcal_bindings.AccountSeparateSignIn
import uniffi.mailcal_bindings.UnreachedUses
import uniffi.mailcal_bindings.UseProblem
import uniffi.mailcal_bindings.AccountKind as CoreAccountKind

@RunWith(RobolectricTestRunner::class)
class SettingsSeparateServerTest {
    @get:Rule val compose = createComposeRule()

    private fun ctx() = RuntimeEnvironment.getApplication()

    private val state = AccountsUiState()
    private val opened = mutableListOf<String>()
    private val changes = mutableListOf<AccountPageIntent>()
    private var signIns = 0

    /** The account's browser sign-in, as the core would put it on the page. */
    private var browser: BrowserSignIn? = null

    private fun mailbox(server: AccountSeparateServer?): AccountEntry {
        val base = accountEntry(
            CoreAccountKind.IMAP,
            MAIL to CapabilityState.ON,
            CALENDAR to CapabilityState.ON,
            CONTACTS to CapabilityState.ON,
        )
        val problem = UseProblem.NEEDS_OWN_SIGN_IN.takeIf { server != null }
        return base.copy(
            uses = base.uses.map { if (it.capability == CALENDAR) it.copy(problem = problem) else it },
        )
    }

    private var section: AccountSeparateServer? = null

    private fun show(entry: AccountEntry) {
        state.snapshot = AccountsSnapshot(listOf(entry))
        state.page = entry.id
        // A mailbox always has a Sign-in section, which the switch's question is asked from.
        state.accountPage = pageOf(
            entry,
            uses = entry.uses.map {
                useSwitch(it.capability, it.state == CapabilityState.ON, separately = section?.uses?.last() == it.capability)
            },
            signIn = AccountSignIn(AccountSignInLine.PASSWORD, emptyList(), FREE, null, switches = false, browser = browser),
            separate = section,
        )
        val ui = AccountsSettingsUi(
            state = state,
            onSeparateSignIn = { opened += it },
            onChange = { changes += it },
            onSeparateProviderSignIn = { signIns++ },
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

    private val calendar = separateSection(CALENDAR)

    @Test
    fun the_action_sits_under_exactly_the_use_the_core_names() {
        val entry = mailbox(calendar)
        section = calendar
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
        section = calendar
        state.separate.open(UnreachedRules.form(entry.id, calendar)!!)
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
        browser = BrowserSignIn(7uL, entry.id, BrowserSignInStage.Asking(listOf(CALENDAR)), FREE)
        show(entry)

        compose.onNodeWithText(L10n.signin_unreached_title_calendar(ctx())).assertExists()
        compose.onNodeWithTag("signin-unreached-keep").performClick()

        assertEquals(
            listOf(AccountPageIntent.AnswerUnreached(entry.id, 7uL, UnreachedUses.KEEP_ON_PASSWORD)),
            changes,
        )
    }

    @Test
    fun the_pages_form_offers_a_sign_in_once_its_server_does() {
        val entry = mailbox(calendar)
        section = calendar
        state.separate.open(UnreachedRules.form(entry.id, calendar)!!)
        show(entry)
        compose.onNodeWithTag("separate-server-sign-in").assertDoesNotExist()

        state.separate.redraw(separateSection(CALENDAR, signIn = AccountSeparateSignIn.PROVIDER))
        compose.waitForIdle()
        compose.onNodeWithTag("separate-server-sign-in").performClick()

        assertEquals(1, signIns)
    }
}
