// Dragging on the grid: what a press-and-hold turned out to be, and where it would leave the event.
//
// Everything here is arithmetic over the core's unit-free geometry, a day index and a wall-clock
// minute, and none of it touches Compose beyond a `Rect`. That is the point: the gesture layer is a
// translation (a finger goes down here, a finger is now there) and every *decision* is a pure
// function a JVM test can drive without composing a screen. Same split as CalendarSurfaceState.
//
// The rule this file exists to hold: **a drag is a delta, not a destination.** What crosses the FFI
// is how far the hand moved, in whole days and minutes, never the clock the block was dropped under.
// A meeting in Amsterdam read on a phone set to New York is drawn six hours earlier, and the delta is
// the same number in either zone, see `mailcal_account::calendar_drag` for the full reasoning.
package eu.allodia.mailcal

import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.geometry.Rect
import kotlin.math.floor
import kotlin.math.roundToInt
import uniffi.mailcal_bindings.EventEdge

/**
 * The grid a drag snaps to, in minutes.
 *
 * Fifteen because that is what a diary is written in and what every calendar snaps to. The delta is
 * snapped, not the destination: an event that genuinely starts at 10:07 keeps its seven minutes when
 * you move it a day sideways, rather than being quietly re-timed by a gesture that was not about its
 * start at all.
 */
internal const val DRAG_SNAP_MINUTES = 15

/** How long a slot a press-and-hold with no movement creates. */
internal const val DEFAULT_CREATE_MINUTES = 60

/** Minutes in a day column. The grid is wall-clock, so this never changes. */
internal const val DAY_MINUTES = 24 * 60

/** Minutes in an hour, the coarser grid a press-and-hold pins its slot to. */
internal const val HOUR_MINUTES = 60

/**
 * The hour band a press-and-hold's slot fills, the one the finger is **inside**.
 *
 * A press is "an event here", not a time to the minute, so it fills the hour it landed in rather
 * than the quarter-hour it happened to touch. The band **contains** the touch point, which is the
 * property that matters: rounding to the *nearest* boundary instead sends a press at 17:50 to
 * 18:00–19:00, drawing the whole block below the finger that asked for it.
 *
 * Takes the unrounded minute for the same reason, a touch at 16:53 snaps to 17:00, and pinning from
 * that would put the block in the next band along from the one under the finger.
 */
internal fun hourBandAt(rawMinute: Int): Int =
    (rawMinute / HOUR_MINUTES * HOUR_MINUTES)
        .coerceIn(0, DAY_MINUTES - DEFAULT_CREATE_MINUTES)

/** What a press-and-hold on the grid turned out to be. */
internal enum class DragKind {
    /** The whole block moves: both edges by the same delta, so the duration is preserved exactly. */
    MOVE,

    /** The top edge moves; the end stays. */
    RESIZE_START,

    /** The bottom edge moves; the start stays. */
    RESIZE_END,

    /** Empty grid: a new slot is being drawn out. */
    CREATE,
}

/**
 * The event a move or resize is reshaping, captured when the finger went down.
 *
 * Captured rather than looked up per frame for the reason everything in `CalendarSurfaceModel` is:
 * the page underneath can be repainted mid-gesture (a sync lands, the calendar signal fires) and a
 * drag that re-resolved its subject would jump to a different block, or to none.
 */
internal data class DragSubject(
    val account: String,
    val event: String,
    /**
     * The occurrence's own start, as the core minted it, **empty when the event does not recur**.
     *
     * Non-empty is the signal to ask "this event, or all of them?" before writing anything. The
     * value is opaque: it goes back across the FFI verbatim, never parsed here.
     */
    val occurrenceStart: String,
    val day: Int,
    val startMinutes: Int,
    val endMinutes: Int,
) {
    val minutes: Int get() = endMinutes - startMinutes
}

/**
 * A drag in flight: where it began, and where the finger is now.
 *
 * Both positions are the core's own currency, a day column and a wall-clock minute, so the deltas
 * below are exactly what `Intent.MoveEvent` wants and nothing has to be converted at the boundary.
 */
