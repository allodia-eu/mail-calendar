// The calendar's own back levels, the manager, an open event, and the event editor.
//
// These are the reason this pass existed. Each is an opaque `Surface` composed OVER the grid, not a
// window of its own, so unlike a `Dialog` or a bottom sheet NOTHING hands them the system back
// press: without a handler it fell straight through to the activity and closed the app from three
// levels deep. They unwind topmost-first, and once the grid is bare the calendar goes quiet so the
// enclosing AppNavScaffold can take the press home (BackNavigationTest covers that half).
package eu.allodia.mailcal

import androidx.activity.compose.LocalOnBackPressedDispatcherOwner
import androidx.compose.material3.Text
import androidx.compose.ui.test.junit4.v2.createComposeRule
import androidx.compose.ui.test.onNodeWithContentDescription
import androidx.compose.ui.test.onNodeWithText
import androidx.compose.ui.test.performClick
import java.time.DayOfWeek
import java.time.LocalDateTime
import org.junit.Assert.assertEquals
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.RuntimeEnvironment
import uniffi.mailcal_bindings.Appearance
import uniffi.mailcal_bindings.CalendarColor
import uniffi.mailcal_bindings.CalendarLayout
import uniffi.mailcal_bindings.CalendarPage
import uniffi.mailcal_bindings.CalendarRow
import uniffi.mailcal_bindings.CalendarWriteStatus
import uniffi.mailcal_bindings.DisplaySettings
import uniffi.mailcal_bindings.EventDetail
import uniffi.mailcal_bindings.EventRow
import uniffi.mailcal_bindings.GridDay
import uniffi.mailcal_bindings.MonthCell
import uniffi.mailcal_bindings.MonthPage
import uniffi.mailcal_bindings.ResponseStatus
import uniffi.mailcal_bindings.Swatch
import uniffi.mailcal_bindings.TimeFormat
import uniffi.mailcal_bindings.WeekStart

// Pinned "now", as in the other calendar suites, the header must not race the wall clock.
private val NOW: LocalDateTime = LocalDateTime.of(2026, 7, 12, 9, 45)

private fun calendarRow(id: String) = CalendarRow(
    account = "acct-1",
    id = id,
    name = id,
    color = CalendarColor(
        hex = "#2f6fa8",
        light = Swatch("#2f6fa8", "#ffffff", "#245782"),
        dark = Swatch("#23537e", "#ffffff", "#2f6fa8"),
    ),
    visible = true,
    canWrite = true,
    isDefault = true,
)

private fun eventDetail() = EventDetail(
    account = "acct-1",
    key = "evt-1",
    calendar = "work",
    title = "Board meeting",
    allDay = false,
    timezone = "Europe/Amsterdam",
    start = "2026-07-16T09:00:00",
    end = "2026-07-16T10:00:00",
    location = null,
    notes = null,
    reminderMinutes = null,
    recurrence = null,
    repeatSummary = null,
    repeatDraft = null,
    isRecurring = false,
    canWrite = true,
    occurrenceStart = "",
    attendees = emptyList(),
)

@RunWith(RobolectricTestRunner::class)
class CalendarBackNavigationTest {
    @get:Rule val compose = createComposeRule()

    private fun ctx() = RuntimeEnvironment.getApplication()

    private lateinit var back: () -> Unit

    private fun pressBack() {
        compose.runOnUiThread { back() }
        compose.waitForIdle()
    }

    /** Captures the activity dispatcher's back hook; call from inside `setContent`. */
    @androidx.compose.runtime.Composable
    private fun captureBack() {
        val dispatcher = LocalOnBackPressedDispatcherOwner.current!!.onBackPressedDispatcher
        back = { dispatcher.onBackPressed() }
    }

    /** Renders the calendar inside the nav host, exactly as MainActivity composes it. */
    private fun calendar(layout: CalendarLayout = CalendarLayout.WEEK, events: List<EventRow> = emptyList()) {
        val calendars = listOf(calendarRow("work"))
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
                captureBack()
                AppNavScaffold(
                    destination = AppDestination.CALENDAR,
                    home = AppDestination.MAIL,
                    onSelect = {},
                ) {
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
                        events = events,
                        writeStatus = CalendarWriteStatus.IDLE,
                        activeZoneId = "Europe/Amsterdam",
                        palette = emptyList(),
                        onRefreshCalendar = {},
                        onDeleteEvent = { _, _, _ -> },
                        onCreateEvent = {},
                        onUpdateEvent = {},
                        onMoveEvent = {},
                        eventDetailFor = { _, _, _ -> eventDetail() },
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
        compose.waitForIdle()
    }

    @Test
    fun back_closes_the_calendar_manager_before_it_leaves_the_calendar() {
        calendar()
        compose.onNodeWithContentDescription(L10n.calendar_view_label(ctx())).performClick()
        compose.onNodeWithText(L10n.calendar_manage(ctx())).performClick()
        // The manager is up: its title is the only "Manage calendars" left once the menu closed.
        compose.onNodeWithText(L10n.calendar_manage(ctx())).assertExists()
        compose.onNodeWithText(L10n.action_done(ctx())).assertExists()

        pressBack()

        // The manager is gone and the grid is back, one level, not out of the app.
        compose.onNodeWithText(L10n.action_done(ctx())).assertDoesNotExist()
        compose.onNodeWithContentDescription(L10n.calendar_view_label(ctx())).assertExists()
    }

    @Test
    fun back_closes_an_open_event_before_it_leaves_the_calendar() {
        calendar(
            layout = CalendarLayout.AGENDA,
            events = listOf(
                EventRow(
                    account = "acct-1",
                    key = "evt-1",
                    title = "Board meeting",
                    start = "2026-07-16T09:00:00Z",
                    canWrite = true,
                    participation = ResponseStatus.ACCEPTED,
                ),
            ),
        )
        compose.onNodeWithText("Board meeting").performClick()
        // The detail is open: it is the only screen here with a back arrow.
        compose.onNodeWithContentDescription(L10n.action_close(ctx())).assertExists()

        pressBack()

        compose.onNodeWithContentDescription(L10n.action_close(ctx())).assertDoesNotExist()
    }

    @Test
    fun back_closes_the_event_editor_before_it_leaves_the_calendar() {
        calendar()
        compose.onNodeWithContentDescription(L10n.action_new_event(ctx())).performClick()
        compose.onNodeWithText(L10n.event_new_title(ctx())).assertExists()

        pressBack()

        compose.onNodeWithText(L10n.event_new_title(ctx())).assertDoesNotExist()
        compose.onNodeWithContentDescription(L10n.calendar_view_label(ctx())).assertExists()
    }

    /** With nothing open the calendar defers: the scaffold above it takes the press home. */
    @Test
    fun back_on_the_bare_grid_falls_through_to_the_tab_rule() {
        val selected = mutableListOf<AppDestination>()
        compose.setContent {
            AppTheme {
                captureBack()
                AppNavScaffold(
                    destination = AppDestination.CALENDAR,
                    home = AppDestination.MAIL,
                    onSelect = { selected += it },
                ) { Text("grid") }
            }
        }
        compose.waitForIdle()

        pressBack()

        assertEquals(listOf(AppDestination.MAIL), selected)
    }
}
