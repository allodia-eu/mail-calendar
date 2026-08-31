// The calendar's localised copy. The core emits ISO dates and wall-clock minutes and nothing else,
// so everything a user actually reads on the grid is assembled here, which means a bug here is a
// bug the user sees.
package eu.allodia.mailcal

import androidx.compose.ui.unit.dp
import java.time.LocalDate
import java.util.Locale
import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Test

private val EN = Locale.forLanguageTag("en-GB")
private val NL = Locale.forLanguageTag("nl-NL")

private fun days(vararg iso: String): List<LocalDate> = iso.map(::parseIsoDate)

class CalendarFormatTest {

    @Test
    fun a_week_inside_one_month_is_titled_with_that_month() {
        val week = days("2026-07-06", "2026-07-07", "2026-07-08", "2026-07-12")
        assertEquals("Jul 2026", periodTitle(week, EN))
    }

    @Test
    fun a_week_straddling_a_month_names_both() {
        // Titling this week "July" is wrong for three of the columns on screen.
        val week = days("2026-06-29", "2026-06-30", "2026-07-01", "2026-07-05")
        assertEquals("Jun – Jul 2026", periodTitle(week, EN))
    }

    @Test
    fun a_week_straddling_a_year_names_both_years() {
        val week = days("2026-12-28", "2026-12-31", "2027-01-01", "2027-01-03")
        assertEquals("Dec 2026 – Jan 2027", periodTitle(week, EN))
    }

    @Test
    fun the_title_and_the_weekday_headings_are_localized() {
        // **Assert that it is Dutch, not which Dutch.**
        //
        // The exact abbreviation is the *platform's* to choose, and it moves under us. Dutch July is
        // `jul.` on JDK 17 and `jul` on JDK 21, a newer CLDR dropped the full stop, and under
        // Robolectric `java.time` reads the locale data of the **host JDK**, which is not the ICU data
        // the device uses at all. (On a real phone the header renders `jul 2026`, no stop; CI's JDK 17
        // renders `jul.`.) So pinning the glyphs tests the JDK the test happens to run on, and it
        // failed in CI while passing locally for no better reason than a version gap.
        //
        // What the client actually promises (§1: all localised copy is assembled client-side) is that
        // the title names the right month, in the user's language, with the year. That is what this
        // asserts, and it still catches every bug worth catching: the wrong month, a missing year, or
        // English served to a Dutch user.
        val dutch = periodTitle(days("2026-07-06"), NL).lowercase(NL)
        assertTrue("expected a Dutch July, got '$dutch'", dutch.startsWith("jul"))
        assertTrue("the title must name the year, got '$dutch'", dutch.contains("2026"))
        assertTrue("a Dutch title must not be English", !dutch.contains("july"))

        // The heading a Dutch user reads over Monday's column. Same rule: `ma` or `ma.`, never `mon`.
        assertEquals("ma", weekdayShort(parseIsoDate("2026-07-06"), NL).lowercase(NL).take(2))
        assertEquals("mon", weekdayShort(parseIsoDate("2026-07-06"), EN).lowercase(EN).take(3))
    }

    @Test
    fun week_numbers_are_iso_8601() {
        // The "WK 28" a Dutch or German user expects. ISO weeks start on Monday and belong to the
        // year holding their Thursday, so the turn of the year is where a naive implementation
        // breaks.
        assertEquals(28, isoWeekNumber(parseIsoDate("2026-07-06")))
        assertEquals(28, isoWeekNumber(parseIsoDate("2026-07-12"))) // the Sunday closing week 28
        assertEquals(29, isoWeekNumber(parseIsoDate("2026-07-13"))) // the Monday opening week 29
        // 2027-01-01 is a Friday, so it belongs to ISO week 53 of 2026, not week 1.
        assertEquals(53, isoWeekNumber(parseIsoDate("2027-01-01")))
    }

    @Test
    fun the_hour_ruler_follows_the_devices_clock_setting() {
        assertEquals("09", hourLabel(9, use24Hour = true))
        assertEquals("23", hourLabel(23, use24Hour = true))
        assertEquals("9 AM", hourLabel(9, use24Hour = false))
        assertEquals("12 PM", hourLabel(12, use24Hour = false))
        assertEquals("11 PM", hourLabel(23, use24Hour = false))
        // Midnight is deliberately unlabelled: it would collide with the day heading above it.
        assertEquals("", hourLabel(0, use24Hour = true))
        assertEquals("", hourLabel(0, use24Hour = false))
    }

    @Test
    fun clock_times_round_trip_the_awkward_hours() {
        assertEquals("09:30", clockTime(570, use24Hour = true))
        assertEquals("00:00", clockTime(0, use24Hour = true))
        assertEquals("23:59", clockTime(1439, use24Hour = true))
        // 12-hour has two traps: midnight is "12 AM", not "0 AM", and noon is "12 PM", not "0 PM".
        assertEquals("12:00 AM", clockTime(0, use24Hour = false))
        assertEquals("12:30 PM", clockTime(750, use24Hour = false))
        assertEquals("9:30 AM", clockTime(570, use24Hour = false))
        assertEquals("11:59 PM", clockTime(1439, use24Hour = false))
    }

    @Test
    fun a_blocks_spoken_time_is_the_range_it_covers() {
        assertEquals("09:30 – 09:45", timeRange(570, 585, use24Hour = true))
    }

    @Test
    fun a_block_never_draws_a_label_it_has_no_room_for() {
        // Regression: Material's labelSmall carries a 16sp line box, TALLER than a quarter-hour
        // block at any sane zoom, so the clip sliced every standup's title through the middle. The
        // grid was geometrically perfect and looked broken.
        //
        // Asserted here, in dp and sp, rather than in a UI test, because a UI test CANNOT see it:
        // Compose coerces the text node to the space available, so the node reports a perfectly good
        // fit while the glyphs overflow it and get clipped.
        //
        // Swept across the whole zoom range, because the hour height is no longer a constant: what
        // fits pinched in does not fit zoomed out, and 15 minutes (the core's minimum segment) is
        // the shortest block that can exist.
        for (hourHeight in listOf(20.dp, 32.dp, 48.dp, 64.dp, 120.dp, 200.dp)) {
            for (minutes in listOf(15, 20, 30, 45, 60, 120)) {
                if (!blockShowsLabel(minutes, hourHeight)) continue
                val space = blockLabelSpace(minutes, hourHeight).value
                val line = blockLabelLineHeight(minutes).value
                // A block long enough to show its start time needs room for a second line too.
                val needed = if (blockShowsTime(minutes, hourHeight)) line * 2 else line
                assertTrue(
                    "at $hourHeight/hour a $minutes-minute block gives ${space}dp but draws a " +
                        "label needing ${needed}dp, the title will be cut through the middle",
                    needed <= space,
                )
            }
        }
    }

    @Test
    fun zooming_in_reveals_a_short_events_title_and_zooming_out_hides_it() {
        // The behaviour that makes the rule above acceptable rather than a silent loss: a 15-minute
        // event is a few pixels tall at the whole-day zoom and simply cannot hold text, so it stays
        // a coloured block (keeping its spoken label), and pinching in brings the title back.
        assertTrue("pinched in, a standup shows its name", blockShowsLabel(15, 200.dp))
        assertTrue("zoomed out to the whole day, it cannot", !blockShowsLabel(15, 20.dp))
        // An hour-long meeting reads at any sane zoom.
        assertTrue(blockShowsLabel(60, 48.dp))
    }
}
