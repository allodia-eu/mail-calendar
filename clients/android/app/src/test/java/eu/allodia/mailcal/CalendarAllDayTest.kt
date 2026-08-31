// The all-day banner's cap: when it hides bars, and how many it says it hid.
//
// The count is the whole point. A "+1" where it should say "+2" is a lie the user cannot see
// through, they tap, find an event they weren't told about, and now they don't trust the banner.
package eu.allodia.mailcal

import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test

// Only the geometry decides what a column is hiding, a title never did.
private fun band(day: Int, days: Int, lane: Int) = BandSpan(day = day, days = days, lane = lane)

class CalendarAllDayTest {

    @Test
    fun a_banner_that_fits_shows_everything_and_offers_no_expand() {
        // Exactly three lanes fit. Showing two and a "+1" here would be hiding an event for no
        // reason at all.
        for (lanes in 0..ALL_DAY_COLLAPSED_LANES) {
            assertFalse("$lanes lanes should fit", allDayOverflows(lanes))
            assertEquals(lanes, allDayDrawnLanes(lanes, expanded = false))
            assertEquals(lanes, allDayBannerLanes(lanes, expanded = false))
        }
    }

    @Test
    fun past_the_cap_the_last_row_becomes_the_more_chip() {
        val lanes = 5
        assertTrue(allDayOverflows(lanes))
        // Two rows of real bars, and a third row of "+N", so the banner is never taller than the
        // cap, however busy the week.
        assertEquals(ALL_DAY_VISIBLE_LANES, allDayDrawnLanes(lanes, expanded = false))
        assertEquals(ALL_DAY_COLLAPSED_LANES, allDayBannerLanes(lanes, expanded = false))
    }

    @Test
    fun expanding_shows_every_lane() {
        val lanes = 7
        assertEquals(lanes, allDayDrawnLanes(lanes, expanded = true))
        assertEquals(lanes, allDayBannerLanes(lanes, expanded = true))
    }

    @Test
    fun a_hidden_multi_day_bar_counts_against_every_day_it_covers() {
        // The trap. A three-day offsite pushed out of view is hidden on all three of its days, so it
        // must add one to each of their counts. Counting it once, on its first day, would leave
        // two columns quietly under-reporting.
        val bands = listOf(
            band(day = 0, days = 7, lane = 0), // visible
            band(day = 0, days = 7, lane = 1), // visible
            band(day = 1, days = 3, lane = 2), // hidden, spans days 1..3
        )
        val hidden = allDayOverflowPerDay(bands, dayCount = 7, drawnLanes = ALL_DAY_VISIBLE_LANES)
        assertEquals(listOf(0, 1, 1, 1, 0, 0, 0), hidden)
    }

    @Test
    fun each_column_reports_only_what_it_is_actually_hiding() {
        // Monday hides two, Friday hides one, the rest hide nothing, a single global "+N" would be
        // wrong on every column but one.
        val bands = listOf(
            band(day = 0, days = 7, lane = 0),
            band(day = 0, days = 7, lane = 1),
            band(day = 0, days = 1, lane = 2),
            band(day = 0, days = 1, lane = 3),
            band(day = 4, days = 1, lane = 2),
        )
        val hidden = allDayOverflowPerDay(bands, dayCount = 7, drawnLanes = ALL_DAY_VISIBLE_LANES)
        assertEquals(listOf(2, 0, 0, 0, 1, 0, 0), hidden)
    }

    @Test
    fun an_expanded_banner_hides_nothing() {
        val bands = listOf(
            band(day = 0, days = 7, lane = 0),
            band(day = 0, days = 7, lane = 1),
            band(day = 0, days = 7, lane = 2),
            band(day = 0, days = 7, lane = 3),
        )
        val drawn = allDayDrawnLanes(lanes = 4, expanded = true)
        assertEquals(List(7) { 0 }, allDayOverflowPerDay(bands, dayCount = 7, drawnLanes = drawn))
    }
}
