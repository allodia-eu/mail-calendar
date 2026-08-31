// One gesture, one owner.
//
// This is the file the calendar contract's §6 asks for. Every pointer event on the time grid arrives
// here, and here alone: no pager, no nested scrollables, no transform modifier reading the same
// stream behind our back. The handler decides, pan, page, or zoom, and drives all three itself.
//
// Because there is nobody to take the gesture *from*, this consumes freely, and that inverts the
// hard-won rule of the old design. Under the four-handler grid, consuming a pointer change did not
// politely ask the pager to stand aside: it CANCELLED its drag, and Compose's scrollable flings only
// when a drag *ends*, never when it is cancelled. No fling, no settle, and the grid sat between two
// weeks forever. Refusing to consume fixed the swipe and broke the zoom instead, the scrollers went
// on panning the grid around under a pinch, because nothing had told them not to. Both were symptoms
// of the same root, and the root was that four things were reading one finger.
//
// The invariant survives its bug, because it is worth more than the bug: **between two weeks is never
// a resting place.** Whatever ends a gesture, a lift, a cancel, a system dialog, the week lands.
package eu.allodia.mailcal

import androidx.compose.animation.core.AnimationState
import androidx.compose.animation.core.DecayAnimationSpec
import androidx.compose.animation.core.animateDecay
import androidx.compose.animation.core.animateTo
import androidx.compose.runtime.snapshots.Snapshot
import androidx.compose.foundation.gestures.awaitEachGesture
import androidx.compose.foundation.gestures.awaitFirstDown
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.input.pointer.PointerInputChange
import androidx.compose.ui.input.pointer.PointerInputScope
import androidx.compose.ui.input.pointer.util.VelocityTracker
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Job
import kotlinx.coroutines.launch
import kotlinx.coroutines.withTimeoutOrNull
import kotlin.math.abs

/**
 * What the finger turned out to be. Decided once, on the first movement that means anything, or on
 * the finger *not* moving for long enough, which is [GestureMode.DRAG].
 *
 * The long press has to be part of this enum rather than a `detectDragGesturesAfterLongPress`
 * alongside it, and that is §6 again in a new costume: two handlers reading one finger is the
 * arrangement this file was written to delete, and a long-press detector is a handler.
 */
private enum class GestureMode { UNDECIDED, PAN_DAYS, PAN_HOURS, ZOOM, DRAG }

/**
 * How far apart two fingers must be **on an axis** before that axis's scale means anything.
 *
 * This is what keeps the axes independent without forbidding diagonals. Fingers spread purely
 * sideways sit at almost the same height, so their vertical spread is a few noisy pixels, and
 * dividing by it would produce a wild factor and lurch the hours about while the user was only asking
 * for more days. Below this, an axis reports "no change" rather than a number it cannot know.
 *
 * Spread them at an angle, though, and *both* spreads are real, so both axes zoom, each by its own.
 */
private const val MIN_SPREAD_PX = 48f

/**
 * How far the fingers' spread must **change** before the gesture is a pinch and not a two-finger
 * swipe.
 *
 * Two fingers 200px apart that wobble by one pixel give a scale of 1.005, which is not 1. Without a
 * slop, *any* two-finger contact read as a pinch.
 */
private const val PINCH_SLOP_PX = 10f

/** Whether the fingers have spread or closed enough, on some axis, to mean a pinch and not a swipe. */
internal fun beginsPinch(startSpread: Offset, spread: Offset): Boolean =
    abs(spread.x - startSpread.x) > PINCH_SLOP_PX || abs(spread.y - startSpread.y) > PINCH_SLOP_PX

/** The fingers' distance apart on each axis. */
internal fun spreadOf(a: Offset, b: Offset): Offset = Offset(abs(a.x - b.x), abs(a.y - b.y))

/** One axis's scale, or exactly `1` when the fingers are too close together on it to know. */
internal fun axisScale(before: Float, after: Float): Float =
    if (before < MIN_SPREAD_PX || after < MIN_SPREAD_PX) 1f else after / before

/**
 * Every animation the grid runs, the flings, the page turns, and the snap that guarantees the week
 * always lands.
 *
 * All of them run in **their own job**, and this is not a detail. `animateTo` on a scroll takes a
 * mutex the next touch preempts, and a preempted animation throws `CancellationException`. Awaited
 * inline in the gesture loop, that exception tore the loop down and the grid stopped settling for the
 * rest of the session, which is exactly what "it sits there forever" turned out to be. Launched
 * separately, a cancellation stays inside the animation: the loop lives, and the next gesture simply
 * starts a new one.
 */
