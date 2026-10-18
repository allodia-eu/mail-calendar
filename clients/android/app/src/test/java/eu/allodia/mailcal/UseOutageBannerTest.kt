// The calendar and contacts screens draw one line per use that did not connect, above the week or
// the list, and its one action opens that account's page in Settings (docs/accounts.md rule 16).
package eu.allodia.mailcal

import androidx.compose.ui.test.assertCountEquals
import androidx.compose.ui.test.assertIsDisplayed
import androidx.compose.ui.test.junit4.v2.createComposeRule
import androidx.compose.ui.test.onAllNodesWithText
import androidx.compose.ui.test.onNodeWithText
import androidx.compose.ui.test.performClick
import java.time.DayOfWeek
import org.junit.Assert.assertEquals
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import uniffi.mailcal_bindings.CalendarWriteStatus

private const val ACCOUNT_SETTINGS = "Account settings"

@RunWith(RobolectricTestRunner::class)
class UseOutageBannerTest {
    @get:Rule val compose = createComposeRule()

    private val opened = mutableListOf<String>()

    private val ada = OutageLine("acct-a", "The calendar for ada@example.com isn’t connected.")
    private val bob = OutageLine("acct-b", "The calendar for bob@example.com isn’t connected.")

    private fun calendar(outages: List<OutageLine>) {
        compose.setContent {
            AppTheme {
                CalendarScreen(
                    pageFor = { _, _ -> GRID_NEIGHBOUR },
                    monthFor = { GRID_EMPTY_MONTH },
                    weekStartFor = { it.with(DayOfWeek.MONDAY) },
                    display = GRID_DISPLAY,
                    calendarVersion = 0,
                    events = emptyList(),
                    writeStatus = CalendarWriteStatus.IDLE,
                    activeZoneId = "Europe/Amsterdam",
                    palette = emptyList(),
                    onRefreshCalendar = {},
                    onDeleteEvent = { _, _, _ -> },
                    onCreateEvent = {},
                    onUpdateEvent = {},
                    onMoveEvent = {},
                    eventDetailFor = { _, _, _ -> null },
                    seriesWarningFor = { null },
                    deviceZoneId = "Europe/Amsterdam",
                    onSetVisibleHours = {},
                    onSetLayout = {},
                    onSetCalendarVisible = { _, _, _ -> },
                    onSetCalendarColor = { _, _, _ -> },
                    outages = outages,
                    onOpenAccount = { opened += it },
                    clock = { GRID_NOW },
                )
            }
        }
        compose.waitForIdle()
    }

    private fun contacts(outages: List<OutageLine>) {
        compose.setContent {
            AppTheme {
                ContactsScreen(
                    rows = emptyList(),
                    onSearch = {},
                    detailFor = { null },
                    outages = outages,
                    onOpenAccount = { opened += it },
                )
            }
        }
        compose.waitForIdle()
    }

    @Test
    fun the_calendar_draws_a_line_per_account_whose_action_opens_its_page() {
        calendar(listOf(ada, bob))

        compose.onNodeWithText(ada.text).assertIsDisplayed()
        compose.onNodeWithText(bob.text).assertIsDisplayed()
        val actions = compose.onAllNodesWithText(ACCOUNT_SETTINGS)
        actions.assertCountEquals(2)
        actions[1].performClick()

        assertEquals(listOf("acct-b"), opened)
    }

    @Test
    fun the_calendar_draws_no_line_without_an_outage() {
        calendar(emptyList())

        compose.onNodeWithText(ACCOUNT_SETTINGS).assertDoesNotExist()
    }

    @Test
    fun the_contacts_list_draws_the_line_and_its_action_opens_the_account() {
        val line = OutageLine("acct-a", "The address book for ada@example.com isn’t connected.")
        contacts(listOf(line))

        compose.onNodeWithText(line.text).assertIsDisplayed()
        compose.onNodeWithText(ACCOUNT_SETTINGS).performClick()

        assertEquals(listOf("acct-a"), opened)
    }

    @Test
    fun the_contacts_list_draws_no_line_without_an_outage() {
        contacts(emptyList())

        compose.onNodeWithText(ACCOUNT_SETTINGS).assertDoesNotExist()
    }
}
