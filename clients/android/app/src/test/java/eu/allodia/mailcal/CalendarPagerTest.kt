// The calendar's navigation model: which week a pager page shows, and what a zoom does to it.
//
// The rule this file exists to defend: **a zoom must not move the days**. A page is a whole week and
// the zoom only decides how many of its columns fit on screen. The first design snapped a pinch to a
// differently-anchored "view", and a Monday-aligned week cannot contain an arbitrary three-day
// window, so a user reading Sunday-to-Tuesday who pinched outwards was shown the *previous*
// Monday-to-Sunday, and two of the three days they were reading vanished. It looked like a glitch;
// it was the design.
package eu.allodia.mailcal

import java.time.LocalDate
import java.util.Locale
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNotEquals
import org.junit.Assert.assertTrue
import org.junit.Test

// The Monday opening the week that holds Sunday 2026-07-12.
private val WEEK: LocalDate = LocalDate.of(2026, 7, 6)

class CalendarPagerTest {

    @Test
    fun a_grid_page_is_a_whole_week_and_swiping_moves_a_whole_week() {
        val pager = CalendarPager(WEEK, CalendarMode.THREE_DAY)
        assertEquals(WEEK, pager.anchorFor(CALENDAR_PAGE_ORIGIN))
        assertEquals(WEEK.plusWeeks(1), pager.anchorFor(CALENDAR_PAGE_ORIGIN + 1))
        assertEquals(WEEK.minusWeeks(1), pager.anchorFor(CALENDAR_PAGE_ORIGIN - 1))
    }

    @Test
    fun the_page_is_the_same_week_at_every_zoom() {
        // The whole point. Day, three-day and week are ONE grid at three zooms, the page they sit on
        // does not change, so neither can the days.
        for (mode in listOf(
            CalendarMode.DAY,
            CalendarMode.THREE_DAY,
            CalendarMode.WORK_WEEK,
            CalendarMode.WEEK,
        )) {
            val pager = CalendarPager(WEEK, mode)
            assertEquals("$mode", WEEK, pager.anchorFor(CALENDAR_PAGE_ORIGIN))
            assertEquals("$mode", WEEK.plusWeeks(2), pager.anchorFor(CALENDAR_PAGE_ORIGIN + 2))
        }
    }

    @Test
    fun a_zoom_does_not_move_the_week() {
        // THE regression. `setZoom` leaves the origin alone: after a pinch the user is looking at the
        // same week, with the columns a different width. `setMode` (a menu choice) is allowed to
        // re-origin; a pinch is not.
        val pager = CalendarPager(WEEK, CalendarMode.THREE_DAY)
        val paged = CALENDAR_PAGE_ORIGIN + 3
        val week = pager.anchorFor(paged)

        pager.setZoom(CalendarMode.WEEK)
        assertEquals(CalendarMode.WEEK, pager.mode)
        assertEquals("the origin moved under a zoom", WEEK, pager.origin)
        assertEquals("the week on screen changed under a zoom", week, pager.anchorFor(paged))

        pager.setZoom(CalendarMode.DAY)
        assertEquals(week, pager.anchorFor(paged))
    }

    @Test
    fun a_zoom_never_re_centres_the_pager() {
        // Re-centring mid-gesture would yank the strip out from under the fingers driving it.
        val pager = CalendarPager(WEEK, CalendarMode.THREE_DAY)
        val token = pager.resetToken
        pager.setZoom(CalendarMode.WEEK)
        assertEquals(token, pager.resetToken)
    }

    @Test
    fun a_zoom_cannot_turn_the_grid_into_a_month_or_an_agenda() {
        // Those are different layouts, not zoom levels, a pinch must not be able to reach them.
        val pager = CalendarPager(WEEK, CalendarMode.WEEK)
        pager.setZoom(CalendarMode.MONTH)
        assertEquals(CalendarMode.WEEK, pager.mode)
        pager.setZoom(CalendarMode.AGENDA)
        assertEquals(CalendarMode.WEEK, pager.mode)
    }

    @Test
    fun choosing_a_shape_from_the_menu_keeps_the_period_you_are_looking_at() {
        val pager = CalendarPager(WEEK, CalendarMode.WEEK)
        val paged = CALENDAR_PAGE_ORIGIN + 3
        val week = pager.anchorFor(paged)

        pager.setMode(CalendarMode.MONTH, paged)
        assertEquals(week, pager.origin)
        assertNotEquals(0, pager.resetToken)
    }

    @Test
    fun re_selecting_the_current_shape_does_nothing() {
        // Otherwise re-tapping "Week" would silently re-origin the strip on the page you had swiped
        // to, and the *next* "back to today" would be measured from there.
        val pager = CalendarPager(WEEK, CalendarMode.WEEK)
        val token = pager.resetToken
        pager.setMode(CalendarMode.WEEK, CALENDAR_PAGE_ORIGIN + 3)
        assertEquals(token, pager.resetToken)
        assertEquals(WEEK, pager.origin)
    }

    @Test
    fun back_to_today_re_origins_and_re_centres() {
        val pager = CalendarPager(WEEK, CalendarMode.WEEK)
        pager.setMode(CalendarMode.DAY, CALENDAR_PAGE_ORIGIN + 10)
        assertNotEquals(WEEK, pager.origin)

        val token = pager.resetToken
        pager.jumpTo(WEEK)
        assertEquals(WEEK, pager.origin)
        assertEquals(WEEK, pager.anchorFor(CALENDAR_PAGE_ORIGIN))
        assertNotEquals(token, pager.resetToken)
    }

