// What a settled drag sends to the core, split out of CalendarDragTest.kt: a move is a delta not a
// destination, each resize names its own edge, and only a repeating event is asked about the
// series.
package eu.allodia.mailcal

import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test
import uniffi.mailcal_bindings.EventEdge

class CalendarDragArgsTest {
    private val once = DragSubject("acct", "evt", "", 1, 540, 600)
    private val weekly = DragSubject("acct", "evt", "2026-07-07T09:00:00", 1, 540, 600)

    @Test
    fun a_move_sends_a_delta_not_a_destination() {
        val args = CalendarDragState(DragKind.MOVE, once, 1, 540, 2, 570).moveArgs(false)
        assertEquals(EventEdge.WHOLE, args?.edge)
        assertEquals(1, args?.days)
        assertEquals(30, args?.minutes)
    }

    @Test
    fun each_resize_names_its_own_edge() {
        assertEquals(
            EventEdge.START,
            CalendarDragState(DragKind.RESIZE_START, once, 1, 540, 1, 525).moveArgs(false)?.edge,
        )
        assertEquals(
            EventEdge.END,
            CalendarDragState(DragKind.RESIZE_END, once, 1, 600, 1, 630).moveArgs(false)?.edge,
        )
    }

    @Test
    fun a_repeating_event_is_asked_about_and_a_one_off_is_not() {
        assertTrue(CalendarDragState(DragKind.MOVE, weekly, 1, 540, 2, 540).asksAboutTheSeries())
        assertTrue(!CalendarDragState(DragKind.MOVE, once, 1, 540, 2, 540).asksAboutTheSeries())
    }

    @Test
    fun this_event_names_the_occurrence_and_all_events_names_none() {
        val drag = CalendarDragState(DragKind.MOVE, weekly, 1, 540, 2, 540)
        assertEquals("2026-07-07T09:00:00", drag.moveArgs(thisOccurrenceOnly = true)?.occurrence)
        assertNull(
            "the whole series is named by sending no occurrence at all",
            drag.moveArgs(thisOccurrenceOnly = false)?.occurrence,
        )
    }

    @Test
    fun a_one_off_never_names_an_occurrence_whatever_it_is_asked() {
        val drag = CalendarDragState(DragKind.MOVE, once, 1, 540, 2, 540)
        assertNull(drag.moveArgs(thisOccurrenceOnly = true)?.occurrence)
    }

    @Test
    fun a_create_asks_for_no_move_at_all() {
        assertNull(CalendarDragState(DragKind.CREATE, null, 1, 540, 1, 600).moveArgs(false))
    }
}