internal data class CalendarDragState(
    val kind: DragKind,
    /** `null` for a [DragKind.CREATE], which has no event yet. */
    val subject: DragSubject?,
    val anchorDay: Int,
    val anchorMinute: Int,
    val day: Int,
    val minute: Int,
    /**
     * Where the finger actually is, **unsnapped**, the picture's currency, never the write's.
     *
     * [minute] steps a quarter-hour at a time because that is what gets written; a block that moved
     * only when it stepped would jump a dozen pixels at a zoomed-out horizon, which is what reads as
     * coarse next to a calendar that glides. So the renderer follows this instead ([livePreview]),
     * and the readout and [moveArgs] keep following [minute]. Defaults to [minute], which is what a
     * drag that has not moved yet means.
     */
    val rawMinute: Int = minute,
    /**
     * Where the finger went **down**, unrounded, what the hour pin is measured from.
     *
     * [anchorMinute] is snapped, and a touch at 16:53 snaps forward to 17:00; pinning from that puts
     * the slot in the band below the one the finger is actually in. Defaults to [anchorMinute], which
     * is right for a move or a resize: neither pins, and both anchor on an edge rather than a touch.
     */
    val rawAnchorMinute: Int = anchorMinute,
) {
    val dayDelta: Int get() = day - anchorDay
    val minuteDelta: Int get() = minute - anchorMinute

    /** The unsnapped distance the finger has travelled, [minuteDelta]'s picture-side twin. */
    val rawMinuteDelta: Int get() = rawMinute - anchorMinute

    /** The finger moved: re-aim at where it is now, snapped and unsnapped alike. */
    fun movedTo(day: Int, minute: Int, rawMinute: Int): CalendarDragState =
        copy(day = day, minute = minute, rawMinute = rawMinute)

    /**
     * Where the drag would leave things, as `(day, startMinutes, endMinutes)`, what the preview
     * draws, and, for a create, what the editor opens on.
     *
     * A **create** stays in the column it began in: dragging sideways while drawing out a slot is
     * how you would expect to widen it across days, which is not a thing an event can be. A
     * **move** carries its column with the finger.
     */
    fun preview(): DragPreview = previewUsing(minuteDelta)

    /**
     * Where the block is **drawn** while the finger is still down.
     *
     * The same geometry as [preview], carried by the unsnapped delta instead of the snapped one, so
     * the block glides with the hand rather than stepping a quarter-hour at a time. The two differ
     * only mid-gesture and never by a whole snap step, and only ever on the edge actually in the
     * hand, the anchored edge is drawn exactly where the write will put it.
     *
     * This is the one place the picture is allowed to differ from the write, and it is why the live
     * readout exists: the pill says the time that will be *written*, so nothing on screen is
     * claiming a minute the drop will not honour.
     */
    fun livePreview(): DragPreview = previewUsing(rawMinuteDelta)

    private fun previewUsing(delta: Int): DragPreview = when (kind) {
        DragKind.CREATE -> {
            // The slot is the **union** of the hour the press landed in and where the finger is now.
            //
            // Stay inside the band and it is the band: "an event here", a clean hour on the hour.
            // Drag below it and the top stays on that hour while the bottom follows; drag above it
            // and the bottom stays on the following hour while the top follows. So the edge that is
            // not in the hand is always a whole hour, which is what a diary entry is drawn from.
            //
            // Union rather than an anchor-and-span, because a union is *continuous*: there is no
            // threshold at which the slot changes shape, so nothing can jump at one. An anchored
            // span has to choose an anchor, and choosing the touch point makes the block leap off
            // the hour it was showing the moment the finger moves.
            val band = hourBandAt(rawAnchorMinute)
            val finger = anchorMinute + delta
            DragPreview(
                day = anchorDay,
                startMinutes = minOf(band, finger),
                endMinutes = maxOf(band + DEFAULT_CREATE_MINUTES, finger),
            )
        }
        DragKind.MOVE -> {
            val subject = requireNotNull(subject) { "a move needs a subject" }
            DragPreview(
                day = subject.day + dayDelta,
                startMinutes = subject.startMinutes + delta,
                endMinutes = subject.endMinutes + delta,
            )
        }
        DragKind.RESIZE_START -> {
            val subject = requireNotNull(subject) { "a resize needs a subject" }
            DragPreview(
                day = subject.day,
                startMinutes = subject.startMinutes + delta,
                endMinutes = subject.endMinutes,
            )
        }
        DragKind.RESIZE_END -> {
            val subject = requireNotNull(subject) { "a resize needs a subject" }
            DragPreview(
                day = subject.day,
                startMinutes = subject.startMinutes,
                endMinutes = subject.endMinutes + delta,
            )
        }
    }

    /** Whether this drag has actually changed anything, a hold that went nowhere writes nothing. */
    fun movesAnything(): Boolean = when (kind) {
        DragKind.CREATE -> true
        DragKind.MOVE -> dayDelta != 0 || minuteDelta != 0
        DragKind.RESIZE_START, DragKind.RESIZE_END -> minuteDelta != 0
    }
}

/** The arguments a settled drag dispatches (`Intent.MoveEvent`). */
internal data class MoveArgs(
    val account: String,
    val key: String,
    val edge: EventEdge,
    val days: Int,
    val minutes: Int,
    /** `null` moves the whole series; a token names one occurrence. */
    val occurrence: String?,
)

