// How an event wears its calendar's colour on every surface that draws one: the grid's blocks, the
// all-day bars, the month chips and the invitation card's preview (docs/calendar.md §1). A tint
// fills it, the colour itself runs down its leading edge, and an unanswered hold swaps that bar for
// a hatched edge and a dashed border instead (CalendarParticipation.kt). One call per surface kind,
// so no two of them can wear a colour differently.
package eu.allodia.mailcal

import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.drawBehind
import androidx.compose.ui.geometry.CornerRadius
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.geometry.Rect
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.drawscope.DrawScope
import androidx.compose.ui.graphics.drawscope.clipRect
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.dp

/** The bar of the calendar's colour down an event's leading edge. */
internal val EVENT_BAR = 3.dp

/** Between the bar and the label, and between the label and the trailing edge. */
internal val EVENT_LABEL_PAD = 4.dp

/**
 * One event's shape in [rect]: the [fill], then the [accent] bar down the leading edge, or for an
 * unanswered hold ([awaiting]) the hatched edge and the dashed border instead. [fill] arrives already
 * faded for a hold, so nothing here derives a colour inside a frame.
 */
internal fun DrawScope.drawEventShape(
    rect: Rect,
    fill: Color,
    accent: Color,
    awaiting: Boolean,
    corner: Float,
) {
    if (rect.width <= 0f || rect.height <= 0f) return
    val radius = CornerRadius(corner)
    drawRoundRect(fill, rect.topLeft, rect.size, radius)
    if (awaiting) {
        drawHold(rect, accent, corner, awaiting = true)
        return
    }
    // The block's own rounded shape, cut to the bar's width, so the bar follows the corners.
    val bar = minOf(EVENT_BAR.toPx(), rect.width)
    clipRect(rect.left, rect.top, rect.left + bar, rect.bottom) {
        drawRoundRect(accent, rect.topLeft, rect.size, radius)
    }
}

/** The same shape behind a composed chip: the month grid's, which are `Text`s rather than draw calls. */
internal fun Modifier.eventChip(
    fill: Color,
    accent: Color,
    awaiting: Boolean,
    cornerRadius: Dp,
): Modifier = drawBehind {
    drawEventShape(Rect(Offset.Zero, size), fill, accent, awaiting, cornerRadius.toPx())
}

/** The leading inset of an event's label: past the bar, then the label's own padding. */
internal val EVENT_LABEL_START: Dp get() = EVENT_BAR + EVENT_LABEL_PAD
