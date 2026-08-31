// What the calendar is showing and where, the state a swipe, a view switch, and "back to today"
// all move.
//
// The Rust core never tracks where the user is: `calendarPage(view, anchor, weekStartsMonday)` is a
// pull with an argument, so the *client* owns the anchor. That makes this class the whole of the
// navigation model, and it is a plain class rather than a knot of `remember`s in the screen so the
// page<->date mapping is unit-testable without composing anything (cf. SwipeUndoController).
package eu.allodia.mailcal

import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.setValue
import uniffi.mailcal_bindings.CalendarLayout
import java.time.DayOfWeek
import java.time.LocalDate
import java.time.temporal.ChronoUnit
import kotlin.math.abs
import java.util.Locale
import java.time.temporal.WeekFields

// A HorizontalPager needs a finite page count, so the calendar is a very long strip of pages with
// "now" in the middle. 5,000 strides either way is ~13 years of day-pages and ~95 years of
// week-pages, far past the core's rolling horizon, which reports `isMaterialized = false` long
// before this runs out. Paging can't dead-end; it just stops having data, honestly.
internal const val CALENDAR_PAGE_COUNT = 10_001
internal const val CALENDAR_PAGE_ORIGIN = 5_000

/**
 * The shape the calendar is drawn in.
 *
 * The four grid shapes are **not four views**. They are four zoom levels of one grid: a page is
 * always a whole week, and the "view" is just how many of its seven columns are on screen. That is
 * what makes zooming smooth, the days never move, only their width changes.
 *
 * Snapping a zoom to a differently-anchored view is what made the old design jump: a Monday-aligned
 * week cannot contain an arbitrary three-day window, so a user reading Sunday-to-Tuesday who pinched
 * outwards was shown the *previous* Monday-to-Sunday, and two of their three days vanished.
 */
internal enum class CalendarMode {
    DAY,
    THREE_DAY,
    WORK_WEEK,
    WEEK,
    MONTH,
    AGENDA,
    ;

    /** Whether this is the month grid, a different layout, with no hour axis and no zoom. */
    val isMonth: Boolean get() = this == MONTH

    /** Whether this is the time grid, at any zoom. */
    val isGrid: Boolean get() = this in GRID_MODES

    /** How many of the week's seven columns this zoom level shows. */
    val columns: Int
        get() = when (this) {
            DAY -> 1
            THREE_DAY -> 3
            WORK_WEEK -> 5
            WEEK -> 7
            MONTH, AGENDA -> 0
        }

    /**
     * The columns to seed the day-axis zoom with.
     *
     * The month and the agenda have no columns of their own ([columns] is 0, and dividing the
     * viewport by it would be a crash or an infinity). But the grid is still *there*, one menu tap
     * away, so it is seeded with the whole week, which is what it should show when the user comes
     * back to it.
     */
    val gridColumns: Int get() = if (isGrid) columns else DAYS_IN_WEEK
}

private val GRID_MODES = setOf(
    CalendarMode.DAY,
    CalendarMode.THREE_DAY,
    CalendarMode.WORK_WEEK,
    CalendarMode.WEEK,
)

/**
 * The zoom level showing `columns` of the week's days, the inverse of [CalendarMode.columns].
 *
 * A settled pinch often lands exactly between two rungs (two columns is as close to one as to
 * three). The tie is broken **towards more days**, deliberately, rather than by whatever order a
 * set happens to iterate in: showing a day the user didn't ask for is a smaller sin than hiding one
 * they did.
 */
internal fun modeForColumns(columns: Int): CalendarMode =
    GRID_MODES.minWithOrNull(
        compareBy({ abs(it.columns - columns) }, { -it.columns }),
    ) ?: CalendarMode.THREE_DAY

/**
 * The shape the grid opens in.
 *
 * The week, and not a subset of it, because a page **is** a week, and any zoom showing fewer than
 * seven columns leaves the rest of the page hanging off the side of the screen. That overflow is a
 * horizontal scroll nested inside the pager, and a nested scroll gets the drag *first*: a swipe
 * meant to turn the week is instead spent sliding along the days of the week you are already on,
 * and it comes to rest somewhere in the middle of it. Opening on the whole week means the columns
 * fill the viewport exactly, there is nothing to scroll, and every swipe reaches the pager.
 */
internal val DEFAULT_CALENDAR_MODE = CalendarMode.WEEK

/** The persisted core setting this mode is stored as, so the calendar reopens the way it was left. */
internal fun CalendarMode.toLayout(): CalendarLayout = when (this) {
    CalendarMode.DAY -> CalendarLayout.DAY
    CalendarMode.THREE_DAY -> CalendarLayout.THREE_DAY
    CalendarMode.WORK_WEEK -> CalendarLayout.WORK_WEEK
    CalendarMode.WEEK -> CalendarLayout.WEEK
    CalendarMode.MONTH -> CalendarLayout.MONTH
    CalendarMode.AGENDA -> CalendarLayout.AGENDA
}

/** The mode a persisted [CalendarLayout] restores to. */
internal fun CalendarLayout.toMode(): CalendarMode = when (this) {
    CalendarLayout.DAY -> CalendarMode.DAY
    CalendarLayout.THREE_DAY -> CalendarMode.THREE_DAY
    CalendarLayout.WORK_WEEK -> CalendarMode.WORK_WEEK
    CalendarLayout.WEEK -> CalendarMode.WEEK
    CalendarLayout.MONTH -> CalendarMode.MONTH
    CalendarLayout.AGENDA -> CalendarMode.AGENDA
}

