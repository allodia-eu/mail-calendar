// Pinch-to-zoom: how much of the day, and how many of the week's days, the grid shows at once.
//
// The model is Samsung's, and it is the one that never jumps: **a page is a week**. That week is the
// hard boundary for horizontal scrolling, you cannot scroll out of it, you page to the next one.
// Zooming horizontally does not switch to a different *view*; it just narrows the columns, and you
// scroll among the same seven days.
//
// That is why the earlier design felt broken. Snapping a pinch to a Monday-aligned "week view"
// CANNOT keep an arbitrary three-day window on screen, a user reading Sunday, Monday and Tuesday
// who pinched outwards was shown the previous Monday-to-Sunday, and two of the three days they were
// reading vanished. Here the days never move; only their width does.
//
// The horizon (hours) and the column count are both persisted as core settings, so all three clients
// open the same way. But neither an hour nor a day has a *size* until this client multiplies, the
// core's geometry is unit-free, so the zoom itself lives here, and only the settled values go back.
package eu.allodia.mailcal

import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableFloatStateOf
import androidx.compose.runtime.setValue
import kotlin.math.roundToInt

// The same clamps the core enforces (mailcal-account: MIN/MAX_VISIBLE_HOURS). Held here too so the
// live gesture is bounded as it happens, rather than snapping back when the core rejects it.
internal const val MIN_VISIBLE_HOURS = 4f
internal const val MAX_VISIBLE_HOURS = 24f

/** The days in a page. A page is a week, the boundary a horizontal scroll cannot cross. */
internal const val DAYS_IN_WEEK = 7

/** You can zoom down to a single day, and out to the whole week. Never further: the week is the page. */
internal const val MIN_VISIBLE_DAYS = 1f
internal const val MAX_VISIBLE_DAYS = DAYS_IN_WEEK.toFloat()

/**
 * How much of the day, and how many of the week's days, are on screen, and what a pinch does to
 * each.
 *
 * Both are **fractional**. A pinch is continuous, and rounding mid-gesture would make the grid jump
 * between whole hours (or whole columns) instead of tracking the fingers, which is the difference
 * between "buttery" and "broken". Only the settled values are whole, and only those escape.
 */
internal class CalendarZoom(visibleHours: Int, visibleDays: Int = 3) {
    /** Hours of the day currently on screen. */
    var visibleHours by mutableFloatStateOf(
        visibleHours.toFloat().coerceIn(MIN_VISIBLE_HOURS, MAX_VISIBLE_HOURS),
    )
        private set

    /** Day columns of the week currently on screen. Fractional: half a column may be showing. */
    var visibleDays by mutableFloatStateOf(
        visibleDays.toFloat().coerceIn(MIN_VISIBLE_DAYS, MAX_VISIBLE_DAYS),
    )
        private set

    /**
     * Applies one frame of a vertical pinch, and returns **the factor the hour height actually grew
     * by**, which is not the factor asked for once the zoom hits its clamp.
     *
     * The caller needs the real one: it corrects the scroll offset by exactly this, to keep the
     * content under the fingers still. Correcting by the *requested* factor at the end of the range
     * would drag the grid on every further frame of a pinch that has nowhere left to go.
     *
     * `zoom > 1` is fingers spreading apart, which must show *fewer* hours (zoom in), so it divides.
     * Get that backwards and the grid zooms out when the user pinches in, which feels broken
     * instantly.
     */
    fun pinchVertical(zoom: Float): Float {
        if (zoom <= 0f) return 1f
        val before = visibleHours
        visibleHours = (visibleHours / zoom).coerceIn(MIN_VISIBLE_HOURS, MAX_VISIBLE_HOURS)
        // An hour got taller by exactly as much as the horizon got shorter.
        return before / visibleHours
    }

    /** The same, for the day axis: spreading sideways shows fewer, wider days. */
    fun pinchHorizontal(zoom: Float): Float {
        if (zoom <= 0f) return 1f
        val before = visibleDays
        visibleDays = (visibleDays / zoom).coerceIn(MIN_VISIBLE_DAYS, MAX_VISIBLE_DAYS)
        return before / visibleDays
    }

    /** The whole-hour horizon to persist once the fingers lift. */
    fun settledHours(): Int = visibleHours.roundToInt()
        .coerceIn(MIN_VISIBLE_HOURS.toInt(), MAX_VISIBLE_HOURS.toInt())

    /** The whole-column count to persist once the fingers lift. */
    fun settledDays(): Int = visibleDays.roundToInt()
        .coerceIn(MIN_VISIBLE_DAYS.toInt(), MAX_VISIBLE_DAYS.toInt())

    /** Re-seeds the horizon (on load, or when the settings screen changes it). */
    fun resetHours(hours: Int) {
        visibleHours = hours.toFloat().coerceIn(MIN_VISIBLE_HOURS, MAX_VISIBLE_HOURS)
    }

    /** Re-seeds the day axis (on load, or when a view is picked from the menu). */
    fun resetDays(days: Int) {
        visibleDays = days.toFloat().coerceIn(MIN_VISIBLE_DAYS, MAX_VISIBLE_DAYS)
    }

    /**
     * Snaps the day axis to the **zoom level** the pinch settled on, once the fingers lift.
     *
     * **A column count that is not a rung is not a cosmetic imperfection, it breaks the swipe.**
     * The page holds all seven days at a width of `viewport / visibleDays`, so any count that does
     * not divide the week evenly leaves part of it hanging off the screen. That overhang is a
     * horizontal scroll nested inside the pager, and a nested scroll consumes the drag *first*: the
     * swipe that should turn the week is spent sliding along the current one, and the grid comes to
     * rest between two weeks.
     *
     * It snaps to the settled **mode's** columns, not to [settledDays], and the difference is the
     * bug. A pinch outwards from the week lands on ~6.4 columns; that rounds to *6*, while the mode
     * it maps to is the whole WEEK, of *7*. So the grid would draw seven columns at one-sixth of the
     * viewport each, and the mode and the width would disagree by a whole column, an overhang, and
     * a swallowed swipe, in the one view that is supposed to have neither.
     *
     * Mid-gesture the count stays fractional: rounding while the fingers are still moving is what
     * makes a pinch stutter instead of track.
     */
    fun settleDays() {
        resetDays(modeForColumns(settledDays()).columns)
    }

    /**
     * How tall one hour is, in pixels, given the height of the grid's viewport.
     *
     * The bridge from the core's unit-free geometry to pixels: every block's vertical offset and
     * height is a multiple of it. Pixels rather than `Dp` because the renderer is a canvas, a
     * `DrawScope` works in pixels, and converting per block per frame is a cost that buys nothing.
     */
    fun hourHeight(viewport: Float): Float = viewport / visibleHours

    /** How wide one day column is, given the width of the grid's viewport. The horizontal twin. */
    fun dayWidth(viewport: Float): Float = viewport / visibleDays
}