    @Test
    fun the_strip_reaches_far_past_the_cores_horizon() {
        // Paging must never dead-end. The core's rolling horizon runs out long before the strip does
        // and reports `isMaterialized = false` when it has, so the user hits an honest "loading",
        // never an edge.
        val pager = CalendarPager(WEEK, CalendarMode.WEEK)
        val furthest = pager.anchorFor(CALENDAR_PAGE_COUNT - 1)
        assertTrue("should reach decades ahead, reached $furthest", furthest.year >= 2100)
    }

    @Test
    fun each_shape_shows_the_right_number_of_columns() {
        assertEquals(1, CalendarMode.DAY.columns)
        assertEquals(3, CalendarMode.THREE_DAY.columns)
        assertEquals(5, CalendarMode.WORK_WEEK.columns)
        assertEquals(7, CalendarMode.WEEK.columns)
        // And a settled pinch maps back to the nearest shape.
        assertEquals(CalendarMode.DAY, modeForColumns(1))
        assertEquals(CalendarMode.THREE_DAY, modeForColumns(3))
        assertEquals(CalendarMode.WORK_WEEK, modeForColumns(5))
        assertEquals(CalendarMode.WEEK, modeForColumns(7))
        // Between rungs it takes the nearer one rather than refusing to land.
        assertEquals(CalendarMode.THREE_DAY, modeForColumns(2))
        assertEquals(CalendarMode.WEEK, modeForColumns(6))
    }

    @Test
    fun the_agenda_is_not_a_grid_and_does_not_page() {
        assertFalse(CalendarMode.AGENDA.isGrid)
        assertEquals(0, CalendarMode.AGENDA.columns)
    }

    @Test
    fun the_week_start_follows_the_locale() {
        // Only a fallback: the core owns the real setting. But an app that had to guess should guess
        // the way the user's locale does.
        assertTrue(weekStartsMonday(Locale.forLanguageTag("nl-NL")))
        assertTrue(weekStartsMonday(Locale.forLanguageTag("en-GB")))
        assertFalse(weekStartsMonday(Locale.forLanguageTag("en-US")))
    }

    @Test
    fun `the grid opens on the whole week, so a swipe turns it instead of sliding along it`() {
        // Two bugs in one default. It opened on THREE_DAY, which (a) showed Monday-to-Wednesday, so
        // on a Sunday today was four columns off the right of the screen and the app appeared to
        // open on a week that did not contain today; and (b) left the other four columns hanging off
        // the side as a nested horizontal scroll, which takes a drag before the pager does, so a
        // swipe meant for the next week was spent sliding along this one and stopped in the middle.
        assertEquals(CalendarMode.WEEK, DEFAULT_CALENDAR_MODE)
        assertEquals(DAYS_IN_WEEK, DEFAULT_CALENDAR_MODE.columns)
    }

    @Test
    fun `today is a column of its week, not always the first one`() {
        // What the grid scrolls to when it opens, and when the user asks to come home. Both used to
        // scroll to column 0, the week's first day, which is where today is exactly one day in
        // seven. On the Sunday this was found, today was the LAST column and six of them off screen.
        val monday = LocalDate.of(2026, 7, 6)
        assertEquals(0, todayColumn(monday, monday))
        assertEquals(2, todayColumn(LocalDate.of(2026, 7, 8), monday))
        assertEquals(6, todayColumn(LocalDate.of(2026, 7, 12), monday), )
        // A date outside the week cannot produce an off-grid column.
        assertEquals(6, todayColumn(LocalDate.of(2026, 7, 30), monday))
        assertEquals(0, todayColumn(LocalDate.of(2026, 6, 1), monday))
    }

    @Test
    fun `the wide shapes frame from the week start, the narrow ones on today`() {
        // A deliberate, cross-platform product decision (kept identical on Windows): the wide shapes
        // open on the week's first day whatever day it is; the narrow ones frame on today.
        val monday = LocalDate.of(2026, 7, 13)
        val tuesday = LocalDate.of(2026, 7, 14)

        assertEquals(0, framingColumn(CalendarMode.WORK_WEEK, tuesday, monday))
        // Even on a weekend the work week still opens Mon–Fri.
        assertEquals(0, framingColumn(CalendarMode.WORK_WEEK, LocalDate.of(2026, 7, 18), monday))

        // Day shows today; 3-day shows today plus the next two (today at the left edge → Tue–Thu).
        assertEquals(1, framingColumn(CalendarMode.DAY, tuesday, monday))
        assertEquals(1, framingColumn(CalendarMode.THREE_DAY, tuesday, monday))

        // **The whole week frames on the week's FIRST DAY, and it says so rather than relying on a
        // clamp.** Here it is a no-op either way, the seven columns fill the viewport, so `maxDayX` is
        // zero and a framing column of 1 was already being clamped to 0 by the surface. But a client
        // whose days are NOT bounded by their week would open on a week that *begins* on Tuesday, which
        // is the misalignment §3 exists to prevent, and that is exactly what happened on Windows when
        // its day axis became one continuous strip. A rule that only holds because of a bound somewhere
        // else is not a rule.
        assertEquals(0, framingColumn(CalendarMode.WEEK, tuesday, monday))
    }
}
