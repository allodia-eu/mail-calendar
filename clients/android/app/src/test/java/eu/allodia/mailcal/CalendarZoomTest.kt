// Pinch-to-zoom: the rules, without a two-finger gesture.
//
// The gesture itself has to be tried on a device, but everything that can actually be *wrong* about
// it is here, which way a pinch runs, where it stops, which axis it belongs to, and (the bug that
// made it feel broken) whether the content stays under the fingers.
package eu.allodia.mailcal

import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.unit.dp
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNotEquals
import org.junit.Assert.assertTrue
import org.junit.Test

class CalendarZoomTest {

    @Test
    fun spreading_your_fingers_shows_less_of_the_day_not_more() {
        // The direction is the whole feel of the gesture. Spreading (zoom > 1) means "closer", which
        // means FEWER hours on screen. Get it backwards and the grid zooms out when the user pinches
        // in, instantly, obviously broken.
        val zoom = CalendarZoom(12)
        zoom.pinchVertical(2f)
        assertEquals(6f, zoom.visibleHours, 0.01f)

        zoom.pinchVertical(0.5f)
        assertEquals(12f, zoom.visibleHours, 0.01f)
    }

    @Test
    fun the_horizon_stops_at_the_cores_limits() {
        // A pinch runs off the end of its own gesture constantly, fingers keep moving after the
        // grid has nothing left to give. It must stop, not invert or divide by nothing.
        val zoom = CalendarZoom(12)
        repeat(20) { zoom.pinchVertical(2f) }
        assertEquals(MIN_VISIBLE_HOURS, zoom.visibleHours, 0.01f)

        repeat(40) { zoom.pinchVertical(0.5f) }
        assertEquals(MAX_VISIBLE_HOURS, zoom.visibleHours, 0.01f)
    }

    @Test
    fun a_zoom_reports_the_factor_it_ACTUALLY_applied_not_the_one_it_was_asked_for() {
        // The caller corrects the scroll by this, to keep the content under the fingers still. At the
        // clamp the hour height did NOT change, so the factor must be exactly 1, or the grid would
        // be dragged on every further frame of a pinch that has nowhere left to go.
        val zoom = CalendarZoom(12)
        assertEquals(2f, zoom.pinchVertical(2f), 0.01f) // 12h -> 6h: an hour doubled in height

        repeat(20) { zoom.pinchVertical(2f) } // pinned at the minimum horizon
        assertEquals(1f, zoom.pinchVertical(2f), 0.001f)
    }

    @Test
    fun a_garbage_scale_factor_leaves_the_zoom_alone() {
        val zoom = CalendarZoom(12)
        assertEquals(1f, zoom.pinchVertical(0f), 0.001f)
        assertEquals(1f, zoom.pinchVertical(-3f), 0.001f)
        assertEquals(12f, zoom.visibleHours, 0.01f)
    }

    @Test
    fun the_content_under_your_fingers_stays_under_your_fingers() {
        // THE bug. Without this the scroll offset stays fixed in PIXELS while the hour height
        // changes, so the same offset maps to an earlier and earlier time, the grid slides out from
        // under the fingers and the zoom appears to be anchored to the top of the day.
        //
        // Scrolled 1000px down, fingers 500px into the viewport: the content point under them is
        // 1500px down the day. Double the scale and it sits at 3000px, so the scroll must become
        // 3000 - 500 = 2500 to leave it exactly where it was.
        assertEquals(2500f, focalPreservingScroll(scroll = 1000f, focus = 500f, factor = 2f), 0.01f)

        // Zooming out by the same factor puts it back.
        assertEquals(1000f, focalPreservingScroll(scroll = 2500f, focus = 500f, factor = 0.5f), 0.01f)

        // A zoom that changed nothing must not move the scroll at all.
        assertEquals(1234f, focalPreservingScroll(scroll = 1234f, focus = 400f, factor = 1f), 0.01f)

        // And it never scrolls above the top of the day.
        assertTrue(focalPreservingScroll(scroll = 0f, focus = 10f, factor = 0.2f) >= 0f)
    }

    @Test
    fun zooming_about_the_top_of_the_viewport_does_not_move_the_scroll() {
        // The degenerate case that proves the formula is anchored and not merely scaled: with the
        // fingers at the very top of the viewport and the grid at the top of the day, there is
        // nothing to correct.
        assertEquals(0f, focalPreservingScroll(scroll = 0f, focus = 0f, factor = 3f), 0.01f)
    }

    @Test
    fun an_axis_the_fingers_are_not_spread_along_reports_no_change() {
        // What keeps the axes independent WITHOUT forbidding a diagonal. In a purely horizontal pinch
        // the fingers sit at almost the same height, so the vertical spread is a few noisy pixels:
        // and dividing by it would produce a wild factor and lurch the hours about while the user was
        // only asking for more days.
        assertEquals(1f, axisScale(before = 3f, after = 9f), 0.001f)
        assertEquals(1f, axisScale(before = 200f, after = 4f), 0.001f)
        // Genuinely spread along the axis: a real scale.
        assertEquals(2f, axisScale(before = 100f, after = 200f), 0.001f)
    }

