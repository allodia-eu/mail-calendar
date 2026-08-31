// The arithmetic half of dragging on the grid: given a point and a page, what did the hold mean,
// and where would the drop leave the block? Every one of these is a pure function, pinned here
// without composing anything, the same split the whole surface is built on (CalendarSurfaceState).
// The gesture-race half (the long press against the rest of the pointer stream) is
// CalendarDragGestureTest.kt; the settled-drag preview and the args a drop sends to the core are
// CalendarDragPreviewTest.kt and CalendarDragArgsTest.kt.
package eu.allodia.mailcal

import androidx.compose.ui.geometry.Offset
import java.time.LocalDate
import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Test

private val ANCHOR: LocalDate = LocalDate.of(2026, 7, 6)

/** A geometry with round numbers: an hour is 60px tall and a day 100px wide, so a pixel is a minute. */
private fun metrics(): SurfaceMetrics = SurfaceMetrics(
    viewport = SurfaceViewport(
        width = 752f,
        height = 800f,
        gutter = 52f,
        headerHeight = 56f,
        laneHeight = 24f,
        dividerHeight = 1f,
        lanes = 0,
    ),
    hourHeight = 60f,
    dayWidth = 100f,
    bannerLanes = 0,
)

private fun block(
    day: Int = 1,
    startMinutes: Int = 9 * 60,
    endMinutes: Int = 10 * 60,
    canMove: Boolean = true,
    occurrenceStart: String = "",
    clipped: Boolean = false,
) = BlockPaint(
    account = "acct",
    event = "evt",
    day = day,
    column = 0,
    columns = 1,
    startMinutes = startMinutes,
    endMinutes = endMinutes,
    title = "Standup",
    clock = "09:00",
    spoken = "Standup",
    background = androidx.compose.ui.graphics.Color.Blue,
    border = androidx.compose.ui.graphics.Color.Blue,
    titleStyle = androidx.compose.ui.text.TextStyle.Default,
    clockStyle = androidx.compose.ui.text.TextStyle.Default,
    awaitingResponse = false,
    canMove = canMove,
    occurrenceStart = occurrenceStart,
    clipped = clipped,
)

private fun page(blocks: List<BlockPaint> = listOf(block())) = PagePaint(
    headings = (0..6).map {
        val date = ANCHOR.plusDays(it.toLong())
        DayHeading(date, "Mon", "${date.dayOfMonth}")
    },
    weekNumber = 28,
    weekSpoken = "Week 28",
    blocks = blocks,
    bands = emptyList(),
    lanes = 0,
    hiddenPerDay = List(7) { 0 },
    moreLabels = List(7) { "" },
    moreSpoken = List(7) { "" },
    isMaterialized = true,
)

/** A surface point inside day column [day] at wall-clock [minute], with the grid unscrolled. */
private fun at(m: SurfaceMetrics, day: Int, minute: Int): Offset = Offset(
    x = m.gutter + m.dayWidth * day + m.dayWidth / 2f,
    y = m.contentTop + m.hourHeight * (minute / 60f),
)

private fun hold(
    page: PagePaint,
    m: SurfaceMetrics = metrics(),
    point: Offset,
    resizeEdge: Float = 14f,
    canCreate: Boolean = true,
) = page.dragAt(point, m, dayX = 0f, scrollY = 0f, resizeEdge = resizeEdge, canCreate = canCreate)

class CalendarDragDecisionTest {
    @Test
    fun a_hold_in_the_middle_of_our_own_block_moves_it() {
        val m = metrics()
        val drag = hold(page(), m, at(m, day = 1, minute = 9 * 60 + 30))
        assertEquals(DragKind.MOVE, drag?.kind)
        assertEquals("evt", drag?.subject?.event)
    }

    @Test
    fun a_hold_near_an_edge_resizes_that_edge_and_anchors_on_it() {
        // Anchoring on the *edge* rather than on the finger is what stops the first frame jumping
        // the edge to wherever inside the grab zone the finger happened to land.
        val m = metrics()
        val top = hold(page(), m, Offset(at(m, 1, 9 * 60).x, m.contentTop + 60f * 9 + 5f))
        assertEquals(DragKind.RESIZE_START, top?.kind)
        assertEquals(9 * 60, top?.anchorMinute)

        val bottom = hold(page(), m, Offset(at(m, 1, 10 * 60).x, m.contentTop + 60f * 10 - 5f))
        assertEquals(DragKind.RESIZE_END, bottom?.kind)
        assertEquals(10 * 60, bottom?.anchorMinute)
    }

    @Test
    fun a_block_too_short_for_two_grab_zones_is_always_a_move() {
        // A quarter-hour block at this zoom is 15px tall and the zones are 14px each: applied
        // literally, every hold on it would be a resize of something whose middle you cannot reach.
        val m = metrics()
        val short = page(listOf(block(startMinutes = 9 * 60, endMinutes = 9 * 60 + 15)))
        val drag = hold(short, m, Offset(at(m, 1, 9 * 60).x, m.contentTop + 60f * 9 + 2f))
        assertEquals(DragKind.MOVE, drag?.kind)
    }

    @Test
    fun a_meeting_we_do_not_own_is_not_picked_up() {
        // The core's answer, not ours: `canMove` is narrower than "the calendar is writable". A hold
        // on somebody else's meeting falls through to a create, exactly as a hold on bare grid does
        // doing nothing at all would read as the app having missed the gesture.
        val m = metrics()
        val theirs = page(listOf(block(canMove = false)))
        val drag = hold(theirs, m, at(m, day = 1, minute = 9 * 60 + 30))
        assertEquals(DragKind.CREATE, drag?.kind)
        assertNull(drag?.subject)
    }

    @Test
    fun a_segment_clipped_by_midnight_is_not_picked_up() {
        // Its visible rectangle is a clip of the event, not the event: every gesture on it would
        // mean something other than what it looks like.
        val m = metrics()
        val overnight = page(listOf(block(startMinutes = 0, endMinutes = 8 * 60, clipped = true)))
        val drag = hold(overnight, m, at(m, day = 1, minute = 4 * 60))
        assertEquals(DragKind.CREATE, drag?.kind)
    }

    @Test
    fun a_hold_creates_nothing_when_no_calendar_can_be_written() {
        // The same gate the "New event" button is disabled by. Drawing out a slot that can never be
        // filed anywhere is an affordance that cannot fire.
        val m = metrics()
        assertNull(hold(page(emptyList()), m, at(m, 3, 14 * 60), canCreate = false))
    }

    @Test
    fun a_hold_above_the_grid_or_on_the_ruler_is_not_a_drag() {
        // Everything above `contentTop` is chrome, the headings and the all-day banner, whose bars
        // are not draggable in this release, and so is the hour ruler down the left.
        val m = metrics()
        assertNull("the day headings", hold(page(), m, Offset(200f, m.contentTop - 2f)))
        assertNull("the hour ruler", hold(page(), m, Offset(10f, m.contentTop + 100f)))
    }

    @Test
    fun a_point_past_the_last_column_is_not_a_drag() {
        val m = metrics()
        assertNull(hold(page(), m, Offset(m.gutter + m.dayWidth * 9, m.contentTop + 100f)))
    }

    @Test
    fun minutes_snap_to_the_quarter_hour() {
        val m = metrics()
        assertEquals(9 * 60, m.minuteAt(60f * 9 + 4f))
        assertEquals(9 * 60 + 15, m.minuteAt(60f * 9 + 12f))
    }
}
