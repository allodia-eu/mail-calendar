// Write-capability gating: what a read-only calendar is NOT offered.
//
// The core stamps `canWrite` on every rendered event record and on every `CalendarRow`; the
// client's only job is to withhold the affordances. Two rules, shared with the Apple and Windows
// clients: a per-event delete is HIDDEN when that event's own record says `canWrite = false`, and
// the header's new-event button is DISABLED (not hidden, the header keeps its shape) when no
// calendar on any account reports `canWrite = true`. An empty calendars list means disabled too:
// nothing has synced, so a new event has nowhere to go.
package eu.allodia.mailcal

import androidx.compose.ui.test.assertCountEquals
import androidx.compose.ui.test.assertIsEnabled
import androidx.compose.ui.test.assertIsNotEnabled
import androidx.compose.ui.test.junit4.v2.createComposeRule
import androidx.compose.ui.test.onAllNodesWithContentDescription
import androidx.compose.ui.test.onNodeWithContentDescription
import androidx.compose.ui.test.onNodeWithText
import androidx.compose.ui.test.performTouchInput
import androidx.compose.ui.test.swipeRight
import java.time.DayOfWeek
import java.time.LocalDateTime
import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import uniffi.mailcal_bindings.Appearance
import uniffi.mailcal_bindings.CalendarColor
import uniffi.mailcal_bindings.CalendarLayout
import uniffi.mailcal_bindings.CalendarPage
import uniffi.mailcal_bindings.CalendarRow
import uniffi.mailcal_bindings.CalendarWriteStatus
import uniffi.mailcal_bindings.DisplaySettings
import uniffi.mailcal_bindings.EventRow
import uniffi.mailcal_bindings.GridDay
import uniffi.mailcal_bindings.MonthCell
import uniffi.mailcal_bindings.MonthPage
import uniffi.mailcal_bindings.ResponseStatus
import uniffi.mailcal_bindings.Swatch
import uniffi.mailcal_bindings.TimeFormat
import uniffi.mailcal_bindings.WeekStart

// Pinned "now", as in CalendarGridTest, the header must not race the wall clock.
private val NOW: LocalDateTime = LocalDateTime.of(2026, 7, 12, 9, 45)

private fun calendarRow(id: String, canWrite: Boolean) = CalendarRow(
    account = "acct-1",
    id = id,
    name = id,
    color = CalendarColor(
        hex = "#2f6fa8",
        light = Swatch("#2f6fa8", "#ffffff", "#245782"),
        dark = Swatch("#23537e", "#ffffff", "#2f6fa8"),
    ),
    visible = true,
    canWrite = canWrite,
    isDefault = canWrite,
)

private fun agendaEvent(key: String, title: String, canWrite: Boolean) = EventRow(
    account = "acct-1",
    key = key,
    title = title,
    start = "2026-07-16T09:00:00Z",
    canWrite = canWrite,
    participation = ResponseStatus.ACCEPTED,
)

@RunWith(RobolectricTestRunner::class)
class CalendarWriteGatingTest {
    @get:Rule val compose = createComposeRule()

    /** Renders the calendar with every page (grid and month) listing [calendars]. */
    private fun screen(calendars: List<CalendarRow>, layout: CalendarLayout = CalendarLayout.WEEK) {
        val page = CalendarPage(
            days = (6..12).map { GridDay("2026-07-%02d".format(it)) },
            timed = emptyList(),
            allDay = emptyList(),
            allDayLanes = 0u,
            timezone = "Europe/Amsterdam",
            calendars = calendars,
            isMaterialized = true,
        )
        val month = MonthPage(
            cells = (0 until 42).map { MonthCell("2026-07-01", inMonth = true, chips = emptyList()) },
            timezone = "Europe/Amsterdam",
            calendars = calendars,
            isMaterialized = true,
        )
        compose.setContent {
            AppTheme {
                CalendarScreen(
                    pageFor = { _, _ -> page },
                    monthFor = { month },
                    weekStartFor = { it.with(DayOfWeek.MONDAY) },
                    display = DisplaySettings(
                        WeekStart.MONDAY,
                        TimeFormat.TWENTY_FOUR_HOUR,
                        Appearance.SYSTEM,
                        12u,
                        layout,
                    ),
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
                    clock = { NOW },
                )
            }
        }
    }

    private fun newEventButton() = compose.onNodeWithContentDescription("New Event")

    @Test
    fun new_event_is_disabled_when_no_calendar_can_write() {
        screen(calendars = listOf(calendarRow("subscribed", canWrite = false)))
        newEventButton().assertIsNotEnabled()
    }

    @Test
    fun new_event_is_enabled_when_any_calendar_can_write() {
        // One read-only calendar must not veto the writable one beside it.
        screen(
            calendars = listOf(
                calendarRow("subscribed", canWrite = false),
                calendarRow("work", canWrite = true),
            ),
        )
        newEventButton().assertIsEnabled()
    }

    @Test
    fun new_event_is_disabled_before_anything_has_synced() {
        // No calendars at all: a new event has nowhere to go yet.
        screen(calendars = emptyList())
        newEventButton().assertIsNotEnabled()
    }

    @Test
    fun the_agenda_answers_the_same_question_without_a_page_of_its_own() {
        // The agenda composes neither a grid page nor a month, it pulls one just for the
        // calendars list, so the gate cannot read "no page" as "no writable calendar".
        screen(calendars = listOf(calendarRow("work", canWrite = true)), layout = CalendarLayout.AGENDA)
        newEventButton().assertIsEnabled()
    }

    /** Renders the agenda list alone, recording every delete it dispatches. */
    private fun agenda(events: List<EventRow>, deleted: MutableList<Pair<String, String>>) {
        compose.setContent {
            AppTheme {
                AgendaList(
                    events = events,
                    activeZoneId = "Europe/Amsterdam",
                    use24Hour = true,
                    onDeleteEvent = { account, key -> deleted += account to key },
                    onOpenEvent = { },
                )
            }
        }
    }

    @Test
    fun a_read_only_event_offers_no_delete_at_all() {
        val deleted = mutableListOf<Pair<String, String>>()
        agenda(listOf(agendaEvent("evt-1", "Board meeting", canWrite = false)), deleted)
        // The trash affordance is not composed, hidden, not disabled.
        compose.onAllNodesWithContentDescription("Delete Event").assertCountEquals(0)
        // And the swipe that would reveal it dispatches nothing.
        compose.onNodeWithText("Board meeting").performTouchInput { swipeRight() }
        compose.waitForIdle()
        assertTrue("a swipe on a read-only event must not delete", deleted.isEmpty())
    }

    @Test
    fun a_writable_event_still_deletes_on_swipe() {
        val deleted = mutableListOf<Pair<String, String>>()
        agenda(listOf(agendaEvent("evt-2", "Team standup", canWrite = true)), deleted)
        compose.onNodeWithText("Team standup").performTouchInput { swipeRight() }
        compose.waitForIdle()
        assertEquals(listOf("acct-1" to "evt-2"), deleted)
    }
}
