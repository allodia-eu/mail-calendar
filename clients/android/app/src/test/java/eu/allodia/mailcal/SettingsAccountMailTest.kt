// An account's mail settings on its page (docs/settings.md), drawn from the core's page record:
// each control asks the core for its change, and what may be pressed is the record's to say.
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
import androidx.compose.ui.test.performImeAction
import androidx.compose.ui.test.performScrollTo
import androidx.compose.ui.test.performTextReplacement
import org.junit.Assert.assertEquals
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.RuntimeEnvironment
import uniffi.mailcal_bindings.AccountCountChoice
import uniffi.mailcal_bindings.AccountMailSettings
import uniffi.mailcal_bindings.AccountPageIntent
import uniffi.mailcal_bindings.AccountSenderName
import uniffi.mailcal_bindings.AccountToggle
import uniffi.mailcal_bindings.AccountWatchedFolder
import uniffi.mailcal_bindings.AccountsSnapshot
import uniffi.mailcal_bindings.ControlState
import uniffi.mailcal_bindings.FolderRole

@RunWith(RobolectricTestRunner::class)
class SettingsAccountMailTest {
    @get:Rule val compose = createComposeRule()

    private val ctx = RuntimeEnvironment.getApplication()
    private val state = AccountsUiState()
    private val changes = mutableListOf<AccountPageIntent>()
    private val entry = mailboxOnly()
    private val held = ControlState(enabled = false, running = false)

    private fun count(vararg options: Int, selected: Int) =
        AccountCountChoice(options.map { it.toUShort() }, selected.toUInt(), FREE)

    private fun mail(watchedFull: Boolean = false) = AccountMailSettings(
        depth = count(3, 6, 12, 0, selected = 1),
        size = count(2, 5, 0, selected = 1),
        push = AccountToggle(on = true, control = FREE),
        interval = null,
        folders = listOf(
            AccountWatchedFolder("INBOX", "INBOX", FolderRole.INBOX, on = true, control = FREE),
            AccountWatchedFolder("Lists", "Lists", null, on = false, control = if (watchedFull) held else FREE),
        ),
        watchLimit = 1u,
    )

    private fun show(name: AccountSenderName, mail: AccountMailSettings = mail()) {
        state.snapshot = AccountsSnapshot(listOf(entry))
        state.page = entry.id
        state.accountPage = pageOf(entry, senderName = name, mail = mail)
        val ui = AccountsSettingsUi(state = state, onChange = { changes += it })
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
    fun the_name_is_asked_for_on_done_and_not_when_unchanged() {
        show(AccountSenderName("Ada Lovelace", editable = true, control = FREE))
        val field = compose.onNodeWithTag("account-sender-name")
        field.performScrollTo()
        field.performImeAction()
        assertEquals("an unchanged name is not an edit", emptyList<AccountPageIntent>(), changes)

        field.performTextReplacement("Ada King")
        field.performImeAction()
        assertEquals(listOf(AccountPageIntent.SetSenderName(entry.id, "Ada King")), changes)
    }

    @Test
    fun a_name_the_provider_holds_is_shown_without_a_field() {
        show(AccountSenderName("Directory Name", editable = false, control = held))
        compose.onNodeWithText("Directory Name").assertExists()
        compose.onNodeWithText(L10n.settings_sender_name_managed(ctx)).assertExists()
        compose.onNodeWithTag("account-sender-name").assertDoesNotExist()
    }

    @Test
    fun a_count_picked_asks_for_that_count() {
        show(AccountSenderName("", editable = true, control = FREE))
        compose.onNodeWithTag("account-sync-depth").performScrollTo().performClick()
        compose.onNodeWithText(L10n.sync_depth_months(ctx, 12)).performClick()
        assertEquals(listOf(AccountPageIntent.SetSyncDepth(entry.id, 12u)), changes)
    }

    @Test
    fun a_folder_the_record_holds_cannot_be_pressed() {
        show(AccountSenderName("", editable = true, control = FREE), mail(watchedFull = true))
        compose.onNodeWithTag("account-watched-INBOX").performScrollTo().assertIsEnabled()
        compose.onNodeWithTag("account-watched-Lists").performScrollTo().assertIsNotEnabled()
        compose.onNodeWithTag("account-watched-INBOX").performClick()
        assertEquals(listOf(AccountPageIntent.SetWatchedFolder(entry.id, "INBOX", false)), changes)
    }
}
