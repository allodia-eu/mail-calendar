// The long press against the rest of the pointer stream, split out of CalendarDragTest.kt.
package eu.allodia.mailcal

import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.test.junit4.v2.createComposeRule
import androidx.compose.ui.test.onRoot
import androidx.compose.ui.test.performTouchInput
import androidx.compose.ui.test.swipeLeft
import java.time.LocalDate
import java.util.Locale
import org.junit.Assert.assertEquals
import org.junit.Assert.assertNotNull
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import uniffi.mailcal_bindings.CalendarPage
import uniffi.mailcal_bindings.GridDay
import uniffi.mailcal_bindings.Swatch

private val ANCHOR: LocalDate = LocalDate.of(2026, 7, 6)

/** One frame at 60Hz. */
private const val ONE_FRAME_MS = 16L

/** Any calendar swatch: these tests assert on gestures, not on which colour a slot wears. */
private val TEST_SWATCH = Swatch("#2f6fa8", "#ffffff", "#245782")

/**
 * The long press against the rest of the pointer stream.
 *
 * A long press is a **timeout**, sharing one finger with a pan, a page turn and a pinch, which is
 * §6's four-handlers-one-finger arrangement waiting to happen. These are the cases a hand finds:
 * a swipe that must not become a drag, and a drag that must not wedge the week.
 */
@RunWith(RobolectricTestRunner::class)
class CalendarDragGestureTest {
    @get:Rule val compose = createComposeRule()

    private var dropped: CalendarDragState? = null

    private fun weekPage(from: LocalDate) = CalendarPage(
        days = (0..6).map { GridDay(from.plusDays(it.toLong()).toString()) },
        timed = emptyList(),
        allDay = emptyList(),
        allDayLanes = 0u,
        timezone = "Europe/Amsterdam",
        calendars = emptyList(),
        isMaterialized = true,
    )

    private fun grid(state: CalendarSurfaceState) {
        compose.setContent {
            AppTheme {
                CalendarSurface(
                    state = state,
                    pageFor = { from, _ -> weekPage(from) },
                    anchorFor = { week -> ANCHOR.plusWeeks(week.toLong()) },
                    origin = ANCHOR,
                    calendarVersion = 0,
                    today = ANCHOR,
                    weekStart = ANCHOR,
                    nowMinutes = 9 * 60,
                    use24Hour = true,
                    locale = Locale.ENGLISH,
                    mode = CalendarMode.WEEK,
                    recentreToken = 0,
                    onZoomSettled = {},
                    onOpenEvent = { },
                    canCreateEvent = true,
                    createSwatch = TEST_SWATCH,
                    onDrop = { dropped = it },
                )
            }
        }
        compose.mainClock.advanceTimeBy(100)
    }

    @Test
    fun a_swipe_still_turns_the_week_and_creates_nothing() {
        // The regression the long press could most easily cause: a flick is a press followed by
        // movement, and a hold detector that fires on the press half would take every swipe.
        val state = CalendarSurfaceState(visibleHours = 12, visibleDays = DAYS_IN_WEEK)
        compose.mainClock.autoAdvance = false
        grid(state)

        repeat(3) {
            compose.onRoot().performTouchInput {
                swipeLeft(startX = right * 0.8f, endX = right * 0.2f, durationMillis = 50)
            }
            compose.mainClock.advanceTimeBy(ONE_FRAME_MS)
        }
        compose.mainClock.autoAdvance = true
        compose.waitForIdle()
        compose.mainClock.advanceTimeBy(1_000)

        assertEquals("a swipe was eaten by the press-and-hold", 3, state.week)
        assertNull("a swipe drew out an event", dropped)
    }

    @Test
    fun a_slow_drag_is_still_a_pan_not_a_hold() {
        // A finger that takes its time getting going is still panning. The hold window is measured
        // off the *pointer* clock rather than restarted per event, precisely so a jittery finger
        // still eventually decides one way or the other rather than never firing at all.
        val state = CalendarSurfaceState(visibleHours = 12, visibleDays = DAYS_IN_WEEK)
        compose.mainClock.autoAdvance = false
        grid(state)

        compose.onRoot().performTouchInput {
            down(center)
            repeat(20) { step ->
                advanceEventTime(20)
                moveTo(Offset(center.x, center.y - (step + 1) * 12f))
            }
            up()
        }
        compose.mainClock.autoAdvance = true
        compose.waitForIdle()

        assertNull("a pan was read as a press-and-hold", dropped)
        assertTrue("the hours did not scroll", state.scrollY > 0f)
    }

    @Test
    fun two_fingers_resting_before_a_slow_pinch_do_not_begin_a_drag() {
        // A pinch is two fingers that spread, and people spread them *slowly*. Between the second
        // finger landing and the spread becoming a pinch, the gesture is undecided and perfectly
        // still, which is indistinguishable from a press-and-hold unless a second contact is made to
        // end the hold's candidacy outright. Get this wrong and every unhurried zoom drags an event
        // out from under the fingers first.
        val state = CalendarSurfaceState(visibleHours = 12, visibleDays = DAYS_IN_WEEK)
        compose.mainClock.autoAdvance = false
        grid(state)

        compose.onRoot().performTouchInput {
            down(0, Offset(center.x - 40f, center.y))
            down(1, Offset(center.x + 40f, center.y))
            advanceEventTime(700)
            moveTo(0, Offset(center.x - 40f, center.y))
            advanceEventTime(50)
            moveTo(0, Offset(center.x - 200f, center.y))
            moveTo(1, Offset(center.x + 200f, center.y))
            up(0)
            up(1)
        }
        compose.mainClock.autoAdvance = true
        compose.waitForIdle()

        assertNull("a slow pinch was read as a press-and-hold", dropped)
        assertNull(state.drag)
    }

    @Test
    fun a_press_and_hold_on_empty_grid_draws_out_a_slot() {
        val state = CalendarSurfaceState(visibleHours = 12, visibleDays = DAYS_IN_WEEK)
        compose.mainClock.autoAdvance = false
        grid(state)

        compose.onRoot().performTouchInput {
            down(center)
            // Past the long-press window without moving: the grid takes hold.
            advanceEventTime(700)
            moveTo(center)
            advanceEventTime(50)
            moveTo(Offset(center.x, center.y + 60f))
            up()
        }
        compose.mainClock.autoAdvance = true
        compose.waitForIdle()

        assertNotNull("the hold drew out nothing", dropped)
        assertEquals(DragKind.CREATE, dropped?.kind)
        assertEquals("the grid must not have paged under the drag", 0, state.week)
    }

    @Test
    fun the_week_still_turns_after_a_drag_has_been_dropped() {
        // The drag runs inside the one gesture owner. If it left the loop in a state that no longer
        // arbitrates, the failure mode §6 is a whole essay about, this is where it shows.
        val state = CalendarSurfaceState(visibleHours = 12, visibleDays = DAYS_IN_WEEK)
        compose.mainClock.autoAdvance = false
        grid(state)

        compose.onRoot().performTouchInput {
            down(center)
            advanceEventTime(700)
            moveTo(Offset(center.x, center.y + 60f))
            up()
        }
        compose.mainClock.advanceTimeBy(ONE_FRAME_MS)
        compose.onRoot().performTouchInput {
            swipeLeft(startX = right * 0.8f, endX = right * 0.2f, durationMillis = 50)
        }
        compose.mainClock.autoAdvance = true
        compose.waitForIdle()
        compose.mainClock.advanceTimeBy(1_000)

        assertEquals("the gesture owner stopped arbitrating after a drop", 1, state.week)
        assertNull("nothing may still be held once the finger has lifted", state.drag)
    }
}
