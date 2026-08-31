// Fast flicks, against a grid that is still moving.
//
// **This is the file that would have caught the swallowed swipe, and no other kind of test could
// have.** The bug needed a gesture to arrive while the *previous gesture's animation was still
// running*, and every test we had, and every synthetic swipe a script can inject over adb, politely
// waited for the grid to settle first. It reproduced only in a hand, moving fast. Eight flicks turned
// three weeks, and the unit tests were green.
//
// So the clock is taken away from Compose (`autoAdvance = false`) and handed to the test, and the next
// flick is delivered ONE FRAME after the last one, with the page turn still mid-slide, which is
// precisely the state that used to eat it. That is the whole trick, and it makes a gesture/animation
// race a deterministic, millisecond-exact JVM test that gates every PR.
package eu.allodia.mailcal

import androidx.compose.ui.test.junit4.v2.createComposeRule
import androidx.compose.ui.test.onRoot
import androidx.compose.ui.test.performTouchInput
import androidx.compose.ui.test.swipeLeft
import androidx.compose.ui.test.swipeRight
import java.time.LocalDate
import java.util.Locale
import org.junit.Assert.assertEquals
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import uniffi.mailcal_bindings.CalendarPage
import uniffi.mailcal_bindings.GridDay
import uniffi.mailcal_bindings.Swatch

/** A Monday. Every page below is the seven days from its own anchor, so a week is a real week. */
private val ANCHOR: LocalDate = LocalDate.of(2026, 7, 6)
private val TODAY: LocalDate = LocalDate.of(2026, 7, 6)

/** One frame at 60Hz, the smallest slice of time a flick can be separated from the last one by. */
private const val ONE_FRAME_MS = 16L

/** Any calendar swatch: these tests assert on gestures, not on which colour a slot wears. */
private val TEST_SWATCH = Swatch("#2f6fa8", "#ffffff", "#245782")

@RunWith(RobolectricTestRunner::class)
class CalendarFlickTest {
    @get:Rule val compose = createComposeRule()

    private fun weekPage(from: LocalDate) = CalendarPage(
        days = (0..6).map { GridDay(from.plusDays(it.toLong()).toString()) },
        timed = emptyList(),
        allDay = emptyList(),
        allDayLanes = 0u,
        timezone = "Europe/Amsterdam",
        calendars = emptyList(),
        isMaterialized = true,
    )

    /** The grid, at the whole-week zoom, where the day strip has nowhere to go and every sideways
     *  drag reaches the week. */
    private fun grid(state: CalendarSurfaceState) {
        compose.setContent {
            AppTheme {
                CalendarSurface(
                    state = state,
                    pageFor = { from, _ -> weekPage(from) },
                    anchorFor = { week -> ANCHOR.plusWeeks(week.toLong()) },
                    origin = ANCHOR,
                    calendarVersion = 0,
                    today = TODAY,
                    weekStart = ANCHOR,
                    nowMinutes = 9 * 60,
                    use24Hour = true,
                    locale = Locale.ENGLISH,
                    mode = CalendarMode.WEEK,
                    recentreToken = 0,
                    onZoomSettled = {},
                    onOpenEvent = { },
                    canCreateEvent = false,
                    createSwatch = TEST_SWATCH,
                    onDrop = {},
                )
            }
        }
        compose.mainClock.advanceTimeBy(100) // laid out and drawn once
    }

    /** A flick: fast, short, and released, the gesture nearly every real page turn is made of. */
    private fun flickLeft() = compose.onRoot().performTouchInput {
        swipeLeft(startX = right * 0.8f, endX = right * 0.2f, durationMillis = 50)
    }

    private fun flickRight() = compose.onRoot().performTouchInput {
        swipeRight(startX = right * 0.2f, endX = right * 0.8f, durationMillis = 50)
    }

    private fun settle() {
        compose.mainClock.autoAdvance = true
        compose.waitForIdle()
        compose.mainClock.advanceTimeBy(1_000)
    }

