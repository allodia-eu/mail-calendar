// The Shared mailboxes and calendars section of an account's page as a person meets it
// (docs/shared-mailboxes.md rules 3, 7 and 10), drawn from what the core decided: a switch per
// store saying what it holds, the address field where the server lists none, and switching one off
// confirmed first.
package eu.allodia.mailcal

import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.verticalScroll
import androidx.compose.ui.Modifier
import androidx.compose.ui.test.assert
import androidx.compose.ui.test.assertIsNotEnabled
import androidx.compose.ui.test.assertIsOff
import androidx.compose.ui.test.assertIsOn
import androidx.compose.ui.test.hasText
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
import uniffi.mailcal_bindings.AccountSharedStore
import uniffi.mailcal_bindings.AccountSharedStores
import uniffi.mailcal_bindings.AccountsSnapshot
import uniffi.mailcal_bindings.CapabilityState
import uniffi.mailcal_bindings.ControlState
import uniffi.mailcal_bindings.SharedStoreHolds
import uniffi.mailcal_bindings.SharedStoresNote
import uniffi.mailcal_bindings.AccountKind as CoreAccountKind

@RunWith(RobolectricTestRunner::class)
class SettingsSharedStoresTest {
    @get:Rule val compose = createComposeRule()

    private val state = AccountsUiState()
    private val changes = mutableListOf<AccountPageIntent>()

    private fun mailbox(
        title: String,
        label: String? = null,
        on: Boolean = false,
        withdrawn: Boolean = false,
        holds: SharedStoreHolds = SharedStoreHolds.MAIL,
    ) = AccountSharedStore(title, title, label, holds, withdrawn, on, FREE)

    private val support = mailbox("support@example.org", label = "Support Team")
    private val board = mailbox("Board", on = true)

    private fun show(shared: AccountSharedStores?) {
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
    fun each_store_is_a_switch_named_by_its_address_with_the_servers_label_and_what_it_holds() {
        val group = support.copy(holds = SharedStoreHolds.MAIL_AND_CALENDAR)
        val dave = mailbox("dave@example.org", holds = SharedStoreHolds.CALENDAR)
        show(AccountSharedStores(listOf(board, group, dave), null, null))

        compose.onNodeWithText("Shared mailboxes and calendars").assertExists()
        compose.onNodeWithText("Support Team · Mail and calendar").assertExists()
        compose.onNodeWithTag("account-shared-dave@example.org").performScrollTo()
            .assert(hasText("Calendar"))
        compose.onNodeWithTag("account-shared-Board").assertIsOn()
        compose.onNodeWithTag("account-shared-support@example.org").performScrollTo().assertIsOff()
            .performClick()

        assertEquals(
            listOf(AccountPageIntent.SetSharedStore(mailboxOnly().id, "support@example.org", true)),
            changes,
        )
    }

    @Test
    fun switching_one_off_is_confirmed_first() {
        show(AccountSharedStores(listOf(board), null, null))

        compose.onNodeWithTag("account-shared-Board").performScrollTo().performClick()
        // Nothing is asked of the core until the person confirms what it deletes.
        assertEquals(emptyList<Any>(), changes)
        compose.onNodeWithText("Stop using Board?").assertExists()
        compose.onNodeWithText(
            "Its mail and events are removed from this device. Nothing changes on the server, and you can switch it on again.",
        ).assertExists()
        compose.onNodeWithTag("account-shared-off-confirm").performClick()

        assertEquals(listOf(AccountPageIntent.SetSharedStore(mailboxOnly().id, "Board", false)), changes)
    }

    @Test
    fun a_withdrawn_mailbox_says_so_and_a_running_switch_holds_the_rest() {
        val running = board.copy(control = ControlState(enabled = false, running = true))
        val held = mailbox("old@example.org", withdrawn = true).copy(control = ControlState(enabled = false, running = false))
        show(AccountSharedStores(listOf(running, held), null, null))

        compose.onNodeWithText("No longer shared with this account").assertExists()
        compose.onNodeWithTag("account-shared-Board-running").assertExists()
        compose.onNodeWithTag("account-shared-old@example.org").performScrollTo().assertIsNotEnabled()
    }

    @Test
    fun a_server_that_lists_none_takes_an_address() {
        show(AccountSharedStores(emptyList(), SharedStoresNote.BY_ADDRESS, AccountSharedAddress("", FREE)))

        compose.onNodeWithText(
            "Microsoft does not list what is shared with you, so add a mailbox or calendar by its address.",
        ).assertExists()
        compose.onNodeWithTag("account-shared-add").performScrollTo().assertIsNotEnabled()
        compose.onNodeWithTag("account-shared-address").performScrollTo()
            .performTextInput(" orders@example.org ")
        compose.onNodeWithTag("account-shared-add").performClick()

        assertEquals(listOf(AccountPageIntent.AddSharedStore(mailboxOnly().id, "orders@example.org")), changes)
    }

    @Test
    fun an_address_nothing_answered_to_stays_to_be_corrected() {
        show(
            AccountSharedStores(
                emptyList(),
                SharedStoresNote.BY_ADDRESS,
                AccountSharedAddress("typo@example.org", FREE),
            ),
        )
        compose.onNodeWithText("typo@example.org").performScrollTo().assertExists()
    }

    @Test
    fun nothing_shared_says_so() {
        show(AccountSharedStores(emptyList(), SharedStoresNote.NONE_SHARED, null))
        compose.onNodeWithText("Nothing is shared with this account.").assertExists()
    }

    @Test
    fun a_page_without_the_section_draws_none() {
        show(null)
        compose.onNodeWithText("Shared mailboxes and calendars").assertDoesNotExist()
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
