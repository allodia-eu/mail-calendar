// Where the grid is, how big it is, and what a gesture does to it, one owner, one place.
//
// The grid used to be scrolled by a `verticalScroll`, scrolled by a `horizontalScroll`, paged by a
// `HorizontalPager` and zoomed by a `pinchToZoom` modifier: **four handlers arbitrating over one
// pointer stream**, none of which could see the others. That is not a tuning problem, and it was not
// fixable from inside: a consumed pointer event does not politely ask a scroller to stand aside, it
// CANCELS its drag, and a cancelled drag never flings, so the pager stopped dead between two weeks.
// Refusing to consume fixed the swipe and broke the zoom instead (the scrollers happily panned the
// grid around under a pinch, because nothing had told them not to).
//
// So: nobody consumes, because there is nobody to consume *from*. This class is the whole navigation
// model of the time grid, the scroll offsets, the zoom, the week, and the gesture layer is a thin
// thing that decides pan vs. page vs. zoom and calls exactly one of these. It is a plain class rather
// than a knot of `remember`s so the state machine is unit-testable without composing anything (cf.
// CalendarPager, SwipeUndoController).
package eu.allodia.mailcal

import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableFloatStateOf
import androidx.compose.runtime.mutableIntStateOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.setValue
import androidx.compose.ui.geometry.Rect
import kotlin.math.abs

/**
 * How far you must drag before the week turns, as a fraction of the page.
 *
 * Compose's default is `0.5`, half the screen, which feels like wading. A swipe is a flick of the
 * thumb, not a haul. This is the **slow-drag** path only: a quick flick pages on velocity alone,
 * whatever the distance, which is how most swipes actually commit.
 *
 * The floor is set by the opposite failure: low enough, and an idle horizontal wobble while reading
 * turns the week under you. A sixth of a phone screen is still a couple of centimetres of deliberate
 * movement, which no wobble covers.
 */
internal const val PAGE_TURN_THRESHOLD = 1f / 6f

/**
 * How many whole pages the pixels may lag behind the week they have already landed on.
 *
 * A flick decides its week immediately, so flicking faster than the grid can slide leaves the image
 * *behind* the truth, which is fine, and is exactly what "snappy" is made of. Two pages, because two
 * is what the grid can actually draw through: at a lag of `f` pages the visible pages are
 * `(-1 - f) .. (1 - f)`, so a two-page lag reaches page -2 and no further, and the five live pages
 * cover it. Let it lag three and the grid would slide through a week it does not hold, and draw a gap.
 */
internal const val MAX_PAGE_LAG = 2f

/**
 * The chrome around the grid, in pixels, everything a zoom cannot change.
 *
 * [lanes] is the **true** lane count the core stacked the current page's all-day bands into, not the
 * number the banner shows: the cap is applied here, against the banner's own expanded state.
 */
internal data class SurfaceViewport(
    val width: Float,
    val height: Float,
    val gutter: Float,
    val headerHeight: Float,
    val laneHeight: Float,
    val dividerHeight: Float,
    val lanes: Int,
)

/**
 * The grid's pixel geometry at the current zoom.
 *
 * This is the client half of the contract's first rule: the core says "day 3, minute 545, column 1 of
 * 2" and carries no units at all; everything below is a multiplication. An hour has no height and a
 * day no width until this type says so.
 *
 * A page's stride is the **whole surface width**, gutter included, the hour ruler slides out with
 * its own week, because a page is a week and the ruler belongs to it.
 */
