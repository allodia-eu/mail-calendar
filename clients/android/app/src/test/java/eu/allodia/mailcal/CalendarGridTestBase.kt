// The shared fixtures and rendering harness for the calendar-grid tests, split out of
// CalendarGridTest.kt: driving the grid the way a client drives it, with synthetic pages instead
// of the Rust core, and listening to it the way TalkBack does (the grid is a canvas, so its
// semantics overlay is the only honest way to ask "what would a user who cannot see this be
// told?"). CalendarGridTest.kt, CalendarGridGeometryTest.kt, CalendarGridAllDayTest.kt and
// CalendarGridInvitationTest.kt each extend [CalendarGridTestBase] for one group of tests.
package eu.allodia.mailcal

import android.view.accessibility.AccessibilityManager
import androidx.compose.ui.test.getUnclippedBoundsInRoot
import androidx.compose.ui.test.hasContentDescription
import androidx.compose.ui.test.junit4.v2.createComposeRule
import java.time.DayOfWeek
import java.time.LocalDate
import java.time.LocalDateTime
import org.junit.Before
import org.junit.Rule
import org.robolectric.RuntimeEnvironment
import org.robolectric.Shadows.shadowOf
import uniffi.mailcal_bindings.AllDayBand
import uniffi.mailcal_bindings.Appearance
import uniffi.mailcal_bindings.CalendarColor
import uniffi.mailcal_bindings.CalendarLayout
import uniffi.mailcal_bindings.CalendarPage
import uniffi.mailcal_bindings.CalendarRow
import uniffi.mailcal_bindings.CalendarWriteStatus
import uniffi.mailcal_bindings.DisplaySettings
import uniffi.mailcal_bindings.GridDay
import uniffi.mailcal_bindings.MonthCell
import uniffi.mailcal_bindings.MonthPage
import uniffi.mailcal_bindings.ResponseStatus
import uniffi.mailcal_bindings.Swatch
import uniffi.mailcal_bindings.TimeFormat
import uniffi.mailcal_bindings.TimedSegment
import uniffi.mailcal_bindings.WeekStart

// A Sunday, 09:45, "now" is pinned so the now line and the "back to today" glyph are deterministic
// rather than racing the wall clock.
internal val GRID_NOW: LocalDateTime = LocalDateTime.of(2026, 7, 12, 9, 45)
internal val GRID_TODAY: LocalDate = GRID_NOW.toLocalDate()

// The core's defaults: Monday-start, 24-hour, a 12-hour horizon, opening on the whole week.
internal val GRID_DISPLAY =
    DisplaySettings(
        WeekStart.MONDAY,
        TimeFormat.TWENTY_FOUR_HOUR,
        Appearance.SYSTEM,
        12u,
        CalendarLayout.WEEK,
    )

internal val GRID_WORK = CalendarRow(
    account = "acct-1",
    id = "work",
    name = "Work",
    color = CalendarColor(
        hex = "#2f6fa8",
        light = Swatch("#2f6fa8", "#ffffff", "#245782"),
        dark = Swatch("#23537e", "#ffffff", "#2f6fa8"),
    ),
    visible = true,
    canWrite = true,
    isDefault = true,
)

/** An empty month, the grid tests never switch to it, but the screen needs a query. */
internal val GRID_EMPTY_MONTH = MonthPage(
    cells = (0 until 42).map { MonthCell("2026-07-01", inMonth = true, chips = emptyList()) },
    timezone = "Europe/Amsterdam",
    calendars = emptyList(),
    isMaterialized = true,
)

/** The week 2026-07-06 (Mon) … 2026-07-12 (Sun), today is the last column. */
internal fun gridWeekDays(): List<GridDay> =
    (6..12).map { GridDay("2026-07-%02d".format(it)) }

/** The Monday of the week under test. The pager anchors its middle gridPage here. */
internal val GRID_ANCHOR: LocalDate = LocalDate.of(2026, 7, 6)

/**
 * A neighbouring week: a different seven days, holding nothing.
 *
 * The pager composes the weeks either side of the one on screen, that is what keeps a swipe from
 * having to build sixty event blocks inside the fling. So the neighbours are real, composed, and
 * asserted against; handing all three pages the *same* fixture made every title exist three times
 * over and every assertion ambiguous. Giving them their own days is not a workaround, it is what the
 * app actually does: next week is not this week.
 */
internal val GRID_NEIGHBOUR = CalendarPage(
    days = (13..19).map { GridDay("2026-07-%02d".format(it)) },
    timed = emptyList(),
    allDay = emptyList(),
    allDayLanes = 0u,
    timezone = "Europe/Amsterdam",
    calendars = listOf(GRID_WORK),
    isMaterialized = true,
)

internal fun gridBlock(
    title: String = "Team standup",
    day: Int = 6,
    startMinutes: Int = 570, // 09:30
    endMinutes: Int = 630, // 10:30
    column: Int = 0,
    columns: Int = 1,
) = TimedSegment(
    account = "acct-1",
    event = "evt-$title",
    calendar = "work",
    title = title,
    day = day.toUInt(),
    startMinutes = startMinutes.toUInt(),
    endMinutes = endMinutes.toUInt(),
    column = column.toUInt(),
    columns = columns.toUInt(),
    continuesBefore = false,
    continuesAfter = false,
    canWrite = true,
    canMove = true,
    occurrenceStart = "",
    participation = ResponseStatus.ACCEPTED,
)

internal fun gridPage(
    days: List<GridDay> = gridWeekDays(),
    timed: List<TimedSegment> = listOf(gridBlock()),
    allDay: List<AllDayBand> = emptyList(),
    allDayLanes: UInt = 0u,
    isMaterialized: Boolean = true,
) = CalendarPage(
    days = days,
    timed = timed,
    allDay = allDay,
    allDayLanes = allDayLanes,
    timezone = "Europe/Amsterdam",
    calendars = listOf(GRID_WORK),
    isMaterialized = isMaterialized,
)

abstract class CalendarGridTestBase {
    @get:Rule val compose = createComposeRule()

    /**
     * Listen to the grid the way TalkBack does.
     *
     * The surface only materializes its semantics nodes when a screen reader is actually exploring by
     * touch, they cost layout, and a pinch would pay for them sixty times a second for a service that
     * is not running. Turning it on here is what makes the drawn grid visible to a test at all.
     */
    @Before
    fun listenLikeAScreenReader() {
        val manager = RuntimeEnvironment.getApplication()
            .getSystemService(AccessibilityManager::class.java)
        shadowOf(manager).setTouchExplorationEnabled(true)
    }

    /** The spoken label of the gridBlock, band or chip carrying [text]. */
    protected fun spoken(text: String) =
        compose.onNode(hasContentDescription(text, substring = true))

    protected fun boundsOf(text: String) = spoken(text).getUnclippedBoundsInRoot()

    /** Renders the calendar over a fixed gridPage, with "now" pinned. */
    protected fun screen(fixture: CalendarPage) {
        compose.setContent {
            AppTheme {
                CalendarScreen(
                    // The week under test in the middle; its neighbours are other weeks, as in the app.
                    pageFor = { from, _ -> if (from == GRID_ANCHOR) fixture else GRID_NEIGHBOUR },
                    monthFor = { GRID_EMPTY_MONTH },
                    // Monday-start, like the core's own `week_start_date`. The identity stub this
                    // replaces silently anchored the pager on *today* rather than on the Monday, so
                    // the middle page's anchor did not match the week the fixture describes.
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
                    clock = { GRID_NOW },
                )
            }
        }
    }
}
