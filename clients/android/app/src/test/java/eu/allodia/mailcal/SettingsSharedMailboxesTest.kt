// The Shared mailboxes section of an account's page as a person meets it
// (docs/shared-mailboxes.md rules 3 and 7), drawn from what the core decided: a switch per
// mailbox, the address field where the server lists none, and switching one off confirmed first.
package eu.allodia.mailcal

import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.verticalScroll
import androidx.compose.ui.Modifier
import androidx.compose.ui.test.assertIsNotEnabled
import androidx.compose.ui.test.assertIsOff
import androidx.compose.ui.test.assertIsOn
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
import uniffi.mailcal_bindings.AccountPageIntent
import uniffi.mailcal_bindings.AccountSharedAddress
import uniffi.mailcal_bindings.AccountSharedMailbox
import uniffi.mailcal_bindings.AccountSharedMailboxes
import uniffi.mailcal_bindings.AccountsSnapshot
import uniffi.mailcal_bindings.CapabilityState
import uniffi.mailcal_bindings.ControlState
import uniffi.mailcal_bindings.SharedMailboxesNote
import uniffi.mailcal_bindings.AccountKind as CoreAccountKind

@RunWith(RobolectricTestRunner::class)
class SettingsSharedMailboxesTest {
    @get:Rule val compose = createComposeRule()

    private val state = AccountsUiState()
    private val changes = mutableListOf<AccountPageIntent>()

    private fun mailbox(title: String, label: String? = null, on: Boolean = false, withdrawn: Boolean = false) =
        AccountSharedMailbox(title, title, label, withdrawn, on, FREE)

    private val support = mailbox("support@example.org", label = "Support Team")
    private val board = mailbox("Board", on = true)

    private fun show(shared: AccountSharedMailboxes?) {
        val entry = mailboxOnly()
        state.snapshot = AccountsSnapshot(listOf(entry))
        state.page = entry.id
        state.accountPage = pageOf(entry, shared = shared)
        val ui = AccountsSettingsUi(state = state, onChange = { changes += it })
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
    fun each_mailbox_is_a_switch_named_by_its_address_with_the_servers_label() {
        show(AccountSharedMailboxes(listOf(board, support), null, null))

        compose.onNodeWithText("Shared mailboxes").assertExists()
        compose.onNodeWithText("Support Team").assertExists()
        compose.onNodeWithTag("account-shared-Board").assertIsOn()
        compose.onNodeWithTag("account-shared-support@example.org").performScrollTo().assertIsOff()
            .performClick()

        assertEquals(
            listOf(AccountPageIntent.SetSharedMailbox(mailboxOnly().id, "support@example.org", true)),
            changes,
        )
    }

    @Test
    fun switching_one_off_is_confirmed_first() {
        show(AccountSharedMailboxes(listOf(board), null, null))

        compose.onNodeWithTag("account-shared-Board").performScrollTo().performClick()
        // Nothing is asked of the core until the person confirms what it deletes.
        assertEquals(emptyList<Any>(), changes)
        compose.onNodeWithText("Stop using Board?").assertExists()
        compose.onNodeWithTag("account-shared-off-confirm").performClick()

        assertEquals(listOf(AccountPageIntent.SetSharedMailbox(mailboxOnly().id, "Board", false)), changes)
    }

    @Test
    fun a_withdrawn_mailbox_says_so_and_a_running_switch_holds_the_rest() {
        val running = board.copy(control = ControlState(enabled = false, running = true))
        val held = mailbox("old@example.org", withdrawn = true).copy(control = ControlState(enabled = false, running = false))
        show(AccountSharedMailboxes(listOf(running, held), null, null))

        compose.onNodeWithText("No longer shared with this account").assertExists()
        compose.onNodeWithTag("account-shared-Board-running").assertExists()
        compose.onNodeWithTag("account-shared-old@example.org").performScrollTo().assertIsNotEnabled()
    }

    @Test
    fun a_server_that_lists_none_takes_an_address() {
        show(AccountSharedMailboxes(emptyList(), SharedMailboxesNote.BY_ADDRESS, AccountSharedAddress("", FREE)))

        compose.onNodeWithText(
            "Microsoft does not list the mailboxes shared with you, so add one by its address.",
        ).assertExists()
        compose.onNodeWithTag("account-shared-add").performScrollTo().assertIsNotEnabled()
        compose.onNodeWithTag("account-shared-address").performScrollTo()
            .performTextInput(" orders@example.org ")
        compose.onNodeWithTag("account-shared-add").performClick()

        assertEquals(listOf(AccountPageIntent.AddSharedMailbox(mailboxOnly().id, "orders@example.org")), changes)
    }

    @Test
    fun an_address_nothing_answered_to_stays_to_be_corrected() {
        show(
            AccountSharedMailboxes(
                emptyList(),
                SharedMailboxesNote.BY_ADDRESS,
                AccountSharedAddress("typo@example.org", FREE),
            ),
        )
        compose.onNodeWithText("typo@example.org").performScrollTo().assertExists()
    }

    @Test
    fun nothing_shared_says_so() {
        show(AccountSharedMailboxes(emptyList(), SharedMailboxesNote.NONE_SHARED, null))
        compose.onNodeWithText("No mailbox is shared with this account.").assertExists()
    }

    @Test
    fun a_page_without_the_section_draws_none() {
        show(null)
        compose.onNodeWithText("Shared mailboxes").assertDoesNotExist()
    }

    @Test
    fun the_use_switch_says_the_core_holds_it_for_mail() {
        val microsoft = accountEntry(
            CoreAccountKind.MICROSOFT,
            AccountCapability.MAIL to CapabilityState.OFF,
            AccountCapability.SHARED_MAILBOXES to CapabilityState.OFF,
            AccountCapability.CALENDAR to CapabilityState.ON,
        )
        state.snapshot = AccountsSnapshot(listOf(microsoft))
        state.page = microsoft.id
        val held = uniffi.mailcal_bindings.ControlState(enabled = false, running = false)
        state.accountPage = pageOf(
            microsoft,
            uses = listOf(
                useSwitch(AccountCapability.MAIL, on = false),
                useSwitch(
                    AccountCapability.SHARED_MAILBOXES,
                    on = false,
                    control = held,
                    blocked = uniffi.mailcal_bindings.AccountUseBlock.NEEDS_MAIL,
                ),
            ),
        )
        val ui = AccountsSettingsUi(state = state)
        compose.setContent {
            AccountsSettings(ui, allodiaSyncCard = {}) { shown, record ->
                AccountPage(shown, record, ui)
            }
        }
        compose.onNodeWithTag("account-use-SHARED_MAILBOXES").assertIsOff().assertIsNotEnabled()
        compose.onNodeWithText("Switch on Mail first.").assertExists()
    }
}