internal data class SurfaceMetrics(
    val viewport: SurfaceViewport,
    val hourHeight: Float,
    val dayWidth: Float,
    val bannerLanes: Int,
) {
    val width: Float get() = viewport.width
    val height: Float get() = viewport.height
    val gutter: Float get() = viewport.gutter

    /** The width the day columns are scrolled inside, the surface, less the hour ruler. */
    val dayViewport: Float get() = (width - gutter).coerceAtLeast(0f)

    /** A page is a **week**: all seven days, whatever the zoom shows of them. */
    val weekWidth: Float get() = dayWidth * DAYS_IN_WEEK
    val gridHeight: Float get() = hourHeight * HOURS_IN_DAY

    val bannerHeight: Float get() = viewport.laneHeight * bannerLanes
    val contentTop: Float get() = viewport.headerHeight + bannerHeight + viewport.dividerHeight
    val contentHeight: Float get() = (height - contentTop).coerceAtLeast(0f)

    /** How far the day strip can scroll before the week runs out. `0` at the whole-week zoom, and
     *  that is why every sideways drag there reaches the pager, which is exactly the point. */
    val maxDayX: Float get() = (weekWidth - dayViewport).coerceAtLeast(0f)
    val maxScrollY: Float get() = (gridHeight - contentHeight).coerceAtLeast(0f)

    /** The top of a *neighbouring* page's grid, which has its own lane count and so its own banner. */
    fun contentTopFor(lanes: Int, expanded: Boolean): Float =
        viewport.headerHeight +
            viewport.laneHeight * allDayBannerLanes(lanes, expanded) +
            viewport.dividerHeight
}

/**
 * The whole state of the time grid: where it is scrolled, how far it is zoomed, and which week.
 *
 * Both zoom axes are **fractional**. A pinch is continuous, and rounding mid-gesture makes the grid
 * jump between whole hours (or whole columns) instead of tracking the fingers, the difference
 * between "buttery" and "broken". Only the settled values are whole, and only those are persisted.
 */
