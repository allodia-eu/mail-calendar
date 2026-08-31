// What a settled drag shows on screen, split out of CalendarDragTest.kt: the preview the block
// paints while the finger is still down, clamped inside the day/week it began in.
package eu.allodia.mailcal

import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Test

class CalendarDragPreviewTest {
    private fun move(subject: DragSubject, dayDelta: Int, minuteDelta: Int) = CalendarDragState(
        kind = DragKind.MOVE,
        subject = subject,
        anchorDay = subject.day,
        anchorMinute = subject.startMinutes,
        day = subject.day + dayDelta,
        minute = subject.startMinutes + minuteDelta,
    ).clampedTo(DAYS_IN_WEEK)

    private val standup = DragSubject("acct", "evt", "", day = 1, startMinutes = 540, endMinutes = 600)

    @Test
    fun a_move_carries_both_edges_by_the_same_amount() {
        val preview = move(standup, dayDelta = 1, minuteDelta = 30).preview()
        assertEquals(2, preview.day)
        assertEquals(570, preview.startMinutes)
        assertEquals(630, preview.endMinutes)
        assertEquals("the duration must survive a move exactly", 60, preview.minutes)
    }

    @Test
    fun a_move_is_clamped_inside_its_own_day() {
        // What you see is what you get: an event dragged to the top of the screen stops at 00:00
        // rather than silently landing on the previous day. To change the day you drag sideways:
        // which is the one thing the preview can actually show.
        assertEquals(0, move(standup, 0, -900).preview().startMinutes)
        assertEquals(DAY_MINUTES, move(standup, 0, 900).preview().endMinutes)
    }

    @Test
    fun a_move_is_clamped_inside_the_week() {
        assertEquals(0, move(standup, -5, 0).preview().day)
        assertEquals(DAYS_IN_WEEK - 1, move(standup, 12, 0).preview().day)
    }

    @Test
    fun a_resize_moves_one_edge_and_cannot_pass_the_other() {
        val start = CalendarDragState(
            DragKind.RESIZE_START, standup, standup.day, 540, standup.day, 540 + 600,
        ).clampedTo(DAYS_IN_WEEK).preview()
        assertEquals("the end never moved", 600, start.endMinutes)
        assertEquals("clamped to the minimum, not refused", 600 - 15, start.startMinutes)

        val end = CalendarDragState(
            DragKind.RESIZE_END, standup, standup.day, 600, standup.day, 600 - 600,
        ).clampedTo(DAYS_IN_WEEK).preview()
        assertEquals("the start never moved", 540, end.startMinutes)
        assertEquals(540 + 15, end.endMinutes)
    }

    @Test
    fun a_hold_that_never_moved_draws_an_hour() {
        val create = CalendarDragState(DragKind.CREATE, null, 2, 600, 2, 600).preview()
        assertEquals(600, create.startMinutes)
        assertEquals(660, create.endMinutes)
    }

    @Test
    fun a_hold_that_was_dragged_draws_what_the_hand_described() {
        val create = CalendarDragState(DragKind.CREATE, null, 2, 600, 2, 600).movedTo(2, 690, 690).preview()
        assertEquals(600, create.startMinutes)
        assertEquals(690, create.endMinutes)
    }

    @Test
    fun a_slot_dragged_upwards_runs_from_the_finger_to_the_hour_it_began_in() {
        // Upwards the hand is setting the *start*. The end is the bottom of the band the press landed
        // in, 11:00, not the 10:00 the touch happened to be on. See CalendarDragFeelTest.
        val create = CalendarDragState(DragKind.CREATE, null, 2, 600, 2, 600).movedTo(2, 510, 510).preview()
        assertEquals(510, create.startMinutes)
        assertEquals(660, create.endMinutes)
    }

    @Test
    fun a_slot_stays_in_the_column_it_began_in() {
        // Widening a slot across days is not a thing an event can be, so a sideways wobble while
        // drawing one out must not silently file it on Wednesday.
        val create = CalendarDragState(DragKind.CREATE, null, 2, 600, 2, 600).movedTo(5, 690, 690).preview()
        assertEquals(2, create.day)
    }

    @Test
    fun a_hold_that_went_nowhere_writes_nothing() {
        val still = CalendarDragState(DragKind.MOVE, standup, 1, 540, 1, 540)
        assertTrue(!still.movesAnything())
        // ...but a create always does: the hold itself was the request.
        assertTrue(CalendarDragState(DragKind.CREATE, null, 1, 540, 1, 540).movesAnything())
    }
}
