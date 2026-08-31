// The layout tests split out of CalendarGridTest.kt: the ONLY thing this client contributes to the
// grid's layout, the multiplication described in the comment below.
package eu.allodia.mailcal

import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner

@RunWith(RobolectricTestRunner::class)
class CalendarGridGeometryTest : CalendarGridTestBase() {

    // The three tests below pin the ONLY thing this client contributes to layout: the
    // multiplication. The core solves the grid in units, a day *index*, a wall-clock *minute*, a
    // column *fraction*, and this client turns those into pixels. Swap `column` for `columns`, or
    // forget to offset by the day, and every block still renders perfectly plausibly, in the wrong
    // place. Deltas rather than absolutes, so the assertions survive the vertical scroll and any
    // screen width.

    @Test
    fun an_hour_is_the_same_height_wherever_it_falls_on_the_grid() {
        // The hour height is no longer a constant, it is the viewport divided by the zoom horizon:
        // so this asserts the property that actually matters: time maps *linearly* to vertical
        // offset. 09:30 → 10:30 must be exactly as tall as 10:30 → 11:30, whatever the zoom.
        screen(
            gridPage(
                timed = listOf(
                    gridBlock(title = "Alpha", day = 2, startMinutes = 570, endMinutes = 630),
                    gridBlock(title = "Bravo", day = 2, startMinutes = 630, endMinutes = 690),
                    gridBlock(title = "Delta", day = 2, startMinutes = 690, endMinutes = 750),
                ),
            ),
        )
        val alpha = boundsOf("Alpha")
        val bravo = boundsOf("Bravo")
        val delta = boundsOf("Delta")

        val firstHour = (bravo.top - alpha.top).value
        val secondHour = (delta.top - bravo.top).value
        assertTrue("later events must sit lower, got $firstHour", firstHour > 0f)
        assertEquals("an hour changed height halfway down the grid", firstHour, secondHour, 1f)
    }

    @Test
    fun a_blocks_height_tracks_its_duration() {
        // Three blocks 30 minutes apart in duration must be equally far apart in height, because a
        // block's rectangle is exactly its duration times the hour height. (The gap that keeps two
        // adjacent blocks from touching is drawn *inside* this rectangle, so it does not enter here:
        // and it would cancel in the differences even if it did.)
        screen(
            gridPage(
                timed = listOf(
                    gridBlock(title = "Short", day = 1, startMinutes = 540, endMinutes = 570),
                    gridBlock(title = "Medium", day = 3, startMinutes = 540, endMinutes = 600),
                    gridBlock(title = "Long", day = 5, startMinutes = 540, endMinutes = 630),
                ),
            ),
        )
        fun height(title: String): Float {
            val bounds = boundsOf(title)
            return (bounds.bottom - bounds.top).value
        }
        val short = height("Short")
        val medium = height("Medium")
        val long = height("Long")

        assertTrue("blocks must have height", short > 0f)
        assertTrue("a longer event must be taller", medium > short && long > medium)
        assertEquals(
            "half an hour of extra duration changed height depending on where it was added",
            medium - short,
            long - medium,
            1f,
        )
    }

    @Test
    fun the_day_offset_does_not_accumulate_error_across_the_week() {
        // A day column is rarely a whole number of pixels wide (a 7-column week over any real
        // screen), and `Modifier.offset` rounds to whole pixels, so a single gap can legitimately
        // land a pixel either side of the true column width.
        //
        // What must NOT happen is that the error *accumulates*: the block on the last column is
        // placed at `dayWidth * 6`, not at "the previous column plus a width" six times over. If it
        // were the latter, a week's worth of rounding would drift the Sunday column visibly off its
        // own gridline. That is the property pinned here.
        // Measured at the two ends and the middle. Deriving the column width from a single rounded
        // gap and multiplying it out would amplify that one pixel of rounding sixfold, testing the
        // test, not the grid. Interpolating between the endpoints does not.
        screen(
            gridPage(
                timed = listOf(
                    gridBlock(title = "Monday", day = 0),
                    gridBlock(title = "Thursday", day = 3),
                    gridBlock(title = "Sunday", day = 6),
                ),
            ),
        )
        val monday = boundsOf("Monday")
        val thursday = boundsOf("Thursday")
        val sunday = boundsOf("Sunday")

        assertTrue("columns should advance rightwards", sunday.left > monday.left)
        // Column 3 of 0..6 sits exactly halfway across, which is only true if each block is placed
        // at `dayWidth * index` rather than by stepping a rounded width along the week.
        assertEquals(
            "the day offset accumulates rounding error across the week",
            (monday.left.value + sunday.left.value) / 2f,
            thursday.left.value,
            1.5f,
        )
        assertEquals(
            "the day columns are not equally wide",
            (monday.right - monday.left).value,
            (sunday.right - sunday.left).value,
            1.5f,
        )
    }

    @Test
    fun two_overlapping_events_split_the_day_into_side_by_side_lanes() {
        // The core's overlap solution: a cluster of two sits in lanes 0 and 1 of 2. If the client
        // ignored `columns` both blocks would be full width and the later one would simply cover
        // the earlier, an event hidden behind another is an event you miss.
        screen(
            gridPage(
                timed = listOf(
                    gridBlock(title = "Alpha", day = 2, column = 0, columns = 2),
                    gridBlock(title = "Bravo", day = 2, column = 1, columns = 2),
                ),
            ),
        )
        val alpha = boundsOf("Alpha")
        val bravo = boundsOf("Bravo")
        // Same hour, so they must be side by side rather than stacked.
        assertEquals(alpha.top.value, bravo.top.value, 0.5f)
        assertTrue("lane 1 must sit to the right of lane 0", bravo.left > alpha.left)
        // And they must not overlap: lane 0 ends before lane 1 begins.
        assertTrue(
            "the lanes overlap, one event is drawn on top of the other",
            alpha.right.value <= bravo.left.value + 0.5f,
        )
    }
}
