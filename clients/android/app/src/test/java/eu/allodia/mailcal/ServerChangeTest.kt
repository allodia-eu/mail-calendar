// A calendar or address book server found on another site (docs/account-autodetect.md rule 14):
// the dialog names what moved, both sites in bold, and allows the new server only from its Allow
// button; Not now, back and a tap outside leave the use paused. What it says is the core's record,
// which these stand in for.
package eu.allodia.mailcal

import android.content.Context
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.verticalScroll
import androidx.compose.ui.Modifier
import androidx.compose.ui.test.assertIsDisplayed
import androidx.compose.ui.test.assertIsEnabled
import androidx.compose.ui.test.assertIsNotEnabled
import androidx.compose.ui.test.junit4.v2.createComposeRule
import androidx.compose.ui.test.onNodeWithTag
import androidx.compose.ui.test.onNodeWithText
import androidx.compose.ui.test.performClick
import androidx.compose.ui.text.font.FontWeight
import androidx.test.core.app.ApplicationProvider
import org.junit.Assert.assertEquals
import org.junit.Assert.assertNotNull
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import uniffi.mailcal_bindings.AccountCapability
import uniffi.mailcal_bindings.AccountServerChange
import uniffi.mailcal_bindings.AccountUseStatus
import uniffi.mailcal_bindings.AccountsSnapshot
import uniffi.mailcal_bindings.CapabilityState
import uniffi.mailcal_bindings.ControlState
import uniffi.mailcal_bindings.LinkedAccount
import uniffi.mailcal_bindings.ServerChange
import uniffi.mailcal_bindings.UseProblem
import uniffi.mailcal_bindings.AccountKind as CoreAccountKind

@RunWith(RobolectricTestRunner::class)
class ServerChangeTest {
    @get:Rule val compose = createComposeRule()

    private fun ctx(): Context = ApplicationProvider.getApplicationContext()

    private fun change(calendar: Boolean, contacts: Boolean) =
        ServerChange("icloud.com", "example.net", calendar, contacts)

    @Test
    fun the_copy_names_what_moved() {
        val address = "ada@example.com"
        val cases = listOf(
            change(calendar = true, contacts = true) to ServerChangeCopy(
                L10n.server_changed_title_both(ctx()),
                L10n.server_changed_body_both(ctx(), address),
                L10n.server_changed_before(ctx(), "icloud.com"),
                L10n.server_changed_now(ctx(), "example.net"),
                L10n.server_changed_advice(ctx()),
                L10n.server_changed_unsure_both(ctx()),
            ),
            change(calendar = true, contacts = false) to ServerChangeCopy(
                L10n.server_changed_title_calendar(ctx()),
                L10n.server_changed_body_calendar(ctx(), address),
                L10n.server_changed_before(ctx(), "icloud.com"),
                L10n.server_changed_now(ctx(), "example.net"),
                L10n.server_changed_advice(ctx()),
                L10n.server_changed_unsure_calendar(ctx()),
            ),
            change(calendar = false, contacts = true) to ServerChangeCopy(
                L10n.server_changed_title_contacts(ctx()),
                L10n.server_changed_body_contacts(ctx(), address),
                L10n.server_changed_before(ctx(), "icloud.com"),
                L10n.server_changed_now(ctx(), "example.net"),
                L10n.server_changed_advice(ctx()),
                L10n.server_changed_unsure_contacts(ctx()),
            ),
        )
        for ((moved, expected) in cases) {
            assertEquals("$moved", expected, ServerChangeRules.copy(ctx(), moved, address))
        }
    }

    @Test
    fun the_english_dialog_names_the_account_and_both_sites() {
        val copy = ServerChangeRules.copy(ctx(), change(calendar = true, contacts = false), "ada@example.com")
        assertEquals("Your calendar server has changed", copy.title)
        assertEquals("The calendar for ada@example.com now points to a different server:", copy.body)
        assertEquals("Before: icloud.com", copy.before)
        assertEquals("Now: example.net", copy.now)
    }

    @Test
    fun the_site_is_the_bold_part_of_its_line() {
        val line = ServerChangeRules.emphasised("Before: icloud.com", "icloud.com")
        val span = line.spanStyles.single()
        assertEquals(FontWeight.Bold, span.item.fontWeight)
        assertEquals("icloud.com", line.text.substring(span.start, span.end))
        assertTrue(ServerChangeRules.emphasised("Before:", "icloud.com").spanStyles.isEmpty())
    }

    private fun question(
        change: ServerChange = change(calendar = true, contacts = true),
        allow: ControlState = FREE,
        failed: Boolean = false,
    ) = AccountServerChange(LinkedAccount("acct-a", "ada@example.com", null), change, allow, failed)

