// How a drag *feels*, as opposed to what it writes, the two rules that separate the two.
//
// A drag is still a delta and the delta is still snapped (CalendarDragTest pins that, and none of it
// moves here). What this file pins is the gap deliberately opened between the block on screen and the
// number that crosses the FFI:
//
//   - a press that went nowhere is "an event here", so its slot fills the **hour band the finger is
//     in** rather than the quarter-hour it happened to touch;
//   - while the finger is down the block follows it **between** snap steps, so the motion is smooth,
//     while the readout and the write stay on the grid.
//
// Both are picture-only. Every assertion below that touches `preview()` or `moveArgs()` is asserting
// that the picture did *not* leak into the write.
package eu.allodia.mailcal

import kotlin.math.abs
import kotlin.math.roundToInt
import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Test

/**
 * A create drag: the finger went down at [rawAnchor] and is now on [minute] (snapped) / [raw] (not).
 *
 * Built through `movedTo` rather than by naming the fields, because `drawn` latches there, a state
 * assembled by hand can hold a combination the gesture layer cannot actually produce. The anchor is
 * given as a *raw* touch and snapped here, exactly as `dragAt` does it.
 */
private fun create(rawAnchor: Int, minute: Int = snap(rawAnchor), raw: Int = minute) =
    CalendarDragState(
        kind = DragKind.CREATE,
        subject = null,
        anchorDay = 2,
        anchorMinute = snap(rawAnchor),
        day = 2,
        minute = snap(rawAnchor),
        rawMinute = rawAnchor,
        rawAnchorMinute = rawAnchor,
    ).movedTo(2, minute, raw)

private fun snap(raw: Int) = (raw.toFloat() / DRAG_SNAP_MINUTES).roundToInt() * DRAG_SNAP_MINUTES

private fun subject(start: Int, end: Int) = DragSubject(
    account = "a",
    event = "e",
    occurrenceStart = "",
    day = 2,
    startMinutes = start,
    endMinutes = end,
)

class CalendarDragHourPinTest {
    @Test
    fun a_press_fills_the_hour_it_landed_in() {
        // 10:15 is a touch, not a decision to meet at a quarter past.
        val early = create(rawAnchor = 615).preview()
        assertEquals(600, early.startMinutes)
        assertEquals(660, early.endMinutes)
    }

    @Test
    fun the_slot_always_contains_the_finger_that_asked_for_it() {
        // The nit this exists to hold, from a recording with touch points on: a press at 17:50 was
        // rounded to the *nearest* boundary and drew 18:00–19:00, the entire block below the finger.
        // Every minute of the day must land in a band that contains it, so walk them all.
        for (raw in 0 until DAY_MINUTES) {
            val slot = create(rawAnchor = raw).preview()
            assertTrue(
                "a press at $raw drew ${slot.startMinutes}..${slot.endMinutes}",
                raw >= slot.startMinutes && raw <= slot.endMinutes,
            )
            assertEquals("and it is a whole hour", DEFAULT_CREATE_MINUTES, slot.minutes)
        }
    }

    @Test
    fun a_touch_that_snaps_across_the_hour_still_fills_the_band_it_is_in() {
        // 16:53 snaps forward to 17:00. Pinning from the snapped minute would draw 17:00–18:00, which
        // is the band *below* the finger, the same defect by a different route.
        val slot = create(rawAnchor = 1013).preview()
        assertEquals(960, slot.startMinutes)
        assertEquals(1020, slot.endMinutes)
    }

    @Test
    fun a_wobble_under_one_snap_step_is_still_a_press() {
        // A hand is not a script: the finger drifts a few minutes before it lifts. That is the same
        // gesture, and it must not degrade into a ten-minute event at an unrounded time.
        val wobbled = create(rawAnchor = 615, minute = 615, raw = 623).preview()
        assertEquals(600, wobbled.startMinutes)
        assertEquals(660, wobbled.endMinutes)
    }

    @Test
    fun a_press_in_the_last_hour_of_the_day_keeps_its_whole_hour() {
        // 23:50 fills 23:00–24:00: the band that holds it, and the last one with room for an hour.
        val late = create(rawAnchor = 1430).preview()
        assertEquals(1380, late.startMinutes)
        assertEquals(1440, late.endMinutes)
    }

    @Test
    fun dragging_down_keeps_the_top_on_the_hour() {
        // The hand is setting the *end*; the start is the hour the press landed in, not the touch.
        val down = create(rawAnchor = 1250, minute = 1290).preview()
        assertEquals("20:00, not the 20:45 the finger was on", 1200, down.startMinutes)
        assertEquals(1290, down.endMinutes)
    }

    @Test
    fun dragging_up_keeps_the_bottom_on_the_hour() {
        // Mirror image: the hand is setting the *start*, and the end stays on the following hour.
        val up = create(rawAnchor = 1250, minute = 1140).preview()
        assertEquals(1140, up.startMinutes)
        assertEquals("21:00, the bottom of the band it began in", 1260, up.endMinutes)
    }

