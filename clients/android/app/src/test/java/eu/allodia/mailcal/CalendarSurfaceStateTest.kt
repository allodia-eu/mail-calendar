// The single gesture owner's state machine: pan, page, and the hand-off between them.
//
// This is the logic that replaced Compose's nested scrolling, and it is worth testing precisely
// because the thing it replaced was *free*. A `horizontalScroll` inside a `HorizontalPager` gives you
// the inner-takes-the-drag-first rule without asking, and the day it stops being free is the day you
// have to say what it actually was. It was this.
package eu.allodia.mailcal

import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Test

// A 700x600 grid with a 52px hour ruler: the day columns get 648px.
private fun viewport(lanes: Int = 0) = SurfaceViewport(
    width = 700f,
    height = 600f,
    gutter = 52f,
    headerHeight = 56f,
    laneHeight = 24f,
    dividerHeight = 1f,
    lanes = lanes,
)

private fun state(hours: Int = 12, days: Int = 7) = CalendarSurfaceState(hours, days)

class CalendarSurfaceStateTest {

    @Test
    fun at_the_whole_week_zoom_every_sideways_drag_turns_the_page() {
        // The week fills the viewport exactly, so the day strip has nowhere to go, and *that* is what
        // hands the swipe to the page. It is the same invariant the settled pinch exists to protect,
        // seen from the other end: a scroll range of zero is a swipe that always pages.
        val s = state(days = 7)
        val m = s.metrics(viewport())
        assertEquals("the week fills the viewport exactly", 0f, m.maxDayX, 0.01f)

        s.panX(-40f, m)
        assertEquals("the days must not move", 0f, s.dayX, 0.01f)
        assertEquals("the week must", -40f, s.pageOffset, 0.01f)
    }

    @Test
    fun the_day_strip_takes_the_drag_first_and_the_week_gets_the_remainder() {
        // The nested-scroll rule, written out. At three columns the rest of the week hangs off the
        // side, scrolling among the week's days is the design, so a drag scrolls the days until the
        // week runs out, and only what is left over turns the page.
        val s = state(days = 3)
        val m = s.metrics(viewport())
        assertEquals(864f, m.maxDayX, 0.01f) // 3 columns of 216px: 1512px of week, 648px of viewport

        // A drag the strip can absorb entirely: the week does not budge.
        s.panX(-100f, m)
        assertEquals(100f, s.dayX, 0.01f)
        assertEquals("the week turned on a drag the days could have taken", 0f, s.pageOffset, 0.01f)

        // Drag past the end of the week: the strip takes what it can, the page takes the rest.
        s.panX(-800f, m)
        assertEquals("the strip is pinned at the end of the week", 864f, s.dayX, 0.01f)
        assertEquals("the overshoot turns the week", -36f, s.pageOffset, 0.01f) // 900 - 864
    }

    @Test
    fun reversing_a_drag_unwinds_the_week_before_the_days_move_again() {
        // A page offset only ever exists because the day strip was already at a bound. So dragging
        // back must put the week home *first*, otherwise the days would scroll away underneath a
        // page that was still half-turned, which is the grid coming to rest between two weeks by
        // another route.
        val s = state(days = 3)
        val m = s.metrics(viewport())
        s.panX(-900f, m)
        assertEquals(864f, s.dayX, 0.01f)
        assertEquals(-36f, s.pageOffset, 0.01f)

        // Back by less than the page owes: the page unwinds, the days stay put.
        s.panX(20f, m)
        assertEquals(-16f, s.pageOffset, 0.01f)
        assertEquals("the days moved while the week was still half-turned", 864f, s.dayX, 0.01f)

        // Back by more: the page lands home and the overshoot goes to the days.
        s.panX(50f, m)
        assertEquals("the week must be home", 0f, s.pageOffset, 0.01f)
        assertEquals("and the overshoot scrolls the days", 830f, s.dayX, 0.01f) // 864 - 34
    }

    @Test
    fun a_flick_turns_the_week_on_velocity_alone_however_short_the_drag() {
        // Most swipes commit this way. Measured against Samsung: at 20+ pages a second they are
        // committing on velocity, and the finger's travel is irrelevant to it.
        val barely = 700f * 0.02f // a fiftieth of the page, nowhere near the drag threshold
        assertEquals(
            "a flick must turn the week",
            1,
            pageTurnFor(drag = -barely, velocity = -3000f, width = 700f, minFlingVelocity = 400f),
        )
        // The same drag, released still: not enough travel to commit.
        assertEquals(
            "an idle wobble must not turn the week under you",
            0,
            pageTurnFor(drag = -barely, velocity = 0f, width = 700f, minFlingVelocity = 400f),
        )
    }

