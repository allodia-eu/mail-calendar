// Drawing the drag in flight, the block in the hand, and the readout that says where it will land.
//
// Split from CalendarSurfaceDraw for size alone; it is the same draw pass and runs inside the same
// clip and translate, in the grid's content coordinates.
package eu.allodia.mailcal

import androidx.compose.ui.geometry.CornerRadius
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.geometry.Rect
import androidx.compose.ui.geometry.Size
import androidx.compose.ui.graphics.drawscope.DrawScope
import androidx.compose.ui.graphics.drawscope.Stroke
import androidx.compose.ui.graphics.drawscope.clipRect
import androidx.compose.ui.text.TextMeasurer
import androidx.compose.ui.text.drawText
import androidx.compose.ui.unit.dp

// The drag in flight: the block where the finger has it, and a floating readout of the time it would
// land on.
//
// Two currencies, on purpose. The block is drawn from `livePreview()`, which follows the finger to
// the minute, so the motion is smooth instead of stepping a quarter-hour at a time. The readout is
// drawn from `preview()`, which is snapped, it is the number that will actually be written, and a
// readout that agreed with the pixels instead would be quoting a minute the drop cannot honour.
//
// Drawn *after* the blocks and the now line, so a held block is never buried under the grid it is
// being dragged over.
@Suppress("LongParameterList")
internal fun DrawScope.drawDrag(
    drag: CalendarDragState,
    page: PagePaint,
    m: SurfaceMetrics,
    theme: SurfaceTheme,
    strings: SurfaceStrings,
    measurer: TextMeasurer,
    shapeWidth: Float,
    scroll: SurfaceScroll,
    contentTop: Float,
) {
    // A held block takes its whole column: the core's lane packing describes where it *was*, and
    // re-solving overlaps per frame is exactly the kind of work §7 forbids inside a gesture.
    val rect = m.previewRect(drag.livePreview(), columns = 1)
    val held = drag.subject?.let { subject ->
        page.blocks.firstOrNull {
            it.account == subject.account && it.event == subject.event && it.day == subject.day
        }
    }
    val gap = 1.dp.toPx()
    val corner = CornerRadius(CORNER_RADIUS.toPx())
    val body = Rect(rect.left + gap, rect.top + gap, rect.right - gap, rect.bottom - gap)
    if (body.width <= 0f || body.height <= 0f) return

    // A new slot wears the calendar it would be filed on; a held one keeps its own colours.
    val fill = held?.background ?: theme.dragFill
    val edge = held?.border ?: theme.dragBorder
    drawRoundRect(fill, Offset(body.left, body.top), Size(body.width, body.height), corner)
    drawRoundRect(
        color = edge,
        topLeft = Offset(body.left, body.top),
        size = Size(body.width, body.height),
        cornerRadius = corner,
        style = androidx.compose.ui.graphics.drawscope.Stroke(width = 2.dp.toPx()),
    )

    val settled = drag.preview()
    drawDragReadout(
        text = timeRange(settled.startMinutes, settled.endMinutes, strings.use24Hour),
        body = body,
        // What is actually on screen, in the content coordinates this is drawn in. The readout is
        // held inside it rather than beside the block, because the block can be at the very top of
        // the viewport with the finger on it, and a readout clipped away is exactly the failure the
        // in-block label used to have on a short slot.
        visible = Rect(
            left = scroll.dayX,
            top = scroll.scrollY,
            right = scroll.dayX + m.dayViewport,
            bottom = scroll.scrollY + (m.height - contentTop).coerceAtLeast(0f),
        ),
        theme = theme,
        measurer = measurer,
        shapeWidth = shapeWidth,
    )
}

// The time a drag would land on, in a floating pill beside the block it belongs to.
//
// It floats rather than being written *inside* the block for two reasons the in-block label could not
// solve: a fifteen-minute slot at a zoomed-out horizon is a few pixels tall, so the one label that
// tells a 15-minute snap from a 30-minute one was dropped exactly when it was needed most; and the
// block itself is now smooth, so the label is the only thing on screen still quoting the write.
@Suppress("LongParameterList")
private fun DrawScope.drawDragReadout(
    text: String,
    body: Rect,
    visible: Rect,
    theme: SurfaceTheme,
    measurer: TextMeasurer,
    shapeWidth: Float,
) {
    val padH = 8.dp.toPx()
    val padV = 4.dp.toPx()
    val line = measurer.line(text, theme.pillLabel, shapeWidth * 2f)
    val w = line.size.width + padH * 2
    val h = line.size.height + padV * 2
    val margin = 6.dp.toPx()
    if (visible.width < w || visible.height < h) return

    // Centred on the block, then held on screen, a slot drawn in the first or last column must not
    // push its own readout off the side.
    val left = (body.center.x - w / 2f).coerceIn(visible.left, visible.right - w)
    // Above the block by preference, that is the side the hand is not on, and below it when the
    // block is against the top of the screen and there is no room up there.
    val above = body.top - margin - h
    val top = (if (above >= visible.top) above else body.bottom + margin)
        .coerceIn(visible.top, visible.bottom - h)
    drawRoundRect(
        color = theme.pillFill,
        topLeft = Offset(left, top),
        size = Size(w, h),
        cornerRadius = CornerRadius(h / 2f),
    )
    drawText(line, topLeft = Offset(left + padH, top + padV))
}