internal class CalendarSurfaceDriver(
    private val state: CalendarSurfaceState,
    private val scope: CoroutineScope,
    private val decay: DecayAnimationSpec<Float>,
) {
    private var job: Job? = null

    /** A finger on the glass stops whatever the grid was doing. */
    fun stop() {
        job?.cancel()
        job = null
    }

    /** Coasts the hours to a stop. [velocity] is the finger's, in px/s. */
    fun flingHours(velocity: Float, m: SurfaceMetrics) = animate { decayHours(velocity, m) }

    /** Coasts the days to a stop, and hands whatever is left to the week. */
    fun flingDays(velocity: Float, m: SurfaceMetrics, minFlingVelocity: Float) =
        animate { decayDays(velocity, m, minFlingVelocity) }

    /** The week a gesture left half-turned: commit it, or put it back. */
    fun releaseWeek(drag: Float, velocity: Float, m: SurfaceMetrics, minFlingVelocity: Float) =
        animate {
            val turn = pageTurnFor(drag, velocity, m.width, minFlingVelocity)
            if (turn == 0) snapHome() else turnPage(turn, m)
        }

    /**
     * **Between two weeks is never a resting place.** Whatever cancelled the gesture, a stray touch,
     * a dialog, an interrupted fling, the grid returns to the nearest week.
     */
    fun settleHome() = animate { snapHome() }

    private suspend fun decayHours(velocity: Float, m: SurfaceMetrics) {
        // The content moves opposite the finger: flick up, the day scrolls down.
        AnimationState(initialValue = state.scrollY, initialVelocity = -velocity)
            .animateDecay(decay) {
                state.scrollTo(value, m)
                // Off the end of the day: stop, rather than glide on against a wall.
                if (value != state.scrollY) cancelAnimation()
            }
    }

    /**
     * A hard flick from the middle of a three-day zoom carries through the end of the week and turns
     * it, in one gesture. That is what a nested scroll gives you for free, leftover fling velocity
     * bubbles up to the parent, and losing it would make the grid feel like it had grown a wall.
     */
    private suspend fun decayDays(velocity: Float, m: SurfaceMetrics, minFlingVelocity: Float) {
        var remaining = -velocity
        AnimationState(initialValue = state.dayX, initialVelocity = -velocity)
            .animateDecay(decay) {
                state.scrollDaysTo(value, m)
                remaining = this.velocity
                if (value != state.dayX) cancelAnimation()
            }
        // The strip hit the end of the week with speed to spare: the week takes the rest.
        val leftover = -remaining
        if (abs(leftover) > minFlingVelocity && !state.dayStripCanScroll(leftover, m)) {
            turnPage(if (leftover > 0f) -1 else 1, m)
        }
    }

    private suspend fun snapHome() {
        AnimationState(initialValue = state.pageOffset).animateTo(0f, snapSpec()) {
            state.slidePageTo(value)
        }
    }

    /**
     * Turns the week, briskly, and **decides it before it animates it**.
     *
     * The commit used to come at the *end* of the slide, which meant a turn was only real once it had
     * finished being drawn. Flick again before then and the next gesture cancelled the animation, and
     * with it a week the user had already won: its progress survived only as a partial `pageOffset`,
     * which is capped at a page. Eight fast flicks turned three weeks. **A decision that a later
     * event can undo is not a decision.**
     *
     * So the week lands first and the pixels follow. `commitPageTurn` rebases the offset by exactly
     * the width it moves the week, so the frame is unchanged, all that is different is that flicking
     * again now adds to a week already banked, instead of cancelling one still in flight.
     *
     * Measured against Samsung on the same phone: their page turn takes 0.02–0.15s; Compose's default
     * spring took us 0.32–0.50s, and *that*, not the drag threshold, is what makes rapid swiping
     * feel like fighting the app. A new flick lands on a grid still gliding from the last one.
     */
    private suspend fun turnPage(turn: Int, m: SurfaceMetrics) {
        Snapshot.withMutableSnapshot {
            state.commitPageTurn(turn, m.width)
            // A turn opens the new week on its FIRST day. The day axis is a scroll *within* a week, so
            // without this a sub-week zoom carries the old end-of-week offset across and lands mid-week
            // work-week scrolled to its end turned to the next week showing Wednesday, not Monday
            // (docs/calendar.md §6). Both directions re-seat to the start.
            state.scrollDaysTo(0f, m)
        }
        CalendarTrace.turned()
        AnimationState(initialValue = state.pageOffset).animateTo(0f, snapSpec()) {
            state.slidePageTo(value)
        }
    }

    /**
     * Runs one animation, in its own job.
     *
     * **The helpers above must never call each other through this.** A page turn that settled by
     * calling the public `settleHome()` would re-enter here, and [stop] would cancel the very job it
     * was running in, the grid killing its own settle, which is precisely the bug this design
     * exists to make impossible. Chaining happens between the private `suspend` functions, inside one
     * job.
     */
    private fun animate(block: suspend () -> Unit) {
        stop()
        job = scope.launch {
            // A preempted animation throws, and that is normal, the user grabbed the grid. It must
            // not escape into the gesture loop.
            runCatching { block() }
        }
    }
}

