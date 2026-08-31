// The default-calendar picker: which calendars it offers, and which one it shows as chosen.
//
// The *rule*, the stored choice while it exists and can be written to, else the first writable
// calendar, is the core's, and is tested there (`calendar_colors::default_calendar_tests`). What is
// tested here is the half that lives on this side: a read-only calendar is never offered, the row
// the core marked is the one that reads as selected, and a name that appears in two accounts is
// qualified so the list does not offer the same word twice.
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
import uniffi.mailcal_bindings.Swatch

private fun swatch() = Swatch(background = "#2f6fa8", text = "#ffffff", border = "#245782")

private fun calendar(
    account: String,
    id: String,
    name: String,
    canWrite: Boolean = true,
    isDefault: Boolean = false,
) = CalendarRow(
    account = account,
    id = id,
    name = name,
    color = CalendarColor(hex = "#2f6fa8", light = swatch(), dark = swatch()),
    visible = true,
    canWrite = canWrite,
    isDefault = isDefault,
)

@RunWith(RobolectricTestRunner::class)
class SettingsDefaultCalendarTest {
    @get:Rule val compose = createComposeRule()

    private fun ctx() = RuntimeEnvironment.getApplication()

    @Test
    fun a_read_only_calendar_is_never_offered() {
        // Choosing one would produce a default that fails at save time, with the event already
        // typed, the core refuses it too, and this is the affordance not existing in the first
        // place rather than a choice that silently does nothing.
        compose.setContent {
            DefaultCalendarSettingsRows(
                calendars = listOf(
                    calendar("a", "work", "Work", isDefault = true),
                    calendar("a", "holidays", "Holidays", canWrite = false),
                ),
                onSetDefaultCalendar = { _, _ -> },
            )
        }
        compose.onNodeWithText("Work").assertIsDisplayed()
        compose.onNodeWithText("Holidays").assertDoesNotExist()
    }

    @Test
    fun the_row_the_core_marked_is_the_one_that_reads_as_chosen() {
        compose.setContent {
            DefaultCalendarSettingsRows(
                calendars = listOf(
                    calendar("a", "work", "Work"),
                    calendar("a", "private", "Private", isDefault = true),
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
        // pick or the core cannot tell two `work` calendars apart. The row shows only the name now,
        // so the two rows below are told apart by the account heading above each.
        var picked: Pair<String?, String?>? = null
        compose.setContent {
            DefaultCalendarSettingsRows(
                calendars = listOf(
                    calendar("a@imap.example.com", "work", "Work", isDefault = true),
                    calendar("b@graph.example.com", "private", "Private"),
                ),
                onSetDefaultCalendar = { account, calendar -> picked = account to calendar },
            )
        }
        compose.onNodeWithText("Private").performClick()
        assertEquals("b@graph.example.com" to "private", picked)
    }

    @Test
    fun more_than_one_account_gets_a_heading_each_instead_of_a_suffix_per_row() {
        // The nit this fixes: an account id is `address@provider-host`, and repeating it beside
        // every calendar wrapped each row over three lines. It belongs above the group, once.
        compose.setContent {
            DefaultCalendarSettingsRows(
                calendars = listOf(
                    calendar("a@imap.example.com", "work", "Work", isDefault = true),
                    calendar("b@graph.example.com", "agenda", "Agenda"),
                    calendar("b@graph.example.com", "todoist", "Todoist"),
                ),
                onSetDefaultCalendar = { _, _ -> },
            )
        }
        // One heading per account, and the rows carry the bare calendar name.
        compose.onNodeWithText("a@imap.example.com").assertIsDisplayed()
        compose.onNodeWithText("b@graph.example.com").assertIsDisplayed()
        compose.onNodeWithText("Agenda").assertIsDisplayed()
        compose.onNodeWithText("Todoist").assertIsDisplayed()
        compose.onNodeWithText("Agenda, b@graph.example.com").assertDoesNotExist()
    }

    @Test
    fun one_account_states_itself_and_needs_no_heading() {
        // With a single account the rows are its calendars and there is nothing to tell apart, so
        // the heading would be a line of noise above the only group.
        compose.setContent {
            DefaultCalendarSettingsRows(
                calendars = listOf(calendar("a@imap.example.com", "work", "Work", isDefault = true)),
                onSetDefaultCalendar = { _, _ -> },
            )
        }
        compose.onNodeWithText("Work").assertIsDisplayed()
        compose.onNodeWithText("a@imap.example.com").assertDoesNotExist()
    }

    @Test
    fun no_writable_calendar_says_so_instead_of_offering_an_empty_list() {
        compose.setContent {
            DefaultCalendarSettingsRows(
                calendars = listOf(calendar("a", "holidays", "Holidays", canWrite = false)),
                onSetDefaultCalendar = { _, _ -> },
            )
        }
        compose.onNodeWithText(L10n.settings_default_calendar_none(ctx())).assertIsDisplayed()
    }
}