internal class CalendarSurfaceState(
    visibleHours: Int,
    visibleDays: Int,
) {
    /** The two zoom axes, and the rules about where they stop. */
    val zoom = CalendarZoom(visibleHours, visibleDays)

    /** How far down the day the grid is scrolled, in pixels. */
    var scrollY by mutableFloatStateOf(0f)
        private set

    /** How far along the **week** the day strip is scrolled. Shared by every page, so turning the
     *  week keeps the same days in view. */
    var dayX by mutableFloatStateOf(0f)
        private set

    /**
     * How far the week itself has slid, in pixels. Zero at rest, **between two weeks is never a
     * resting place**, and non-zero while a turn is in flight, or while the pixels are still catching
     * up with a week that has already been decided.
     *
     * Positive means the page has moved right, revealing the *previous* week from the left.
     *
     * It may lag by up to [MAX_PAGE_LAG] whole pages, and that is what lets rapid flicks accumulate
     * rather than swallow one another (see [commitPageTurn]). Two pages is the ceiling because that is
     * what the five live pages either side can actually draw, past it the grid would have to slide
     * through a week it does not hold.
     */
    var pageOffset by mutableFloatStateOf(0f)
        private set

    /** Which week is showing, counted from the pager's origin. */
    var week by mutableIntStateOf(0)
        private set

    /** Whether the all-day banner is showing every lane, or capped at three with a per-day "+N". */
    var bannerExpanded by mutableStateOf(false)
        private set

    /**
     * The column width the labels are **shaped** against, frozen for the length of a pinch, and `0`
     * when no pinch is in flight.
     *
     * Measured, on a real diary: a pinch's draw cost **3.4× a swipe's** (1709µs against 496µs) while
     * drawing *half* as many blocks. Backwards, and the reason is the one thing a zoom genuinely does
     * change. During a swipe the column width is constant, so every text measurement hits the shaper's
     * cache and costs nothing. During a pinch it changes every frame, the cache key changes with it,
     * and every visible label is re-shaped from scratch, sixty times a second, in the gesture the
     * whole grid is judged on. Bucketing the width only delays the miss; it does not stop it.
     *
     * So the shaping width simply **stops moving** while the fingers are down. The block's rectangle
     * scales every frame, as it must, it is the *text layout* inside it that is held, and the text is
     * clipped to the live rectangle regardless. The cost is that a title ellipsises against the width
     * the block had when the pinch began, which is invisible: nobody reads a label while it is moving.
     * It re-shapes when the fingers lift.
     */
    var shapedDayWidth by mutableFloatStateOf(0f)
        private set

    /**
     * The drag in flight, a block being moved or resized, or a slot being drawn out, or `null`.
     *
     * It lives here, beside the scroll offsets, for the same reason they do: the gesture owner is
     * the only thing that writes it and the renderer is the only thing that reads it, and a second
     * place to keep "what is the finger doing" is a second thing that can disagree with the first.
     */
    var drag by mutableStateOf<CalendarDragState?>(null)
        private set

    /** The fingers went down: pin the shaping width where it is. */
    fun beginZoom(m: SurfaceMetrics) {
        shapedDayWidth = m.dayWidth
    }

    // ---- Dragging --------------------------------------------------------------------------------

    /** A press-and-hold landed on something: the drag begins. */
    fun beginDrag(drag: CalendarDragState) {
        this.drag = drag
    }

    /** The finger moved: re-aim the drag, clamped so its preview stays on the grid it is drawn on. */
    fun dragTo(day: Int, minute: Int, rawMinute: Int, columns: Int) {
        drag = drag?.movedTo(day, minute, rawMinute)?.clampedTo(columns)
    }

    /** The finger lifted: hand back the settled drag (if any) and clear it. */
    fun endDrag(): CalendarDragState? = drag.also { drag = null }

    /** The gesture was cancelled, a dialog, a torn-down filter. Nothing is written. */
    fun cancelDrag() {
        drag = null
    }

    /** The pixel geometry the current zoom implies, inside [viewport]. */
    fun metrics(viewport: SurfaceViewport): SurfaceMetrics = SurfaceMetrics(
        viewport = viewport,
        // The whole surface height over the horizon, so "show me 12 hours" means the same span on a
        // phone and on a tablet, the cells just get bigger.
        hourHeight = zoom.hourHeight(viewport.height),
        dayWidth = zoom.dayWidth((viewport.width - viewport.gutter).coerceAtLeast(0f)),
        bannerLanes = allDayBannerLanes(viewport.lanes, bannerExpanded),
    )

    // ---- Panning ---------------------------------------------------------------------------------

    /** Scrolls the hours. [dy] is the finger's movement: down reveals earlier hours. */
    fun panY(dy: Float, m: SurfaceMetrics) {
        scrollY = (scrollY - dy).coerceIn(0f, m.maxScrollY)
    }

    /**
     * Scrolls the days, and turns the week with whatever is left over.
     *
     * **The day strip takes the drag first, and the week gets the remainder.** That is not an
     * accident of how Compose nests scrollables, it is the behaviour we want, and having one
     * function do both is what stops the two of them fighting over the same finger. Reverse a drag
     * mid-turn and the page unwinds to home *before* the days start moving again, because a page
     * offset only ever exists when the day strip is already at a bound.
     */
    fun panX(dx: Float, m: SurfaceMetrics) {
        var d = dx
        if (pageOffset != 0f) {
            val next = pageOffset + d
            val crossedHome = (pageOffset > 0f && next < 0f) || (pageOffset < 0f && next > 0f)
            if (!crossedHome) {
                pageOffset = next.coerceIn(-m.width * MAX_PAGE_LAG, m.width * MAX_PAGE_LAG)
                return
            }
            // The turn unwound all the way home; the overshoot belongs to the days.
            pageOffset = 0f
            d = next
        }
        val next = (dayX - d).coerceIn(0f, m.maxDayX)
        val absorbed = dayX - next
        dayX = next
        val remainder = d - absorbed
        if (remainder != 0f) {
            val lag = m.width * MAX_PAGE_LAG
            pageOffset = (pageOffset + remainder).coerceIn(-lag, lag)
        }
    }

    /** Whether the day strip can still absorb a drag in [dx]'s direction, i.e. this is a *pan*, not
     *  the start of a page turn. Lets the fling hand its leftover velocity to the week. */
    fun dayStripCanScroll(dx: Float, m: SurfaceMetrics): Boolean =
        if (dx < 0f) dayX < m.maxDayX else dayX > 0f

    // ---- Zooming ---------------------------------------------------------------------------------

    /**
     * One frame of a pinch: both axes, each by its own component of the spread.
     *
     * [focusX] and [focusY] are the fingers' midpoint **relative to the content viewport**, the day
     * strip's left edge and the grid's top. Anchoring on them is what keeps the content under the
     * fingers still; without it the offset stays fixed in *pixels* while the scale changes, so the
     * same offset maps to a different time and the grid slides out from under the user's hand.
     *
     * Each axis is corrected by **the factor its zoom actually applied**, not the one it was asked
     * for. At a clamp that is `1`, and correcting by the requested factor there would drag the grid
     * on every further frame of a pinch that has nowhere left to go, and let an exhausted hour axis
     * drag the day axis to a halt mid-diagonal.
     *
     * Note what this does **not** do: pan. The fingers' midpoint travelling across the glass moves
     * nothing. That it used to, that a pinch dragged the week around under your hand, was the price
     * of the pinch consuming nothing while three other handlers still read the same stream. There are
     * no other handlers now.
     */
    fun pinch(xScale: Float, yScale: Float, focusX: Float, focusY: Float, viewport: SurfaceViewport) {
        val hours = zoom.pinchVertical(yScale)
        if (hours != 1f) scrollY = focalPreservingScroll(scrollY, focusY, hours)
        val days = zoom.pinchHorizontal(xScale)
        if (days != 1f) dayX = focalPreservingScroll(dayX, focusX, days)
        // The zoom just moved both maxima; a scroll left past the new end would show empty space.
        clampScroll(metrics(viewport))
    }

    /**
     * Snaps the day axis to the zoom level the pinch settled on, and returns that level.
     *
     * The snap itself, and the reason it is to the settled *level's* columns rather than to the
     * rounded count, lives in [CalendarZoom.settleDays]. All this adds is the scroll clamp: the
     * columns just changed width, so the week's end moved.
     */
    fun settleZoom(viewport: SurfaceViewport): CalendarMode {
        zoom.settleDays()
        clampScroll(metrics(viewport))
        // The fingers are up: the labels may re-shape against the width they actually have.
        shapedDayWidth = 0f
        return modeForColumns(zoom.settledDays())
    }

    /** The whole-hour horizon to persist once the fingers lift. */
    fun settledHours(): Int = zoom.settledHours()

    // ---- Paging ----------------------------------------------------------------------------------

    /** Slides the week, the animation target, driven a frame at a time. */
    fun slidePageTo(offset: Float) {
        pageOffset = offset
    }

    /**
     * Lands on [turn] weeks from here, **now**, before a single pixel has moved.
     *
     * The offset is rebased by exactly the width of the turn, which leaves the image *identical*:
     * a page sits at `(k - week) * width + pageOffset`, so adding one to `week` and one page-width to
     * `pageOffset` cancel. Nothing on screen shifts. What changes is that the week is now **decided**,
     * and the animation that follows is only the pixels catching up.
     *
     * **This is what stops fast flicks eating each other.** The turn used to be committed at the *end*
     * of its animation, so a second flick landing mid-flight cancelled the first before it had ever
     * been recorded, its progress left as a partial `pageOffset`, which is capped at one page. Two
     * flicks could then only ever add up to one week. Measured: eight fast flicks turned **three**
     * weeks. Decide first, animate second, and a flick can no longer be un-decided by the next one.
     *
     * Both writes must land in one frame, or a composition between them draws the shifted image.
     */
    fun commitPageTurn(turn: Int, width: Float) {
        week += turn
        val lag = MAX_PAGE_LAG * width
        pageOffset = (pageOffset + turn * width).coerceIn(-lag, lag)
    }

    /** Drops the week back to the origin, a view switch, or "back to today". */
    fun resetWeek() {
        week = 0
        pageOffset = 0f
    }

    // ---- Everything else -------------------------------------------------------------------------

    fun toggleBanner() {
        bannerExpanded = !bannerExpanded
    }

    /** Re-seeds the horizon (on load, or when the settings screen changes it). */
    fun resetHours(hours: Int) = zoom.resetHours(hours)

    /** Re-seeds the day axis (on load, or when a shape is picked from the menu). */
    fun resetDays(days: Int) = zoom.resetDays(days)

    fun scrollTo(y: Float, m: SurfaceMetrics) {
        scrollY = y.coerceIn(0f, m.maxScrollY)
    }

    fun scrollDaysTo(x: Float, m: SurfaceMetrics) {
        dayX = x.coerceIn(0f, m.maxDayX)
    }

    /** Puts both axes back inside their range, after a zoom, or a change of banner height. */
    fun clampScroll(m: SurfaceMetrics) {
        scrollY = scrollY.coerceIn(0f, m.maxScrollY)
        dayX = dayX.coerceIn(0f, m.maxDayX)
    }
}