/**
 * The whole pointer contract of the time grid.
 *
 * [viewport] is read fresh on every event rather than captured: a pinch changes the zoom, which
 * changes every bound the pan clamps against, mid-gesture.
 */
@Suppress("LongMethod", "CyclomaticComplexMethod", "LongParameterList")
internal suspend fun PointerInputScope.calendarSurfaceGestures(
    state: CalendarSurfaceState,
    driver: CalendarSurfaceDriver,
    viewport: () -> SurfaceViewport,
    minFlingVelocity: Float,
    onZoomSettled: () -> Unit,
    onTap: (Offset) -> Unit,
    /** A press-and-hold landed: what does it mean here, if anything? */
    dragAt: (Offset) -> CalendarDragState?,
    /** The finger lifted on a drag. */
    onDrop: (CalendarDragState) -> Unit,
) {
    val slop = viewConfiguration.touchSlop
    val longPress = viewConfiguration.longPressTimeoutMillis
    awaitEachGesture {
        val down = awaitFirstDown(requireUnconsumed = false)
        driver.stop()

        val tracker = VelocityTracker()
        tracker.addPosition(down.uptimeMillis, down.position)

        var mode = GestureMode.UNDECIDED
        var travel = Offset.Zero
        var centroid = down.position
        var pointers = 1
        var spreadStart: Offset? = null
        var landed = false
        // What THIS finger did. Not where the page is sitting, that carries the lag of a week already
        // won, which looks exactly like a drag the other way.
        var dragX = 0f
        // How much of the long-press window is left, and when it was last measured.
        //
        // Measured off the **pointer clock** rather than restarted per event, because a real finger
        // is never still: it jitters within the slop every frame, and a `withTimeout` re-armed on
        // each of those would never fire at all. The timeout below only has to cover the one case
        // pointer events cannot, a finger so still that no events arrive.
        var remaining = longPress
        var lastAt = down.uptimeMillis

        try {
            while (true) {
                val waiting = mode == GestureMode.UNDECIDED && remaining > 0
                val event =
                    if (waiting) withTimeoutOrNull(remaining) { awaitPointerEvent() } else awaitPointerEvent()
                if (event == null) {
                    // The window elapsed in silence: a press-and-hold.
                    remaining = 0
                    dragAt(centroid)?.let {
                        mode = GestureMode.DRAG
                        state.beginDrag(it)
                    }
                    continue
                }
                val pressed = event.changes.filter { it.pressed }
                if (pressed.isEmpty()) break
                if (waiting) {
                    val now = pressed[0].uptimeMillis
                    remaining -= (now - lastAt).coerceAtLeast(0L)
                    lastAt = now
                }

                val metrics = state.metrics(viewport())

                // A pinch, once the fingers have genuinely spread. Two fingers merely travelling together
                // are a SWIPE, and reading them as a zoom is how the swipe got stolen.
                //
                // A drag already in flight owns the gesture outright: a second finger arriving on top
                // of a block the user is holding is a hand steadying itself, not a request to zoom.
                if (pressed.size >= 2 && mode != GestureMode.ZOOM && mode != GestureMode.DRAG) {
                    // Two fingers are never a press-and-hold, whatever they do next.
                    remaining = 0
                    val spread = spreadOf(pressed[0].position, pressed[1].position)
                    val start = spreadStart ?: spread.also { spreadStart = it }
                    if (beginsPinch(start, spread)) {
                        mode = GestureMode.ZOOM
                        // Pin the width the labels are shaped against, for as long as the fingers are
                        // down. A pinch moves the column width every frame, and a moving width is a
                        // shaper cache that misses every frame.
                        state.beginZoom(metrics)
                    }
                } else if (pressed.size < 2) {
                    spreadStart = null
                }

                if (mode == GestureMode.ZOOM && pressed.size >= 2) {
                    zoom(pressed[0], pressed[1], state, metrics, viewport)
                    pressed.forEach { it.consume() }
                    continue
                }

                // The centroid, so adding or lifting a finger mid-drag does not lurch the grid: on the
                // frame the pointer count changes there is no delta at all, and the next one measures
                // from the new middle rather than jumping to it.
                val sum = pressed.fold(Offset.Zero) { acc, it -> acc + it.position }
                val next = sum / pressed.size.toFloat()
                val delta = if (pressed.size == pointers) next - centroid else Offset.Zero
                centroid = next
                pointers = pressed.size
                tracker.addPosition(pressed[0].uptimeMillis, centroid)

                if (mode == GestureMode.UNDECIDED) {
                    travel += delta
                    // One axis, decided once. The hours and the days are separate scrolls to the user's
                    // hand, and a drag that did both at once would turn the week while they were reading
                    // down a day.
                    if (abs(travel.x) > slop || abs(travel.y) > slop) {
                        // Deciding the mode is what ends the hold's candidacy: `waiting` is gated on
                        // `UNDECIDED`, so from here the timeout is never armed again. (A second
                        // `remaining = 0` here was written and then deleted, a mutation of it
                        // failed no test, which is how it was found out as dead.)
                        mode = if (abs(travel.x) > abs(travel.y)) {
                            GestureMode.PAN_DAYS
                        } else {
                            GestureMode.PAN_HOURS
                        }
                    }
                }
                when (mode) {
                    GestureMode.PAN_DAYS -> {
                        dragX += delta.x
                        state.panX(delta.x, metrics)
                    }
                    GestureMode.PAN_HOURS -> state.panY(delta.y, metrics)
                    // The grid does not move under a drag: the block follows the finger and the
                    // week stays exactly where the user could see it when they picked it up.
                    GestureMode.DRAG -> dragTo(centroid, state, metrics)
                    else -> Unit
                }
                if (mode != GestureMode.UNDECIDED) pressed.forEach { it.consume() }
            }

            val metrics = state.metrics(viewport())
            val velocity = tracker.calculateVelocity()
            CalendarTrace.gesture(
                when (mode) {
                    GestureMode.ZOOM -> "zoom"
                    GestureMode.PAN_DAYS -> "days"
                    GestureMode.PAN_HOURS -> "hours"
                    GestureMode.DRAG -> "drag"
                    GestureMode.UNDECIDED -> "tap"
                },
            )
            when (mode) {
                GestureMode.ZOOM -> onZoomSettled()
                GestureMode.PAN_HOURS -> driver.flingHours(velocity.y, metrics)
                // The day strip owns the drag while it still has somewhere to go; once it has run out
                // of week, or a turn is already in flight, lagging, the week takes the release.
                GestureMode.PAN_DAYS ->
                    if (state.pageOffset != 0f) {
                        driver.releaseWeek(dragX, velocity.x, metrics, minFlingVelocity)
                    } else {
                        driver.flingDays(velocity.x, metrics, minFlingVelocity)
                    }
                // A drop writes only if it actually moved something: a hold that went nowhere on an
                // existing event is a hold, not an edit, and sending a zero-delta patch would spend a
                // network round-trip and a revision to change nothing.
                GestureMode.DRAG -> state.endDrag()
                    ?.takeIf { it.movesAnything() }
                    ?.let(onDrop)
                // Never moved: a tap.
                GestureMode.UNDECIDED -> onTap(down.position)
            }
            landed = true
        } finally {
            // **Between two weeks is never a resting place.** If the gesture was cancelled rather
            // than released, a system dialog, the pointer filter torn down mid-drag, none of the
            // above ran, and the week would sit half-turned for as long as the user looked at it.
            // The snap runs on the surface's own scope, not this cancelled one, or it would die with
            // the gesture that needed it.
            if (!landed) {
                // A cancelled drag writes nothing. It must still be *cleared*, or the preview stays
                // painted over the grid with no finger anywhere near it.
                state.cancelDrag()
                if (state.pageOffset != 0f) driver.settleHome()
            }
        }
    }
}