/**
 * The move a settled drag asks for, or `null` if it drew out a new slot instead.
 *
 * [thisOccurrenceOnly] is the user's answer to "this event, or all of them?", asked only when the
 * subject carries an occurrence token, and never guessed. Passing `false` for a one-off is correct
 * and costs nothing: its token is empty, so there is no occurrence to name either way.
 */
internal fun CalendarDragState.moveArgs(thisOccurrenceOnly: Boolean): MoveArgs? {
    val subject = subject ?: return null
    return MoveArgs(
        account = subject.account,
        key = subject.event,
        edge = when (kind) {
            DragKind.RESIZE_START -> EventEdge.START
            DragKind.RESIZE_END -> EventEdge.END
            else -> EventEdge.WHOLE
        },
        days = dayDelta,
        minutes = minuteDelta,
        occurrence = subject.occurrenceStart.takeIf { thisOccurrenceOnly && it.isNotEmpty() },
    )
}

/** Whether a settled drag has to ask the user which occurrences it applies to before it writes. */
internal fun CalendarDragState.asksAboutTheSeries(): Boolean =
    subject?.occurrenceStart?.isNotEmpty() == true

/** Where a drag would leave a block, in the core's own geometry. */
internal data class DragPreview(
    val day: Int,
    val startMinutes: Int,
    val endMinutes: Int,
) {
    val minutes: Int get() = endMinutes - startMinutes
}

/**
 * Clamps a drag so its preview stays inside the grid it is drawn on.
 *
 * **What you see is what you get.** A move is clamped to its own day column and to the day's own
 * midnight-to-midnight span, so an event dragged to the top of the screen stops at 00:00 rather than
 * silently landing on the previous day, to move an event to another day you drag *sideways*, which
 * is what every calendar does and what the preview can actually show. A resize is clamped so the
 * edge being dragged cannot pass its opposite, matching the core's own floor.
 */
internal fun CalendarDragState.clampedTo(columns: Int): CalendarDragState {
    val day = day.coerceIn(0, (columns - 1).coerceAtLeast(0))
    // One pair of bounds, applied to both the snapped minute and the raw one behind it. The picture
    // is allowed to be smoother than the write; it is not allowed to show a block off the end of the
    // column, because that is a write that cannot happen.
    val bounds = when (kind) {
        DragKind.CREATE -> 0..DAY_MINUTES
        DragKind.MOVE -> {
            val subject = requireNotNull(subject) { "a move needs a subject" }
            // The delta that keeps [start + d, end + d] inside the day.
            val lo = anchorMinute - subject.startMinutes
            val hi = anchorMinute + (DAY_MINUTES - subject.endMinutes)
            minOf(lo, hi)..maxOf(lo, hi)
        }
        DragKind.RESIZE_START -> {
            val subject = requireNotNull(subject) { "a resize needs a subject" }
            val hi = anchorMinute + (subject.endMinutes - DRAG_SNAP_MINUTES - subject.startMinutes)
            (anchorMinute - subject.startMinutes)..hi
        }
        DragKind.RESIZE_END -> {
            val subject = requireNotNull(subject) { "a resize needs a subject" }
            val lo = anchorMinute - (subject.endMinutes - subject.startMinutes - DRAG_SNAP_MINUTES)
            lo..(anchorMinute + (DAY_MINUTES - subject.endMinutes))
        }
    }
    return copy(
        day = day,
        minute = minute.coerceIn(bounds.first, bounds.last),
        rawMinute = rawMinute.coerceIn(bounds.first, bounds.last),
    )
}

// ---- Turning pixels into the core's geometry ----------------------------------------------------
//
// The inverse of the multiplication in CalendarSurfaceState: that turns a day index and a minute
// into a rectangle, this turns a point back into a day index and a minute. Two directions of one
// mapping, so they live beside each other and are tested against each other.

/** The day column a **content**-space x falls in, past the hour ruler, before the day scroll. */
internal fun SurfaceMetrics.columnAt(contentX: Float): Int =
    if (dayWidth <= 0f) 0 else floor(contentX / dayWidth).toInt()

/** The wall-clock minute a **content**-space y falls on, snapped to the drag grid. */
internal fun SurfaceMetrics.minuteAt(contentY: Float): Int =
    (rawMinuteAt(contentY).toFloat() / DRAG_SNAP_MINUTES).roundToInt() * DRAG_SNAP_MINUTES

/**
 * The wall-clock minute a **content**-space y falls on, to the minute.
 *
 * What the block is drawn from while the finger is down. Never what is written, [minuteAt] is.
 */