// ---- The multiplication -------------------------------------------------------------------------
//
// This is the client's *whole* contribution to layout, and the only thing about it that can be wrong.
// The core solves the grid in units that carry no pixels at all, a day **index**, a wall-clock
// **minute**, a column **fraction**, and the three functions below turn those into rectangles. Swap
// `column` for `columns`, or forget to offset by the day, and every block still renders perfectly
// plausibly, in the wrong place.
//
// They are shared by the renderer **and** by the accessibility overlay, deliberately. Two copies of
// this arithmetic would be two chances to disagree, and a screen reader announcing an event somewhere
// other than where it is drawn is a bug nobody would ever see.

/** A block's rectangle, in its page's content coordinates, before the scroll and the page offset. */
internal fun SurfaceMetrics.blockRect(block: BlockPaint): Rect {
    val columnWidth = dayWidth / block.columns
    val left = dayWidth * block.day + columnWidth * block.column
    val top = hourHeight * (block.startMinutes / 60f)
    return Rect(
        left = left,
        top = top,
        right = left + columnWidth,
        bottom = top + hourHeight * (block.minutes / 60f),
    )
}

/** An all-day bar's rectangle, in the banner's own coordinates. */
internal fun SurfaceMetrics.bandRect(span: BandSpan): Rect {
    val left = dayWidth * span.day
    val top = viewport.laneHeight * span.lane
    return Rect(
        left = left,
        top = top,
        right = left + dayWidth * span.days,
        bottom = top + viewport.laneHeight,
    )
}