/** One frame of a drag: re-aim it at wherever the finger is now, in the core's own geometry. */
private fun dragTo(at: Offset, state: CalendarSurfaceState, m: SurfaceMetrics) {
    val point = m.contentPoint(at, state.dayX, state.scrollY)
    state.dragTo(m.columnAt(point.x), m.minuteAt(point.y), m.rawMinuteAt(point.y), DAYS_IN_WEEK)
}

/**
 * One frame of a pinch, both axes, each by its own component of the spread.
 *
 * The focal point is handed to the state in **content** coordinates (past the hour ruler, below the
 * banner), which is the frame the scroll offsets live in.
 */
private fun zoom(
    a: PointerInputChange,
    b: PointerInputChange,
    state: CalendarSurfaceState,
    metrics: SurfaceMetrics,
    viewport: () -> SurfaceViewport,
) {
    val x = axisScale(
        abs(a.previousPosition.x - b.previousPosition.x),
        abs(a.position.x - b.position.x),
    )
    val y = axisScale(
        abs(a.previousPosition.y - b.previousPosition.y),
        abs(a.position.y - b.position.y),
    )
    if (x == 1f && y == 1f) return
    state.pinch(
        xScale = x,
        yScale = y,
        focusX = (a.position.x + b.position.x) / 2f - metrics.gutter,
        focusY = (a.position.y + b.position.y) / 2f - metrics.contentTop,
        viewport = viewport(),
    )
}
