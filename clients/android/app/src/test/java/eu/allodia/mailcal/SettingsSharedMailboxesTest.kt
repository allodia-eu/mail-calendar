// The Shared mailboxes section of an account's page as a person meets it
// (docs/shared-mailboxes.md rules 3 and 7): asked as the page opens, a switch per mailbox, the
// address field where the server lists none, and switching one off confirmed first.
package eu.allodia.mailcal

import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.verticalScroll
import androidx.compose.ui.Modifier
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
import uniffi.mailcal_bindings.AccountsSnapshot
import uniffi.mailcal_bindings.CapabilityState
import uniffi.mailcal_bindings.SharedMailboxEntry
import uniffi.mailcal_bindings.SharedMailboxSupport
import uniffi.mailcal_bindings.SharedMailboxesView
import uniffi.mailcal_bindings.AccountKind as CoreAccountKind

@RunWith(RobolectricTestRunner::class)
class SettingsSharedMailboxesTest {
    @get:Rule val compose = createComposeRule()

    private val state = AccountsUiState()
    private val asked = mutableListOf<String>()
    private val set = mutableListOf<Triple<String, SharedMailboxEntry, Boolean>>()
    private val added = mutableListOf<Pair<String, String>>()

    private val support = sharedMailbox("Support Team", "support@example.org")
    private val board = sharedMailbox("Board", selected = true)

    private fun show(view: SharedMailboxesView?) {
        val entry = mailboxOnly()
        state.snapshot = AccountsSnapshot(listOf(entry))
        state.page = entry.id
        state.shared.answered(entry.id, view, state.shared.read())
        val ui = AccountsSettingsUi(
            state = state,
            onAskSharedMailboxes = { asked += it },
            onSetSharedMailbox = { id, mailbox, on -> set += Triple(id, mailbox, on) },
            onAddSharedMailbox = { id, address -> added += id to address },
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
    fun the_server_is_asked_as_the_page_opens_and_each_mailbox_is_a_switch() {
        show(sharedView(SharedMailboxSupport.LISTED, board, support))

        assertEquals(listOf(mailboxOnly().id), asked)
        compose.onNodeWithText("Shared mailboxes").assertExists()
        compose.onNodeWithText("Support Team").assertExists()
        compose.onNodeWithTag("account-shared-Board").assertIsOn()
        compose.onNodeWithTag("account-shared-support@example.org").performScrollTo().assertIsOff()
            .performClick()

        assertEquals(listOf(Triple(mailboxOnly().id, support, true)), set)
    }

    @Test
    fun switching_one_off_is_confirmed_first() {
        show(sharedView(SharedMailboxSupport.LISTED, board))

        compose.onNodeWithTag("account-shared-Board").performScrollTo().performClick()
        // Nothing is asked of the core until the person confirms what it deletes.
        assertEquals(emptyList<Any>(), set)
        compose.onNodeWithText("Stop using Board?").assertExists()
        compose.onNodeWithTag("account-shared-off-confirm").performClick()

        assertEquals(listOf(Triple(mailboxOnly().id, board, false)), set)
    }

    @Test
    fun a_server_that_lists_none_takes_an_address() {
        show(sharedView(SharedMailboxSupport.BY_ADDRESS))

        compose.onNodeWithText(
            "Microsoft does not list the mailboxes shared with you, so add one by its address.",
        ).assertExists()
        compose.onNodeWithTag("account-shared-address").performScrollTo()
            .performTextInput(" orders@example.org ")
        compose.onNodeWithTag("account-shared-add").performClick()

        assertEquals(listOf(mailboxOnly().id to "orders@example.org"), added)
    }

    @Test
    fun nothing_shared_says_so() {
        show(sharedView(SharedMailboxSupport.LISTED))
        compose.onNodeWithText("No mailbox is shared with this account.").assertExists()
    }

    @Test
    fun an_account_that_opens_none_has_no_section() {
        show(sharedView(SharedMailboxSupport.UNSUPPORTED))
        compose.onNodeWithText("Shared mailboxes").assertDoesNotExist()
    }

    @Test
    fun the_use_switch_waits_for_mail() {
        val microsoft = accountEntry(
            CoreAccountKind.MICROSOFT,
            AccountCapability.MAIL to CapabilityState.OFF,
            AccountCapability.SHARED_MAILBOXES to CapabilityState.OFF,
            AccountCapability.CALENDAR to CapabilityState.ON,
        )
        state.snapshot = AccountsSnapshot(listOf(microsoft))
        state.page = microsoft.id
        val ui = AccountsSettingsUi(state = state)
        compose.setContent {
            AccountsSettings(ui, allodiaSyncCard = {}) { page ->
                AccountPage(page, ui, syncMode = null, onSetSyncMode = {}) {}
            }
        }
        compose.onNodeWithTag("account-use-SHARED_MAILBOXES").assertIsOff()
        compose.onNodeWithText("Switch on Mail first.").assertExists()
    }
}