    private fun shown(prompt: ServerChangePrompt, allowed: MutableList<ShownServerChange>) {
        prompt.open(ShownServerChange("acct-a", question()))
        compose.setContent {
            AppTheme { ServerChangeDialog(prompt) { allowed += it } }
        }
        compose.waitForIdle()
    }

    @Test
    fun not_now_closes_the_question_without_allowing() {
        val prompt = ServerChangePrompt()
        val allowed = mutableListOf<ShownServerChange>()
        shown(prompt, allowed)

        compose.onNodeWithText(L10n.server_changed_title_both(ctx())).assertIsDisplayed()
        compose.onNodeWithText(L10n.server_changed_not_now(ctx())).performClick()
        compose.waitForIdle()

        assertNull(prompt.shown)
        assertTrue(allowed.isEmpty())
        compose.onNodeWithTag("server-change-dialog").assertDoesNotExist()
    }

    @Test
    fun allow_sends_the_change_shown_and_the_question_holds_while_it_runs() {
        val prompt = ServerChangePrompt()
        val allowed = mutableListOf<ShownServerChange>()
        shown(prompt, allowed)

        compose.onNodeWithTag("server-change-allow").performClick()
        assertEquals(change(calendar = true, contacts = true), allowed.single().question.change)

        prompt.redraw(question(allow = ControlState(enabled = false, running = true)))
        compose.waitForIdle()
        compose.onNodeWithTag("server-change-allow").assertIsNotEnabled()
        compose.onNodeWithTag("server-change-allow-running").assertExists()
        compose.onNodeWithTag("server-change-not-now").assertIsNotEnabled()
        prompt.close()
        assertNotNull("Not now cannot close it while Allow connects", prompt.shown)
    }

    @Test
    fun a_failed_allow_says_so_and_keeps_the_question_open() {
        val prompt = ServerChangePrompt()
        shown(prompt, mutableListOf())

        prompt.redraw(question(failed = true))
        compose.waitForIdle()

        compose.onNodeWithText(L10n.server_changed_failed(ctx())).assertIsDisplayed()
        compose.onNodeWithTag("server-change-allow").assertIsEnabled()
    }

    @Test
    fun a_question_the_account_no_longer_waits_on_closes() {
        val prompt = ServerChangePrompt()
        prompt.open(ShownServerChange("acct-a", question()))
        prompt.redraw(null)
        assertNull(prompt.shown)
    }

    @Test
    fun a_question_replaced_while_shown_asks_the_new_one() {
        val prompt = ServerChangePrompt()
        shown(prompt, mutableListOf())
        val replaced = ServerChange("icloud.com", "example.org", calendar = true, contacts = true)

        prompt.redraw(question(change = replaced))
        compose.waitForIdle()

        assertEquals(replaced, prompt.shown?.question?.change)
        compose.onNodeWithText(L10n.server_changed_now(ctx(), "example.org")).assertExists()
        compose.onNodeWithText(L10n.server_changed_failed(ctx())).assertDoesNotExist()
    }

    @Test
    fun the_account_page_says_the_use_is_paused_and_its_review_opens_the_question() {
        val base = accountEntry(
            CoreAccountKind.IMAP,
            AccountCapability.MAIL to CapabilityState.ON,
            AccountCapability.CALENDAR to CapabilityState.ON,
            AccountCapability.CONTACTS to CapabilityState.ON,
        )
        val entry = base.copy(
            uses = base.uses.map {
                if (it.capability == AccountCapability.CALENDAR) it.copy(problem = UseProblem.SERVER_CHANGED) else it
            },
        )
        val state = AccountsUiState()
        state.snapshot = AccountsSnapshot(listOf(entry))
        state.page = entry.id
        val paused = AccountUseStatus.Problem(UseProblem.SERVER_CHANGED)
        state.accountPage = pageOf(
            entry,
            uses = listOf(
                useSwitch(AccountCapability.MAIL, on = true),
                useSwitch(AccountCapability.CALENDAR, on = true, status = paused, reviews = true),
                useSwitch(AccountCapability.CONTACTS, on = true),
            ),
        )
        val reviewed = mutableListOf<String>()
        val ui = AccountsSettingsUi(state = state, onReviewServerChange = { reviewed += it })
        compose.setContent {
            Column(modifier = Modifier.verticalScroll(rememberScrollState())) {
                AccountsSettings(ui, allodiaSyncCard = {}) { shown, record ->
                    AccountPage(shown, record, ui)
                }
            }
        }
        compose.waitForIdle()

        compose.onNodeWithText(L10n.settings_account_use_server_changed_calendar(ctx())).assertIsDisplayed()
        compose.onNodeWithTag("account-use-review-CONTACTS").assertDoesNotExist()
        compose.onNodeWithTag("account-use-review-CALENDAR").performClick()

        assertEquals(listOf(entry.id), reviewed)
    }
}