/** A "+N" chip's rectangle: one column wide, in the row the last visible lane gave up. */
internal fun SurfaceMetrics.moreRect(day: Int, lane: Int): Rect =
    bandRect(BandSpan(day = day, days = 1, lane = lane))

/**
 * Which way a released gesture turns the week: `-1` back, `+1` on, `0` to stay.
 *
 * **[drag] is how far *this gesture's finger* travelled, not where the page happens to be sitting.**
 * The distinction is the whole bug. Once a turn is committed the instant it is decided, the page
 * carries a *lag*, a week already won, whose pixels have not arrived yet, and that lag looks
 * exactly like a drag in the opposite direction. Judge a flick by it and the second of two fast
 * flicks reads as "he's changed his mind" and cancels itself, which is precisely the swallowed swipe.
 * A gesture is judged only on what the gesture did.
 *
 * Two ways to commit, and the slow one is not the one most swipes take: a flick pages on **velocity**
 * alone, whatever the distance it covered. Both share a sign convention, negative is the finger
 * moving left, which brings on the *next* week.
 *
 * A hard flick **back** over a drag that had already passed the threshold the other way cancels the
 * turn rather than reversing it: that is a change of mind, not a swipe, and it is what makes a swipe
 * feel like it can be taken back.
 */
internal fun pageTurnFor(
    drag: Float,
    velocity: Float,
    width: Float,
    minFlingVelocity: Float,
): Int {
    if (width <= 0f) return 0
    val byDrag = when {
        abs(drag) <= width * PAGE_TURN_THRESHOLD -> 0
        drag < 0f -> 1
        else -> -1
    }
    if (abs(velocity) <= minFlingVelocity) return byDrag
    val byFlick = if (velocity < 0f) 1 else -1
    // Flicked back over a drag that had already committed the other way: he changed his mind.
    if (byDrag != 0 && byDrag != byFlick) return 0
    return byFlick
}

/**
 * The scroll offset that keeps whatever was under [focus] exactly under [focus], after the content
 * has been scaled by [factor].
 *
 * The content point under the fingers sits at `scroll + focus` pixels along the content. Scaling
 * moves it to `(scroll + focus) * factor`; putting it back under the same finger means scrolling to
 * that, less the finger's own offset in the viewport.
 *
 * Works for either axis, the hours and the days anchor by exactly the same arithmetic, which is what
 * lets a diagonal pinch anchor on a single point rather than fighting itself.
 */
internal fun focalPreservingScroll(scroll: Float, focus: Float, factor: Float): Float =
    ((scroll + focus) * factor - focus).coerceAtLeast(0f)