/**
 * Maps pager pages to anchor dates, and moves the origin when the user switches view or jumps home.
 *
 * [origin] is the date page [CALENDAR_PAGE_ORIGIN] shows. It only moves on a deliberate jump, a
 * view switch or "back to today", never on a swipe, because a swipe is just a different page over
 * the same origin. [resetToken] changes whenever the origin moves, telling the screen to snap the
 * pager back to the middle so the new origin is what's on screen.
 */
internal class CalendarPager(today: LocalDate, mode: CalendarMode = DEFAULT_CALENDAR_MODE) {
    /** The shape being drawn. */
    var mode by mutableStateOf(mode)
        private set

    /** The date page [CALENDAR_PAGE_ORIGIN] shows. */
    var origin by mutableStateOf(today)
        private set

    /** Bumped whenever [origin] moves, so the screen re-centres the pager on it. */
    var resetToken by mutableStateOf(0)
        private set

    /**
     * The anchor date `page` shows, the first day the core's query is asked for.
     *
     * A grid page is a **whole week**, whatever the zoom: the week is the boundary a horizontal
     * scroll cannot cross, and the thing a sideways swipe pages between. So the zoom never changes
     * what this returns, only how many of that week's columns fit on screen.
     *
     * The month is the one shape a day-stride cannot express: months are 28–31 days long, so striding
     * by a constant would drift off the month within a year. It pages by calendar month instead.
     */
    fun anchorFor(page: Int): LocalDate {
        val step = (page - CALENDAR_PAGE_ORIGIN).toLong()
        return if (mode.isMonth) {
            // Anchor on the 1st, so adding months from (say) the 31st cannot silently clamp to the
            // 28th and lose a day each time.
            origin.withDayOfMonth(1).plusMonths(step)
        } else {
            origin.plusWeeks(step)
        }
    }

    /** Switches shape, keeping the period the user is looking at. */
    fun setMode(next: CalendarMode, currentPage: Int) {
        if (next == mode) return
        origin = anchorFor(currentPage)
        mode = next
        resetToken += 1
    }

    /**
     * Changes the **zoom level** without touching the origin.
     *
     * The difference from [setMode] matters: that one re-origins on the page you are on, which is
     * right for a menu choice and wrong for a pinch. A zoom must leave the week exactly where it is:
     * the columns only get wider.
     */
    fun setZoom(next: CalendarMode) {
        if (next == mode || !next.isGrid) return
        mode = next
    }

    /** Re-centres on `date` (the "back to today" affordance, with today). */
    fun jumpTo(date: LocalDate) {
        origin = date
        resetToken += 1
    }
}

/**
 * Which column of its own week [date] sits in, `0` for the first.
 *
 * The grid scrolls here when it opens, and when the user asks to come home. Both used to scroll to
 * column 0, the first day of the week, which is not where today is on any day but the first: on a
 * Sunday, with a Monday-start week, today is the *last* column and was six of them off the edge of
 * the screen. The app opened on a week that did not visibly contain today.
 *
 * [weekStart] comes from the **core** (`week_start_date`), so this cannot disagree with the columns
 * the core laid out, deriving it from the device locale here is how the two drift apart.
 */
internal fun todayColumn(date: LocalDate, weekStart: LocalDate): Int =
    ChronoUnit.DAYS.between(weekStart, date).toInt().coerceIn(0, DAYS_IN_WEEK - 1)

/**
 * Which column the grid's left edge frames on when it opens or jumps home.
 *
 * A deliberate, cross-platform product decision (kept identical on Windows):
 * - **Work week** is framed from the week's *first day*, not today: "work week" means Monday–Friday,
 *   so it always opens on the aligned week start and shows five days, whatever day it is. (Reach the
 *   weekend by scrolling, a page is still a whole week.)
 * - The whole **week** is framed from the week's first day too. Here that is a no-op, the seven
 *   columns fill the viewport, so `maxDayX` is zero and any framing column at all was already being
 *   clamped away, but it is said out loud rather than left to fall out of a bound, because a client
 *   whose days are *not* bounded by their week would otherwise open the grid on a week that **begins**
 *   on today: a Tuesday under the first heading, which is the exact misalignment this file exists to
 *   prevent. (Windows found this the hard way when its day axis became one continuous strip; see
 *   docs/calendar.md §3 and §6.)
 * - **Day** shows today; **3-day** shows today plus the next two, today at the left edge.
 */
internal fun framingColumn(mode: CalendarMode, today: LocalDate, weekStart: LocalDate): Int =
    if (mode == CalendarMode.WORK_WEEK || mode == CalendarMode.WEEK) 0
    else todayColumn(today, weekStart)

/**
 * Whether the user's locale starts its weeks on Monday.
 *
 * Get this wrong and every column shifts, so the user reads Tuesday's meetings under Monday's
 * heading. Derived from the locale (Monday across Europe, Sunday in the US and much of Asia).
 */
internal fun weekStartsMonday(locale: Locale): Boolean =
    WeekFields.of(locale).firstDayOfWeek == DayOfWeek.MONDAY