    @Test
    fun a_slow_drag_past_a_sixth_of_the_page_commits() {
        // Compose's default is half the screen, which feels like wading. The floor is set by the
        // opposite failure: low enough and an idle horizontal wobble turns the week under you.
        val short = 700f / 7f // under a sixth
        val long = 700f / 5f // over it
        assertEquals(0, pageTurnFor(short, 0f, 700f, 400f))
        assertEquals("dragging right reveals the PREVIOUS week", -1, pageTurnFor(long, 0f, 700f, 400f))
        assertEquals(1, pageTurnFor(-long, 0f, 700f, 400f))
    }

    @Test
    fun a_hard_flick_back_the_way_you_came_takes_the_turn_back() {
        // Dragged most of the way to next week, then flicked back: the week must stay. Without this a
        // swipe cannot be taken back, which is exactly the "it committed while I was changing my
        // mind" that makes a pager feel like it is fighting you. Note it CANCELS rather than
        // reversing: a change of mind is not a swipe to the previous week.
        val committed = -700f * 0.4f // well past the drag threshold, heading forwards
        assertEquals(
            "the flick back must cancel the turn",
            0,
            pageTurnFor(committed, velocity = 3000f, width = 700f, minFlingVelocity = 400f),
        )
        // ...but a gentle drift back does not: the drag already committed.
        assertEquals(1, pageTurnFor(committed, velocity = 100f, width = 700f, minFlingVelocity = 400f))
    }

    @Test
    fun a_flick_is_judged_on_what_the_FINGER_did_not_on_where_the_page_is_sitting() {
        // THE swallowed-swipe bug, in one assertion.
        //
        // A turn is banked the instant it is decided, so the page carries a LAG: a week already won
        // whose pixels have not arrived. That lag looks, to anything reading the page's position,
        // exactly like a drag in the opposite direction. Judge the second of two fast flicks by it and
        // it reads as "he has changed his mind" and cancels itself, which is the swipe going missing.
        //
        // So the flick is judged on its own finger: a short leftward flick turns the week ON, no
        // matter that the page is still sitting half a screen to the right, mid-catch-up.
        val flickLeft = -40f // this gesture's own travel: short, but fast
        assertEquals(
            "a flick must be able to turn the week while the last turn is still catching up",
            1,
            pageTurnFor(flickLeft, velocity = -4000f, width = 700f, minFlingVelocity = 400f),
        )
    }

    @Test
    fun fast_flicks_accumulate_instead_of_swallowing_one_another() {
        // Measured on a real phone, before the fix: EIGHT fast flicks turned THREE weeks. Each new
        // flick cancelled the animation of the one before it, and with it a week the user had already
        // won, the turn was only committed once its slide had finished, so a turn that was still
        // being drawn had never happened. A decision a later event can undo is not a decision.
        val s = state()
        val w = 700f

        // Flick one: the week lands NOW, and the page is left a whole width behind it.
        s.commitPageTurn(1, w)
        assertEquals("the week is banked immediately", 1, s.week)
        assertEquals("...and the pixels owe it a page", w, s.pageOffset, 0.01f)

        // The image must not have moved: page k sits at (k - week) * width + pageOffset, so the week
        // we were looking at is still exactly where it was.
        assertEquals("the frame must be identical", 0f, (0 - s.week) * w + s.pageOffset, 0.01f)

        // Flick two, arriving mid-slide, with the page still 300px behind.
        s.slidePageTo(300f)
        s.commitPageTurn(1, w)
        assertEquals("two flicks must be two weeks", 2, s.week)
        // ...and again the image has not jumped: the week we were mid-way through is where it was.
        assertEquals("the frame must be identical", 300f, (1 - s.week) * w + s.pageOffset, 0.01f)
    }

    @Test
    fun the_pixels_may_never_lag_further_than_the_grid_can_draw() {
        // The lag is what lets flicks accumulate, but it cannot run away: at a lag of `f` pages the
        // grid is drawing pages (-1-f)..(1-f), and it only holds two either side. Let it lag three and
        // it would slide through a week it does not have, and draw a hole.
        val s = state()
        val w = 700f
        repeat(10) { s.commitPageTurn(1, w) }
        assertEquals("every flick still counts", 10, s.week)
        assertEquals(
            "but the pixels may only fall so far behind",
            MAX_PAGE_LAG * w,
            s.pageOffset,
            0.01f,
        )
    }

    @Test
    fun a_settled_turn_leaves_nothing_between_two_weeks() {
        // The invariant survives the rework: the slide ends at zero, and zero is a week.
        val s = state()
        s.commitPageTurn(1, 700f)
        s.slidePageTo(0f)
        assertEquals(1, s.week)
        assertEquals("between two weeks is never a resting place", 0f, s.pageOffset, 0.001f)
    }