    @Test
    fun a_slot_is_never_shorter_than_the_hour_it_began_in() {
        // The cost of the rule above, stated out loud: the union of a band and a point inside it is
        // the band, so a drag cannot draw anything shorter than an hour. Shorter is the editor's job.
        for (finger in 1200..1260) {
            val slot = create(rawAnchor = 1250, minute = finger).preview()
            assertEquals(1200, slot.startMinutes)
            assertEquals(1260, slot.endMinutes)
        }
    }

    @Test
    fun the_slot_moves_continuously_all_the_way_through_the_gesture() {
        // The flicker report, generalised. Walk the finger across the whole day through the band it
        // started in and out the other side; consecutive frames may never jump. That covers the
        // press-to-drag transition and both crossings at once, which is why there is no mode flag
        // left to get wrong: a union has no threshold to flip at.
        var previous = create(rawAnchor = 1250, minute = 0, raw = 0).livePreview()
        for (finger in 1..DAY_MINUTES) {
            val slot = create(rawAnchor = 1250, minute = finger, raw = finger).livePreview()
            assertTrue(
                "start jumped at $finger: ${previous.startMinutes} -> ${slot.startMinutes}",
                abs(slot.startMinutes - previous.startMinutes) <= 1,
            )
            assertTrue(
                "end jumped at $finger: ${previous.endMinutes} -> ${slot.endMinutes}",
                abs(slot.endMinutes - previous.endMinutes) <= 1,
            )
            previous = slot
        }
    }
}

class CalendarDragSmoothTest {
    @Test
    fun the_live_block_moves_between_snap_steps_while_the_written_one_does_not() {
        // Finger is 7 minutes past the snapped quarter: the picture shows it, the write does not.
        val drag = create(rawAnchor = 600, minute = 690, raw = 697)
        assertEquals("the picture follows the finger", 697, drag.livePreview().endMinutes)
        assertEquals("the write stays on the grid", 690, drag.preview().endMinutes)
    }

    @Test
    fun the_anchored_edge_never_drifts_off_the_grid() {
        // Only the edge in the hand is smooth. The other one is where the write will put it, so it
        // must not wander by a few pixels while the user watches.
        val drag = create(rawAnchor = 600, minute = 690, raw = 697)
        assertEquals(600, drag.livePreview().startMinutes)
        assertEquals(600, drag.preview().startMinutes)
    }

    @Test
    fun a_press_that_went_nowhere_shows_its_pinned_hour_immediately() {
        // The pin is the whole point: it has to be on screen while the finger is still down, not
        // applied in a jump at the moment of release.
        val held = create(rawAnchor = 615, minute = 615, raw = 619).livePreview()
        assertEquals(600, held.startMinutes)
        assertEquals(660, held.endMinutes)
    }

    @Test
    fun a_move_carries_the_raw_delta_in_the_picture_and_the_snapped_one_in_the_write() {
        val moving = CalendarDragState(
            kind = DragKind.MOVE,
            subject = subject(600, 660),
            anchorDay = 2,
            anchorMinute = 600,
            day = 2,
            minute = 630,
            rawMinute = 637,
        )
        assertEquals(637, moving.livePreview().startMinutes)
        assertEquals(697, moving.livePreview().endMinutes)
        assertEquals(630, moving.preview().startMinutes)
        assertEquals(690, moving.preview().endMinutes)
        assertEquals(30, moving.moveArgs(thisOccurrenceOnly = false)?.minutes)
    }

    @Test
    fun a_resize_smooths_only_the_edge_in_the_hand() {
        val resizing = CalendarDragState(
            kind = DragKind.RESIZE_END,
            subject = subject(600, 660),
            anchorDay = 2,
            anchorMinute = 660,
            day = 2,
            minute = 720,
            rawMinute = 713,
        )
        assertEquals("the start is untouched", 600, resizing.livePreview().startMinutes)
        assertEquals("the dragged edge follows the finger", 713, resizing.livePreview().endMinutes)
        assertEquals("the write snaps", 720, resizing.preview().endMinutes)
    }

    @Test
    fun clamping_holds_the_smooth_edge_to_the_day_as_well() {
        // The picture may be smooth, but it may not show a block hanging off the end of the column:
        // it would be showing a write that cannot happen.
        val past = CalendarDragState(
            kind = DragKind.MOVE,
            subject = subject(1380, 1440),
            anchorDay = 2,
            anchorMinute = 1400,
            day = 2,
            minute = 1470,
            rawMinute = 1477,
        ).clampedTo(DAYS_IN_WEEK)
        assertEquals(1440, past.livePreview().endMinutes)
        assertEquals(1440, past.preview().endMinutes)
    }
}