    @Test
    fun a_diagonal_pinch_zooms_both_axes_at_once_each_by_its_own_spread() {
        // The gesture that makes the grid feel alive: drag your fingers apart at an angle and the
        // hours and the days both stretch, each by how far the fingers actually travelled ALONG that
        // axis, not by some blended average of the two.
        //
        // Fingers 100px apart horizontally and 200px vertically, pulled to 200px and 300px: the day
        // axis scaled 2.0, the hour axis only 1.5. The two must not borrow from each other.
        val xScale = axisScale(before = 100f, after = 200f)
        val yScale = axisScale(before = 200f, after = 300f)
        assertEquals(2.0f, xScale, 0.001f)
        assertEquals(1.5f, yScale, 0.001f)

        val zoom = CalendarZoom(12, visibleDays = 6)
        zoom.pinchVertical(yScale)
        zoom.pinchHorizontal(xScale)
        assertEquals(8f, zoom.visibleHours, 0.01f) // 12 / 1.5
        assertEquals(3f, zoom.visibleDays, 0.01f) // 6 / 2.0
    }

    @Test
    fun a_diagonal_pinch_that_runs_out_of_hours_keeps_zooming_days() {
        // Each axis clamps on its own. If they were locked together, or if one dragged the other:
        // hitting the 4-hour floor would freeze the day axis mid-gesture, and the pinch would die in
        // the user's hand.
        val zoom = CalendarZoom(5, visibleDays = 7)
        repeat(10) {
            zoom.pinchVertical(2f)
            zoom.pinchHorizontal(1.2f)
        }
        assertEquals("the hours are pinned at their floor", MIN_VISIBLE_HOURS, zoom.visibleHours, 0.01f)
        assertTrue("but the days kept going", zoom.visibleDays < 7f)
    }

    @Test
    fun spreading_your_fingers_sideways_shows_fewer_days() {
        val zoom = CalendarZoom(12, visibleDays = 6)
        zoom.pinchHorizontal(2f)
        assertEquals(3f, zoom.visibleDays, 0.01f)
        assertEquals(3, zoom.settledDays())
        assertEquals(CalendarMode.THREE_DAY, modeForColumns(zoom.settledDays()))
    }

    @Test
    fun the_day_axis_stays_fractional_mid_pinch() {
        // Rounding to whole columns while the fingers are still moving would make the grid stutter
        // between views instead of tracking them. Only the settled value is whole.
        val zoom = CalendarZoom(12, visibleDays = 7)
        zoom.pinchHorizontal(1.2f)
        assertTrue("columns are continuous mid-pinch", zoom.visibleDays % 1f != 0f)
    }

    @Test
    fun the_week_is_the_boundary_a_zoom_cannot_cross() {
        // You can zoom in to a single day and out to the whole week. Never further: the week IS the
        // page, and beyond it you swipe rather than zoom.
        val zoom = CalendarZoom(12, visibleDays = 3)
        repeat(20) { zoom.pinchHorizontal(2f) }
        assertEquals(MIN_VISIBLE_DAYS, zoom.visibleDays, 0.01f)
        assertEquals(1, zoom.settledDays())

        repeat(40) { zoom.pinchHorizontal(0.5f) }
        assertEquals(MAX_VISIBLE_DAYS, zoom.visibleDays, 0.01f)
        assertEquals(DAYS_IN_WEEK, zoom.settledDays())
    }

    @Test
    fun the_day_axis_also_reports_the_factor_it_actually_applied() {
        // Same reason as the hours: the caller corrects the horizontal scroll by exactly this to keep
        // the column under the fingers still, and at the clamp nothing moved.
        val zoom = CalendarZoom(12, visibleDays = 6)
        assertEquals(2f, zoom.pinchHorizontal(2f), 0.01f)

        repeat(20) { zoom.pinchHorizontal(2f) } // pinned at one day
        assertEquals(1f, zoom.pinchHorizontal(2f), 0.001f)
    }

    @Test
    fun a_day_is_as_wide_as_the_viewport_divided_by_the_columns_on_it() {
        val zoom = CalendarZoom(12, visibleDays = 3)
        assertEquals(120f, zoom.dayWidth(360f), 0.01f)
        // Zoomed out to the whole week the columns narrow to fit it exactly, at which point the
        // horizontal scroll has nowhere to go, and a sideways swipe pages instead of scrolling.
        zoom.resetDays(7)
        assertEquals(360f / 7f, zoom.dayWidth(360f), 0.01f)
    }

    @Test
    fun the_horizon_is_what_decides_how_tall_an_hour_is() {
        // The bridge from the core's unit-free geometry to pixels. "Show me 12 hours" must mean the
        // same span of the day on a phone and on a tablet, the hour just gets taller.
        val zoom = CalendarZoom(12)
        assertEquals(50f, zoom.hourHeight(600f), 0.01f)
        assertEquals(80f, zoom.hourHeight(960f), 0.01f)

        zoom.pinchVertical(2f)
        assertEquals(100f, zoom.hourHeight(600f), 0.01f)
    }

