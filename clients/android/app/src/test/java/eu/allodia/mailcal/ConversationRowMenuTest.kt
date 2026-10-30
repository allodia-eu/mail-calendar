package eu.allodia.mailcal

import androidx.compose.runtime.getValue
import androidx.compose.runtime.setValue
import androidx.compose.ui.test.assertIsEnabled
import androidx.compose.ui.test.assertIsNotEnabled
import androidx.compose.ui.test.junit4.v2.createComposeRule
import androidx.compose.ui.test.assertCountEquals
import androidx.compose.ui.test.onAllNodesWithContentDescription
import androidx.compose.ui.test.onNodeWithTag
import androidx.compose.ui.test.onNodeWithText
import androidx.compose.ui.test.performClick
import org.junit.Assert.assertEquals
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.RuntimeEnvironment
import uniffi.mailcal_bindings.BulkAction
import uniffi.mailcal_bindings.MenuMessage
import uniffi.mailcal_bindings.MessageKind
import uniffi.mailcal_bindings.RowAction
import uniffi.mailcal_bindings.RowMenu
import uniffi.mailcal_bindings.RowMenuItem
import uniffi.mailcal_bindings.RowMenuSection
import uniffi.mailcal_bindings.SelectedRow
import uniffi.mailcal_bindings.ThreadRow

// A conversation row carries the ⋮ menu the core answers, asked as it opens, and draws the flag
// while anything on it is flagged (docs/row-actions.md, rules 1, 3 and 6).
@RunWith(RobolectricTestRunner::class)
class ConversationRowMenuTest {
    @get:Rule val compose = createComposeRule()

    private val ctx get() = RuntimeEnvironment.getApplication()
    private val row = SelectedRow.Thread("acct-1", "thread-1")
    private val draft = MenuMessage(account = "acct-1", key = "draft", kind = MessageKind.DRAFT)

    private val menu = RowMenu(
        sections = listOf(
            RowMenuSection(items = listOf(RowMenuItem(action = RowAction.OPEN, enabled = true, target = draft))),
            RowMenuSection(
                items = listOf(
                    RowMenuItem(action = RowAction.REPLY, enabled = false, target = draft),
                    RowMenuItem(action = RowAction.FORWARD, enabled = false, target = draft),
                ),
            ),
            RowMenuSection(items = listOf(RowMenuItem(action = RowAction.UNFLAG, enabled = true, target = null))),
        ),
    )

    @Test
    fun the_menu_draws_the_cores_answer_and_runs_what_is_picked() {
        val asked = mutableListOf<List<SelectedRow>>()
        val picked = mutableListOf<RowPick>()
        val menus = RowMenus(
            menu = { rows -> asked += rows; menu },
            rowsFor = { listOf(it) },
            filing = null,
            onPick = { picked += it },
        )
        compose.setContent { AppTheme { conversationRow(flagged = true, menus = menus) } }
        assertEquals("nothing is asked before the menu opens", emptyList<List<SelectedRow>>(), asked)

        compose.onNodeWithTag("row-menu", useUnmergedTree = true).performClick()
        assertEquals(listOf(listOf<SelectedRow>(row)), asked)
        // Disabled, but still there: the menu keeps its shape from one row to the next.
        compose.onNodeWithText(L10n.action_reply(ctx)).assertIsNotEnabled()
        compose.onNodeWithText(L10n.action_forward(ctx)).assertIsNotEnabled()
        compose.onNodeWithText(L10n.action_unflag(ctx)).assertIsEnabled().performClick()
        assertEquals(listOf<RowPick>(RowPick.Rows(listOf(row), BulkAction.UNFLAG)), picked)

        compose.onNodeWithTag("row-menu", useUnmergedTree = true).performClick()
        compose.onNodeWithText(L10n.action_open(ctx)).performClick()
        assertEquals(RowPick.Open(draft), picked.last())
    }

    @Test
    fun a_conversation_draws_its_flag_only_while_something_on_it_is_flagged() {
        var flagged by mutableFlag(true)
        compose.setContent { AppTheme { conversationRow(flagged = flagged, menus = RowMenus.NONE) } }
        compose.onAllNodesWithContentDescription(L10n.a11y_flagged(ctx), useUnmergedTree = true)
            .assertCountEquals(1)

        compose.runOnIdle { flagged = false }
        compose.onAllNodesWithContentDescription(L10n.a11y_flagged(ctx), useUnmergedTree = true)
            .assertCountEquals(0)
    }

    @androidx.compose.runtime.Composable
    private fun conversationRow(flagged: Boolean, menus: RowMenus) {
        ThreadConversationRow(
            thread = thread(flagged = flagged),
            activeZoneId = "Europe/Amsterdam",
            selected = false,
            selecting = false,
            onToggleSelect = {},
            onOpenThread = {},
            menus = menus,
        )
    }

    private fun mutableFlag(initial: Boolean) = androidx.compose.runtime.mutableStateOf(initial)

    private fun thread(flagged: Boolean) = ThreadRow(
        account = "acct-1",
        threadId = "thread-1",
        latestKey = "received",
        subject = "Contribution for next year",
        latestFrom = "Ada Lovelace",
        avatar = stubAvatar("AL"),
        latestDate = "2026-07-10T11:34:41Z",
        messageCount = 2u,
        unreadCount = 0u,
        flagged = flagged,
        hasAttachment = false,
        preview = "",
        messages = emptyList(),
    )
}
