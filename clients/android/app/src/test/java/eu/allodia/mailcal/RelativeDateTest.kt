// The shared relative-timestamp bucketing policy (docs/timestamps.md): today → the clock, the
// previous six days → short weekday, this year → day + month, older → day + month + year. The
// pattern selection is pure, so it needs no clock and no Robolectric, this is the one gate that
// runs locally on a policy Apple and Windows re-implement by hand.
package eu.allodia.mailcal

import org.junit.Assert.assertEquals
import org.junit.Test

class RelativeDateTest {
    @Test
    fun today_is_the_clock_honouring_the_24_hour_setting() {
        assertEquals("HH:mm", relativeDatePattern(dayDiff = 0, sameYear = true, use24Hour = true))
        assertEquals("h:mm a", relativeDatePattern(dayDiff = 0, sameYear = true, use24Hour = false))
    }

    @Test
    fun the_previous_six_days_are_the_short_weekday() {
        for (d in 1..6) {
            assertEquals("EEE", relativeDatePattern(dayDiff = d, sameYear = true, use24Hour = true))
        }
    }

    @Test
    fun a_week_ago_falls_back_to_the_date_so_a_weekday_is_never_ambiguous() {
        // Day 7 is the same weekday as today; showing "Mon" for it would read as *this* Monday.
        assertEquals("d MMM", relativeDatePattern(dayDiff = 7, sameYear = true, use24Hour = true))
    }

    @Test
    fun an_older_year_carries_the_year() {
        assertEquals(
            "d MMM yyyy",
            relativeDatePattern(dayDiff = 400, sameYear = false, use24Hour = true),
        )
    }
}