    @Test
    fun settings_can_re_seed_the_zoom_so_the_picker_and_the_pinch_are_one_setting() {
        val zoom = CalendarZoom(12)
        zoom.pinchVertical(2f)
        assertEquals(6f, zoom.visibleHours, 0.01f)
        zoom.resetHours(16)
        assertEquals(16f, zoom.visibleHours, 0.01f)
    }

    @Test
    fun a_persisted_horizon_out_of_range_is_pulled_back_in() {
        assertEquals(MIN_VISIBLE_HOURS, CalendarZoom(0).visibleHours, 0.01f)
        assertEquals(MAX_VISIBLE_HOURS, CalendarZoom(99).visibleHours, 0.01f)
    }

    @Test
    fun `a settled pinch leaves whole columns, so the week fills the viewport exactly`() {
        // THE swipe bug. The page holds all seven days, at a width of viewport/visibleDays, so a
        // pinch that ends on 6.4 columns leaves 0.6 of a column hanging off the screen. That
        // overflow is a horizontal scroll nested INSIDE the pager, and a nested scroll takes the
        // drag first: the swipe that should turn the week is spent sliding along the current one,
        // and the grid comes to rest between two weeks. Whole columns are what make the scroll
        // range zero, and a zero scroll range is what hands every swipe to the pager.
        // A pinch outwards from the week lands on ~6.4 columns. Note where that rounds to: SIX:
        // while the zoom LEVEL it maps to is the whole week, of seven. Settling on the rounded
        // number rather than on the level's columns is what left the grid drawing seven columns at
        // one-sixth of the viewport each.
        val zoom = CalendarZoom(visibleHours = 12, visibleDays = 7)
        zoom.pinchHorizontal(1.1f)
        assertEquals(6, zoom.settledDays())
        assertEquals(CalendarMode.WEEK, modeForColumns(zoom.settledDays()))
        assertNotEquals(
            "mid-pinch it must stay fractional, or the columns stutter as the fingers move",
            0f,
            zoom.visibleDays % 1f,
            0.0001f,
        )

        zoom.settleDays()
        assertEquals(
            "the fingers lifted: the columns must be whole",
            0f,
            zoom.visibleDays % 1f,
            0.0001f,
        )

        // And the invariant that actually matters, stated in the units it breaks in: seven columns
        // at the settled width are exactly one viewport, so there is nothing left to scroll, and a
        // scroll range of zero is what hands the swipe to the pager.
        val viewport = 700f
        val dayWidth = viewport / zoom.visibleDays
        assertEquals(
            "the week must fill the viewport exactly, or the nested scroll eats the swipe",
            viewport,
            dayWidth * DAYS_IN_WEEK,
            0.01f,
        )
    }

    @Test
    fun `a settled pinch to a sub-week zoom does leave the week scrollable`() {
        // The other half of it: at three columns the rest of the week SHOULD hang off the side:
        // that is what makes it scrollable, and scrolling among the week's days is the design. The
        // rule is not "never scroll", it is "never rest on a fraction of a column".
        val zoom = CalendarZoom(visibleHours = 12, visibleDays = 3)
        zoom.pinchHorizontal(1.05f)
        zoom.settleDays()
        assertEquals(3f, zoom.visibleDays, 0.0001f)
        val viewport = 300f
        assertEquals(700f, (viewport / zoom.visibleDays) * DAYS_IN_WEEK, 0.01f)
    }

    @Test
    fun `two fingers travelling together are a swipe, not a pinch`() {
        // THE stuck-mid-scroll bug, and it was not in the pager at all, the pager's drag was being
        // STOLEN. The pinch detector claimed the gesture whenever the finger spread changed at all,
        // and two fingers 200px apart that wobble by a single pixel give a scale of 200/199 = 1.005,
        // which is not 1. So any two-finger contact was read as a pinch and its pointer events were
        // consumed, which CANCELS a drag rather than ending it, and a cancelled drag never flings.
        // Compose then leaves the pager exactly where it stopped. Measured on a real recording: at
        // 0.75 of a page, held for four tenths of a second, showing half of each of two weeks.
        val start = Offset(200f, 40f)

        // Fingers held the same distance apart while the hand travels: a swipe. Hands are not steady,
        // so allow for a few pixels of wobble, that is exactly what must NOT count.
        assertFalse(beginsPinch(start, Offset(203f, 43f)))
        assertFalse(beginsPinch(start, Offset(196f, 37f)))

        // Genuinely spread apart, on either axis: a pinch.
        assertTrue(beginsPinch(start, Offset(260f, 40f)))
        assertTrue(beginsPinch(start, Offset(200f, 90f)))
        // ...and closed, too.
        assertTrue(beginsPinch(start, Offset(150f, 40f)))
    }

    @Test
    fun `the spread is measured per axis, so a diagonal gesture is seen on both`() {
        val spread = spreadOf(Offset(100f, 500f), Offset(340f, 620f))
        assertEquals(240f, spread.x, 0.01f)
        assertEquals(120f, spread.y, 0.01f)
    }
}