internal fun SurfaceMetrics.rawMinuteAt(contentY: Float): Int {
    if (hourHeight <= 0f) return 0
    return (contentY / hourHeight * 60f).roundToInt()
}

/** A surface-space point in the grid, in content coordinates (the frame the blocks are drawn in). */
internal fun SurfaceMetrics.contentPoint(at: Offset, dayX: Float, scrollY: Float): Offset =
    Offset(at.x - gutter + dayX, at.y - contentTop + scrollY)

/** A block's preview rectangle, in the same content coordinates the renderer draws in. */
internal fun SurfaceMetrics.previewRect(preview: DragPreview, columns: Int): Rect {
    val width = dayWidth / columns.coerceAtLeast(1)
    val left = dayWidth * preview.day
    val top = hourHeight * (preview.startMinutes / 60f)
    return Rect(left, top, left + width, top + hourHeight * (preview.minutes / 60f))
}

// ---- Deciding what a press-and-hold meant --------------------------------------------------------

/**
 * How close to a block's edge a finger must land for the hold to be a resize rather than a move,
 * in pixels-per-dp-independent terms the caller supplies.
 *
 * Only applied when the block is tall enough that the two zones do not meet, on a fifteen-minute
 * block at a zoomed-out horizon they would cover the whole thing, and every hold would be a resize
 * of an event you cannot see the middle of.
 */
internal fun resizeZoneApplies(blockHeight: Float, edge: Float): Boolean = blockHeight >= edge * 3f

/**
 * What a press-and-hold at [at] means, or `null` if it means nothing.
 *
 * The order is the order a hand expects: a block that is **the user's own** claims the hold, its
 * edges claim it as a resize, and bare grid claims it as a create. A block that is *not* the user's
 * own claims nothing and falls through to a create, which is deliberate. A meeting somebody else
 * called cannot be re-timed here (`docs/calendar.md` §13), and rather than doing nothing at all, a
 * hold on top of one draws a new slot, exactly as a hold on empty grid does.
 *
 * Hit-tested against the **renderer's own** `blockRect`, so a finger and a screen reader and the
 * pixels all agree, the same rule §7 states for the accessibility overlay.
 */
@Suppress("ReturnCount")
internal fun PagePaint.dragAt(
    at: Offset,
    m: SurfaceMetrics,
    dayX: Float,
    scrollY: Float,
    resizeEdge: Float,
    canCreate: Boolean,
): CalendarDragState? {
    // The banner and the headings are not part of the grid: a hold there is not a drag.
    if (at.y < m.contentTop || at.x < m.gutter) return null
    val point = m.contentPoint(at, dayX, scrollY)
    val day = m.columnAt(point.x)
    if (day < 0 || day >= headings.size) return null
    val minute = m.minuteAt(point.y)

    blocks.forEach { block ->
        // A segment clipped by midnight is not the event, its visible top or bottom is an artefact
        // of the column it is drawn in, so every gesture on it would mean something other than what
        // it looks like. Left undraggable, and said so in `docs/calendar.md`'s Known gaps.
        if (!block.canMove || block.clipped) return@forEach
        val rect = m.blockRect(block)
        if (!rect.contains(point)) return@forEach
        val subject = DragSubject(
            account = block.account,
            event = block.event,
            occurrenceStart = block.occurrenceStart,
            day = block.day,
            startMinutes = block.startMinutes,
            endMinutes = block.endMinutes,
        )
        val kind = when {
            !resizeZoneApplies(rect.height, resizeEdge) -> DragKind.MOVE
            point.y - rect.top <= resizeEdge -> DragKind.RESIZE_START
            rect.bottom - point.y <= resizeEdge -> DragKind.RESIZE_END
            else -> DragKind.MOVE
        }
        // A resize anchors on the edge it grabbed, not on the finger: otherwise the first frame
        // jumps the edge to wherever inside the zone the finger happened to land.
        val anchor = when (kind) {
            DragKind.RESIZE_START -> subject.startMinutes
            DragKind.RESIZE_END -> subject.endMinutes
            else -> minute
        }
        return CalendarDragState(kind, subject, day, anchor, day, anchor)
    }

    if (!canCreate) return null
    // The raw touch rides along: the hour band a press fills is the one the *finger* is in, which the
    // snapped minute can no longer answer once it has rounded across a boundary.
    val rawMinute = m.rawMinuteAt(point.y)
    return CalendarDragState(
        kind = DragKind.CREATE,
        subject = null,
        anchorDay = day,
        anchorMinute = minute,
        day = day,
        minute = minute,
        rawMinute = rawMinute,
        rawAnchorMinute = rawMinute,
    )
}
