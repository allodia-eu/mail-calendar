// The editor's calendar picker draws the core's `new_event_calendars`: every calendar it offers, an
// account heading exactly when the core names the accounts, and a pick carrying its account.
package eu.allodia.mailcal

import androidx.compose.ui.test.junit4.v2.createComposeRule
import androidx.compose.ui.test.onNodeWithText
import androidx.compose.ui.test.performClick
import org.junit.Assert.assertEquals
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner

@RunWith(RobolectricTestRunner::class)
class CalendarPickerTest {
    @get:Rule val compose = createComposeRule()

    private fun sheet(named: Boolean, onPick: (CalendarChoice) -> Unit = {}) {
        compose.setContent {
            AppTheme {
                CalendarPickerSheet(
                    offered = offeredOf(TWO_ACCOUNTS, named = named),
                    selected = null,
                    onPick = onPick,
                    onDismiss = {},
                )
            }
        }
        compose.waitForIdle()
    }

    @Test
    fun named_accounts_head_their_calendars() {
        sheet(named = true)
        compose.onNodeWithText("a@example.org").assertExists()
        compose.onNodeWithText("b@example.org").assertExists()
        compose.onNodeWithText("Todoist").assertExists()
    }

    @Test
    fun accounts_the_core_does_not_name_get_no_heading() {
        sheet(named = false)
        compose.onNodeWithText("Work").assertExists()
        compose.onNodeWithText("Agenda").assertExists()
        compose.onNodeWithText("a@example.org").assertDoesNotExist()
        compose.onNodeWithText("b@example.org").assertDoesNotExist()
    }

    @Test
    fun a_pick_carries_its_account() {
        var picked: CalendarChoice? = null
        sheet(named = true) { picked = it }
        compose.onNodeWithText("Agenda").performClick()
        assertEquals("b@graph.example.com" to "agenda", picked?.let { it.account to it.id })
    }
}