    @Test
    fun the_hours_scroll_and_stop_at_the_ends_of_the_day() {
        val s = state(hours = 12)
        val m = s.metrics(viewport())
        // A 12-hour horizon over a 600px surface: a 50px hour, so the day is 1200px tall. The grid
        // viewport is shorter than the surface by the chrome above it.
        assertEquals(50f, m.hourHeight, 0.01f)
        assertEquals(1200f, m.gridHeight, 0.01f)

        s.panY(-200f, m) // finger up: later in the day
        assertEquals(200f, s.scrollY, 0.01f)

        s.panY(5000f, m)
        assertEquals("the day has a top", 0f, s.scrollY, 0.01f)
        s.panY(-5000f, m)
        assertEquals("and a bottom", m.maxScrollY, s.scrollY, 0.01f)
        assertTrue("which is the day, less what is on screen", m.maxScrollY < m.gridHeight)
    }

    @Test
    fun a_pinch_keeps_the_time_under_your_fingers_under_your_fingers() {
        // The whole point of anchoring. Scrolled to 09:00 with the fingers halfway down the grid, a
        // pinch must leave whatever they are touching exactly where it is, otherwise the grid slides
        // out from under the hand and appears to zoom about the top of the day.
        val s = state(hours = 12, days = 7)
        val v = viewport()
        val before = s.metrics(v)

        s.scrollTo(9 * before.hourHeight, before) // 09:00 at the top of the grid
        val focus = 100f // 100px into the grid: two hours further down, so 11:00 is under the finger
        val timeUnderFinger = (s.scrollY + focus) / before.hourHeight
        assertEquals(11f, timeUnderFinger, 0.01f)

        // Spread vertically: fewer hours on screen, a taller hour.
        s.pinch(xScale = 1f, yScale = 1.5f, focusX = 0f, focusY = focus, viewport = v)
        val after = s.metrics(v)
        assertTrue("an hour must have got taller", after.hourHeight > before.hourHeight)
        assertEquals(
            "the grid slid out from under the fingers",
            11f,
            (s.scrollY + focus) / after.hourHeight,
            0.01f,
        )
    }

    @Test
    fun a_pinch_does_not_pan_the_grid() {
        // The gap this whole refactor closes. The old pinch consumed nothing, it had to, or it
        // cancelled the pager's drag, so the scrollers underneath went on reading the same two
        // fingers and dragged the week around while it zoomed. Nobody is reading them now.
        val s = state(hours = 12, days = 3)
        val v = viewport()
        s.scrollDaysTo(300f, s.metrics(v))
        s.panY(-100f, s.metrics(v))
        val dayX = s.dayX
        val scrollY = s.scrollY

        // A pinch that scales neither axis (the fingers are too close together on both to mean
        // anything) must move nothing at all, however far their midpoint has travelled.
        s.pinch(xScale = 1f, yScale = 1f, focusX = 400f, focusY = 250f, viewport = v)
        assertEquals("a pinch panned the days", dayX, s.dayX, 0.001f)
        assertEquals("a pinch panned the hours", scrollY, s.scrollY, 0.001f)
    }

    @Test
    fun the_shaper_sleeps_while_the_fingers_are_down() {
        // Measured on a real diary: a pinch's draw cost 3.4x a swipe's (1709us against 496us) while
        // drawing HALF as many blocks. Backwards, and the culprit is the one thing a zoom genuinely
        // changes. A swipe holds the column width still, so every text measurement hits the shaper's
        // cache for free; a pinch moves it every frame, taking the cache key with it, and re-shapes
        // every visible label sixty times a second. So the width the text is LAID OUT against stops
        // moving for the length of the gesture, while the rectangle it is clipped to keeps tracking
        // the fingers exactly as before.
        val s = state(days = 7)
        val v = viewport()
        val before = s.metrics(v).dayWidth
        assertEquals("at rest, the text is shaped against the real width", 0f, s.shapedDayWidth, 0f)

        s.beginZoom(s.metrics(v))
        assertEquals(before, s.shapedDayWidth, 0.01f)

        // The columns get much wider under the fingers...
        repeat(6) { s.pinch(xScale = 1.2f, yScale = 1f, focusX = 300f, focusY = 200f, viewport = v) }
        assertTrue("the geometry must follow the fingers", s.metrics(v).dayWidth > before * 1.5f)
        assertEquals(
            "...but the shaping width must not have moved at all",
            before,
            s.shapedDayWidth,
            0.01f,
        )

        // Fingers up: the labels may re-shape against the width they actually have.
        s.settleZoom(v)
        assertEquals(0f, s.shapedDayWidth, 0f)
    }

