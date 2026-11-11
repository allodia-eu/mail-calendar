// How an account is shared with the other devices, at the top of its page (docs/settings.md row
// 10, docs/waiting.md): drawn from the page's `sharing`, changed as a change on the page, and
// held, running and failed as the record says.
package eu.allodia.mailcal

import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.verticalScroll
import androidx.compose.ui.Modifier
import androidx.compose.ui.test.assertIsEnabled
import androidx.compose.ui.test.assertIsNotEnabled
import androidx.compose.ui.test.assertIsSelected
import androidx.compose.ui.test.junit4.v2.createComposeRule
import androidx.compose.ui.test.onNodeWithTag
import androidx.compose.ui.test.onNodeWithText
import androidx.compose.ui.test.performClick
import org.junit.Assert.assertEquals
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.RuntimeEnvironment
import uniffi.mailcal_bindings.AccountPageIntent
import uniffi.mailcal_bindings.AccountSharing
import uniffi.mailcal_bindings.AccountsSnapshot
import uniffi.mailcal_bindings.AllodiaAccountSyncMode
import uniffi.mailcal_bindings.AllodiaSyncFailure
import uniffi.mailcal_bindings.ControlState

@RunWith(RobolectricTestRunner::class)
class SettingsAccountSharingTest {
    @get:Rule val compose = createComposeRule()

    private val ctx = RuntimeEnvironment.getApplication()
    private val state = AccountsUiState()
    private val changes = mutableListOf<AccountPageIntent>()
    private val entry = mailboxOnly()

    private fun show(sharing: AccountSharing?) {
        state.snapshot = AccountsSnapshot(listOf(entry))
        state.page = entry.id
        state.accountPage = pageOf(entry, sharing = sharing)
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

    private fun sharing(
        mode: AllodiaAccountSyncMode = AllodiaAccountSyncMode.ON,
        control: ControlState = FREE,
        failed: AllodiaSyncFailure? = null,
    ) = AccountSharing(mode, control, failed)

    /** Nobody signed in, or an account the other devices cannot hold: the page has no control. */
    @Test
    fun a_page_without_sharing_draws_no_control() {
        show(null)
        compose.onNodeWithTag("account-sharing").assertDoesNotExist()
        compose.onNodeWithText(L10n.settings_account_sync_heading(ctx)).assertDoesNotExist()
    }

    @Test
    fun a_position_picked_is_a_change_on_the_page() {
        show(sharing())
        compose.onNodeWithTag("account-sharing-0").assertIsSelected()
        compose.onNodeWithText(L10n.settings_account_sync_on_hint(ctx)).assertExists()

        compose.onNodeWithTag("account-sharing-1").performClick()
        compose.onNodeWithTag("account-sharing-0").performClick()

        assertEquals(
            "the position in force is not a change",
            listOf(AccountPageIntent.SetSharing(entry.id, AllodiaAccountSyncMode.PAUSED)),
            changes,
        )
    }

    /** While a change runs the core draws where it is going, held, with the spinner beside it. */
    @Test
    fun a_change_running_draws_its_target_held_with_a_spinner() {
        show(sharing(AllodiaAccountSyncMode.OFF, ControlState(enabled = false, running = true)))
        compose.onNodeWithTag("account-sharing-running").assertExists()
        compose.onNodeWithTag("account-sharing-2").assertIsSelected().assertIsNotEnabled()
        compose.onNodeWithText(L10n.settings_account_sync_off_hint(ctx)).assertExists()

        compose.onNodeWithTag("account-sharing-0").performClick()
        assertEquals(emptyList<AccountPageIntent>(), changes)
    }

    /** Another change on the page holds it without claiming this one runs. */
    @Test
    fun a_held_control_has_no_spinner() {
        show(sharing(control = HELD))
        compose.onNodeWithTag("account-sharing-running").assertDoesNotExist()
        compose.onNodeWithTag("account-sharing-1").assertIsNotEnabled()
    }

    /** What did not take is said under the control, in the sign-in's words, never the error's. */
    @Test
    fun a_change_that_did_not_take_says_so_under_the_control() {
        show(sharing(failed = AllodiaSyncFailure.UNAVAILABLE))
        compose.onNodeWithText(L10n.settings_account_sync_failed(ctx)).assertExists()
        compose.onNodeWithTag("account-sharing-1").assertIsEnabled()
    }

    @Test
    fun a_refused_sign_in_is_said_as_one() {
        show(sharing(failed = AllodiaSyncFailure.NEEDS_REAUTH))
        compose.onNodeWithText(L10n.settings_allodia_reauth(ctx)).assertExists()
        compose.onNodeWithText(L10n.settings_account_sync_failed(ctx)).assertDoesNotExist()
    }
}
