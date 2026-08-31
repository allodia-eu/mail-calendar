// How an unanswered invitation looks on the calendar: a dashed border, a hatched leading gutter, and
// a fill that reads as provisional rather than booked.
//
// The core decides *which* records are holds (`participation == NEEDS_ACTION`, docs/invitations.md);
// this file is only the drawing, kept in one place so the grid block, the all-day bar, the month
// chip and the invitation card's preview cannot drift apart.
//
// Every piece here is a no-op on an answered record, so nothing about a confirmed commitment's
// appearance changes: a hold is told apart by shape, not by a restyle of everything around it.
//
// The visual is never the whole disclosure. A dashed border is invisible to a screen reader, so
// every surface that draws a hold also says it, `calendarEventLabel` in InvitationFormat.kt appends
// "Awaiting your response" (docs/calendar.md §4, the spoken-grid rule).
package eu.allodia.mailcal

import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.drawWithContent
import androidx.compose.ui.geometry.CornerRadius
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.geometry.Rect
import androidx.compose.ui.geometry.Size
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.PathEffect
import androidx.compose.ui.graphics.drawscope.DrawScope
import androidx.compose.ui.graphics.drawscope.Stroke
import androidx.compose.ui.graphics.drawscope.clipRect
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.dp

/**
 * How much of a hold's colour survives.
 *
 * Enough to keep its calendar identifiable, little enough that it does not read as a confirmed
 * commitment beside one. Applied when the page is built, never inside a frame, a pinch must not
 * re-derive a colour (CalendarSurfaceModel.kt).
 */
internal const val HOLD_FILL_ALPHA = 0.4f

/** The stripe width of the hatched gutter, and how far apart the diagonals sit. */
private val HOLD_GUTTER = 4.dp
private val HOLD_HATCH_STEP = 4.dp

/** The dash the hold's border is stroked with, on, then off. */
private val HOLD_DASH_ON = 3.dp
private val HOLD_DASH_OFF = 2.dp

/** A colour faded to [HOLD_FILL_ALPHA] on a hold, and untouched on a commitment. */
internal fun Color.holdFill(awaiting: Boolean): Color =
    if (awaiting) copy(alpha = alpha * HOLD_FILL_ALPHA) else this

/**
 * The border a record's edge is stroked with: dashed for an unanswered hold, the grid's own hairline
 * otherwise. A record already carrying a border strokes it with this rather than gaining a second.
 */
internal fun DrawScope.participationStroke(awaiting: Boolean): Stroke = Stroke(
    width = 1.dp.toPx(),
    pathEffect = if (awaiting) {
        PathEffect.dashPathEffect(floatArrayOf(HOLD_DASH_ON.toPx(), HOLD_DASH_OFF.toPx()))
    } else {
        null
    },
)

/**
 * The diagonal hatching down a hold's leading edge, the part of the treatment that survives being
 * looked at quickly, when a dashed border at a phone's hour height does not.
 *
 * Draws nothing unless [awaiting], so a commitment costs one boolean test.
 */
internal fun DrawScope.drawHoldHatch(rect: Rect, color: Color, awaiting: Boolean) {
    if (!awaiting) return
    val width = minOf(HOLD_GUTTER.toPx(), rect.width)
    if (width <= 0f || rect.height <= 0f) return
    val step = HOLD_HATCH_STEP.toPx()
    clipRect(rect.left, rect.top, rect.left + width, rect.bottom) {
        // Start a full height to the left so the first stripe already crosses the strip.
        var x = rect.left - rect.height
        while (x < rect.left + width + rect.height) {
            drawLine(
                color = color,
                start = Offset(x, rect.bottom),
                end = Offset(x + rect.height, rect.top),
                strokeWidth = 1.dp.toPx(),
            )
            x += step
        }
    }
}

/**
 * The whole hold treatment for one rounded rectangle: the hatched gutter, then the dashed edge.
 *
 * One call so the grid block, the all-day bar and the preview cannot each remember a different half
 * of it. A no-op on a commitment.
 */
internal fun DrawScope.drawHold(rect: Rect, color: Color, corner: Float, awaiting: Boolean) {
    if (!awaiting) return
    drawHoldHatch(rect, color, awaiting = true)
    // Inset by half the stroke, which straddles the path it is given: drawn on the boundary its
    // outer half falls outside the chip and is clipped away, leaving a half-weight dash that reads
    // as a rendering artefact rather than as a deliberate edge.
    val half = 1.dp.toPx() / 2f
    if (rect.width <= half * 2 || rect.height <= half * 2) return
    drawRoundRect(
        color = color,
        topLeft = Offset(rect.left + half, rect.top + half),
        size = Size(rect.width - half * 2, rect.height - half * 2),
        cornerRadius = CornerRadius(corner),
        style = participationStroke(awaiting = true),
    )
}

/**
 * The same treatment for a composed surface rather than a canvas, the month grid's chips, which are
 * `Text`s with a background rather than draw calls.
 *
 * A no-op on a commitment, so a month cell of ordinary appointments is byte-for-byte what it was.
 */
internal fun Modifier.holdChip(awaiting: Boolean, edge: Color, cornerRadius: Dp): Modifier =
    if (!awaiting) {
        this
    } else {
        drawWithContent {
            drawContent()
            // The same call the canvas grid makes, so the two surfaces cannot draw a hold differently.
            drawHold(Rect(Offset.Zero, size), edge, cornerRadius.toPx(), awaiting = true)
        }
    }