    @Test
    fun a_settled_pinch_snaps_to_a_rung_and_clamps_the_scroll_to_the_new_week() {
        // Zoom out from three columns to the whole week and the week gets *narrower* than it was:
        // so a day scroll that was legal a moment ago now points past the end of it. Left alone, the
        // grid would come to rest showing empty space beyond Sunday.
        val s = state(hours = 12, days = 3)
        val v = viewport()
        s.scrollDaysTo(864f, s.metrics(v)) // hard against the end of the week
        assertEquals(864f, s.dayX, 0.01f)

        s.resetDays(7)
        val settled = s.settleZoom(v)
        assertEquals(CalendarMode.WEEK, settled)
        assertEquals("the whole week fits, so there is nothing left to scroll", 0f, s.dayX, 0.01f)
        assertEquals(0f, s.metrics(v).maxDayX, 0.01f)
    }

    @Test
    fun a_shape_picked_from_the_menu_cannot_leave_the_week_scrolled_off_the_screen() {
        // The regression. A pinch clamps as it goes, so it looked as though this was covered, and it
        // was not. Pick "3 days" from the menu, scroll deep into the week, then pick "Week": the
        // columns widen, `maxDayX` collapses to zero, and a `dayX` nobody re-clamped draws the whole
        // week a thousand pixels off to the left. The grid comes up **blank**, no columns, no day
        // headings, which looks like a rendering crash and is really just a stale offset.
        val s = state(days = 3)
        val v = viewport()
        s.scrollDaysTo(700f, s.metrics(v))
        assertEquals(700f, s.dayX, 0.01f)

        // The menu re-seeds the day axis. It does NOT clamp, nothing in the zoom knows a pixel.
        s.resetDays(7)
        assertEquals("the week now fills the viewport", 0f, s.metrics(v).maxDayX, 0.01f)

        // Whatever the grid does next, it must not be drawn off its own screen.
        s.clampScroll(s.metrics(v))
        assertEquals("the week must be back on screen", 0f, s.dayX, 0.01f)
    }

    @Test
    fun a_week_with_fewer_all_day_lanes_cannot_leave_the_day_scrolled_past_midnight() {
        // The same bug wearing a different hat. Scroll to the bottom of a week whose banner is three
        // lanes tall, then swipe to a week with none: the banner's rows go back to the grid, the grid
        // gets taller, `maxScrollY` shrinks, and a scroll offset nobody re-clamped is now past the
        // end of the day, showing a strip of nothing below midnight.
        val s = state(hours = 12)
        val busy = viewport(lanes = 3)
        s.panY(-99_999f, s.metrics(busy)) // hard against the bottom of the day
        val bottom = s.scrollY
        assertEquals(s.metrics(busy).maxScrollY, bottom, 0.01f)

        val quiet = viewport(lanes = 0)
        assertTrue(
            "a week with no banner has more grid, so less to scroll",
            s.metrics(quiet).maxScrollY < bottom,
        )
        s.clampScroll(s.metrics(quiet))
        assertEquals(s.metrics(quiet).maxScrollY, s.scrollY, 0.01f)
    }

    @Test
    fun expanding_the_all_day_banner_costs_the_grid_its_room() {
        // The banner grows downwards into the grid, so the hours on screen shrink and the scroll has
        // further to go. An hour does NOT change height, the horizon is measured against the whole
        // surface, so a busier week cannot silently rescale the grid.
        val s = state(hours = 12)
        val lanes = 5
        val collapsed = s.metrics(viewport(lanes))
        s.toggleBanner()
        val expanded = s.metrics(viewport(lanes))

        assertEquals("an hour must not change height", collapsed.hourHeight, expanded.hourHeight, 0.01f)
        assertTrue("the banner must grow", expanded.bannerHeight > collapsed.bannerHeight)
        assertTrue("and the grid must give up the room", expanded.contentHeight < collapsed.contentHeight)
        assertTrue("so there is more day to scroll through", expanded.maxScrollY > collapsed.maxScrollY)
    }

    @Test
    fun the_collapsed_banner_never_grows_past_its_cap_however_busy_the_week() {
        val s = state()
        // Three lanes fit with no overflow row; twenty do not, and the banner still stops at three.
        assertEquals(3, s.metrics(viewport(lanes = 3)).bannerLanes)
        assertEquals(ALL_DAY_COLLAPSED_LANES, s.metrics(viewport(lanes = 20)).bannerLanes)
        s.toggleBanner()
        assertEquals("expanded, it shows every lane", 20, s.metrics(viewport(lanes = 20)).bannerLanes)
    }
}
