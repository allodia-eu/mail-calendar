// The default-calendar picker: which calendars it offers, and which one it shows as chosen.
//
// What is offered, which row is the default and whether accounts are named are the core's
// (`new_event_calendars`) and tested there. What is tested here is the half that lives on this side:
// the row the core marked reads as selected, a pick carries its account, and an account heading is
// drawn exactly when the core names the accounts.
package eu.allodia.mailcal

import androidx.compose.ui.test.assert
import androidx.compose.ui.test.assertIsDisplayed
import androidx.compose.ui.test.hasText
import androidx.compose.ui.test.isSelected
import androidx.compose.ui.test.onParent
import androidx.compose.ui.test.junit4.v2.createComposeRule
import androidx.compose.ui.test.onNodeWithText
import androidx.compose.ui.test.performClick
import org.junit.Assert.assertEquals
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.RuntimeEnvironment
import uniffi.mailcal_bindings.CalendarColor
import uniffi.mailcal_bindings.CalendarRow
import uniffi.mailcal_bindings.NewEventCalendars
import uniffi.mailcal_bindings.Swatch

private fun swatch() = Swatch(background = "#2f6fa8", text = "#ffffff", border = "#245782")

internal fun writableCalendar(account: String, id: String, name: String, isDefault: Boolean = false) = CalendarRow(
    account = account,
    // The address a person reads, distinct from the id the rows are grouped by.
    accountAddress = "${account.substringBefore('@')}@example.org",
    id = id,
    name = name,
    color = CalendarColor(hex = "#2f6fa8", light = swatch(), dark = swatch()),
    visible = true,
    canWrite = true,
    canInvite = false,
    isDefault = isDefault,
)

/** Two accounts' calendars, `a`'s Work the default. */
internal val TWO_ACCOUNTS = listOf(
    writableCalendar("a@imap.example.com", "work", "Work", isDefault = true),
    writableCalendar("b@graph.example.com", "agenda", "Agenda"),
    writableCalendar("b@graph.example.com", "todoist", "Todoist"),
)

@RunWith(RobolectricTestRunner::class)
class SettingsDefaultCalendarTest {
    @get:Rule val compose = createComposeRule()

    private fun ctx() = RuntimeEnvironment.getApplication()

    @Test
    fun the_row_the_core_marked_is_the_one_that_reads_as_chosen() {
        compose.setContent {
            DefaultCalendarSettingsRows(
                offered = offeredOf(
                    listOf(
                        writableCalendar("a", "work", "Work"),
                        writableCalendar("a", "private", "Private", isDefault = true),
                    ),
                    named = false,
                ),
                onSetDefaultCalendar = { _, _ -> },
            )
        }
        // The radio carries the selection, not the label beside it, so assert from the selected
        // node outwards. `onNode` also fails if more than one row is selected, which is the
        // property the whole design rests on.
        compose.onNode(isSelected()).onParent().assert(hasText("Private"))
    }

    @Test
    fun picking_one_reports_its_account_as_well_as_its_id() {
        // A calendar id is unique only within its account, so the account has to travel with the
        // pick or the core cannot tell two `work` calendars apart.
        var picked: Pair<String?, String?>? = null
        compose.setContent {
            DefaultCalendarSettingsRows(
                offered = offeredOf(TWO_ACCOUNTS, named = true),
                onSetDefaultCalendar = { account, calendar -> picked = account to calendar },
            )
        }
        compose.onNodeWithText("Todoist").performClick()
        assertEquals("b@graph.example.com" to "todoist", picked)
    }

    @Test
    fun named_accounts_get_a_heading_each_by_their_address() {
        compose.setContent {
            DefaultCalendarSettingsRows(
                offered = offeredOf(TWO_ACCOUNTS, named = true),
                onSetDefaultCalendar = { _, _ -> },
            )
        }
        compose.onNodeWithText("a@example.org").assertIsDisplayed()
        compose.onNodeWithText("b@example.org").assertIsDisplayed()
        compose.onNodeWithText("b@graph.example.com").assertDoesNotExist()
        compose.onNodeWithText("Agenda").assertIsDisplayed()
        compose.onNodeWithText("Todoist").assertIsDisplayed()
    }

    @Test
    fun accounts_the_core_does_not_name_get_no_heading() {
        // Nothing is counted on this side: groups the core leaves unnamed are drawn without one.
        compose.setContent {
            DefaultCalendarSettingsRows(
                offered = offeredOf(TWO_ACCOUNTS, named = false),
                onSetDefaultCalendar = { _, _ -> },
            )
        }
        compose.onNodeWithText("Work").assertIsDisplayed()
        compose.onNodeWithText("Agenda").assertIsDisplayed()
        compose.onNodeWithText("a@example.org").assertDoesNotExist()
        compose.onNodeWithText("b@example.org").assertDoesNotExist()
    }

    @Test
    fun nowhere_to_put_a_new_event_says_so_instead_of_offering_an_empty_list() {
        compose.setContent {
            DefaultCalendarSettingsRows(
                offered = NewEventCalendars(groups = emptyList(), named = false),
                onSetDefaultCalendar = { _, _ -> },
            )
        }
        compose.onNodeWithText(L10n.settings_default_calendar_none(ctx())).assertIsDisplayed()
    }
}