    @Test
    fun every_flick_turns_a_week_even_while_the_last_one_is_still_sliding() {
        // THE regression. Measured on a real phone before the fix: eight fast flicks turned THREE
        // weeks. The swipe was never dropped, the *decision* was. A turn was only committed when its
        // slide finished, so a turn still being drawn had, as far as the state knew, never happened:
        // flick again and the new gesture cancelled the animation, and with it a week already won.
        // Its progress survived only as a partial page offset, which is capped at one page, so two
        // flicks could never add up to more than one week.
        val state = CalendarSurfaceState(visibleHours = 12, visibleDays = DAYS_IN_WEEK)
        compose.mainClock.autoAdvance = false
        grid(state)

        repeat(8) {
            flickLeft()
            // ONE frame. The page turn is still mid-slide when the next finger lands, which is exactly
            // the condition that used to swallow it, and exactly what a script over adb cannot do.
            compose.mainClock.advanceTimeBy(ONE_FRAME_MS)
        }
        settle()

        assertEquals("a flick was swallowed by the one before it", 8, state.week)
    }

    @Test
    fun a_flick_the_other_way_still_counts_while_the_grid_is_moving() {
        // The same race, reversed halfway: the direction of a flick is decided by that flick's own
        // finger, never by where the page happens to be sitting, because a page mid-catch-up carries
        // a *lag*, and a lag looks exactly like a drag the other way.
        val state = CalendarSurfaceState(visibleHours = 12, visibleDays = DAYS_IN_WEEK)
        compose.mainClock.autoAdvance = false
        grid(state)

        repeat(4) {
            flickLeft()
            compose.mainClock.advanceTimeBy(ONE_FRAME_MS)
        }
        repeat(3) {
            flickRight()
            compose.mainClock.advanceTimeBy(ONE_FRAME_MS)
        }
        settle()

        assertEquals("four on, three back, every one of them counted", 1, state.week)
    }

    @Test
    fun however_hard_it_is_flicked_the_grid_comes_to_rest_on_a_week() {
        // **Between two weeks is never a resting place** (§6). Whatever the hand did, when everything
        // stops the page offset is zero, a week, whole, on the glass.
        val state = CalendarSurfaceState(visibleHours = 12, visibleDays = DAYS_IN_WEEK)
        compose.mainClock.autoAdvance = false
        grid(state)

        repeat(6) {
            flickLeft()
            compose.mainClock.advanceTimeBy(ONE_FRAME_MS)
        }
        flickRight()
        settle()

        assertEquals(
            "the grid came to rest between two weeks",
            0f,
            state.pageOffset,
            0.5f,
        )
    }

    @Test
    fun the_pixels_never_fall_further_behind_than_the_grid_can_draw() {
        // The lag is what lets flicks accumulate, and it is bounded for a reason: at a lag of `f`
        // pages the grid is drawing pages (-1-f)..(1-f), so it is sliding THROUGH weeks to reach the
        // one it has landed on. Let it lag past what it holds and it draws a hole where a week should
        // be. Flick far faster than it can ever catch up, and the offset still stays inside the range
        // the live pages cover.
        val state = CalendarSurfaceState(visibleHours = 12, visibleDays = DAYS_IN_WEEK)
        compose.mainClock.autoAdvance = false
        grid(state)

        repeat(20) {
            flickLeft()
            compose.mainClock.advanceTimeBy(1) // barely a millisecond: nothing has time to catch up
            val width = compose.onRoot().fetchSemanticsNode().size.width.toFloat()
            assert(kotlin.math.abs(state.pageOffset) <= MAX_PAGE_LAG * width + 1f) {
                "the pixels lagged ${state.pageOffset}px, further than the live pages can draw"
            }
        }
        settle()

        assertEquals("every flick still landed a week", 20, state.week)
    }
}
