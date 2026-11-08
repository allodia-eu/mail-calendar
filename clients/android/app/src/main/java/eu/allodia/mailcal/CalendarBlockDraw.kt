// The grid's timed events, drawn: each block's shape (CalendarEventDraw.kt), then its label, laid out
// once per page and width (CalendarBlockLabels.kt) and only placed here.
//
// `column`/`columns` are the core's overlap solution: an event in a cluster of three sits in lane
// `column` of `columns`, so its width is that share of the day. The client never re-packs: if it
// did, two clients could column identical data differently.
package eu.allodia.mailcal

import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.geometry.Rect
import androidx.compose.ui.geometry.Size
import androidx.compose.ui.graphics.ColorFilter
import androidx.compose.ui.graphics.drawscope.DrawScope
import androidx.compose.ui.graphics.drawscope.clipRect
import androidx.compose.ui.graphics.drawscope.translate
import androidx.compose.ui.graphics.painter.Painter
import androidx.compose.ui.text.TextMeasurer
import androidx.compose.ui.text.drawText
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.dp
import kotlin.math.floor

@Suppress("LongParameterList")
internal fun DrawScope.drawBlocks(
    page: PagePaint,
    m: SurfaceMetrics,
    theme: SurfaceTheme,
    measurer: TextMeasurer,
    scroll: SurfaceScroll,
    contentTop: Float,
    shapeWidth: Float,
    drag: CalendarDragState?,
) {
    // The block being dragged is drawn once, by `drawDrag`, at the time the finger is over, never
    // twice. Leaving the original in place reads as a duplicate event.
    val held = drag?.subject
    val hourHeightDp = m.hourHeight.toDp()
    val top = scroll.scrollY
    val bottom = scroll.scrollY + (m.height - contentTop)
    val left = scroll.dayX
    val right = scroll.dayX + m.dayViewport
    val started = if (CalendarTrace.on) System.nanoTime() else 0L
    var drawn = 0
    var culled = 0
    val gap = 1.dp.toPx()
    val corner = CORNER_RADIUS.toPx()

    page.blocks.forEachIndexed { index, block ->
        if (held != null && block.account == held.account && block.event == held.event &&
            block.day == held.day
        ) {
            return@forEachIndexed
        }
        val rect = m.blockRect(block)
        if (rect.bottom <= top || rect.top >= bottom || rect.right <= left || rect.left >= right) {
            culled++
            return@forEachIndexed
        }
        drawn++
        val body = Rect(rect.left + gap, rect.top + gap, rect.right - gap, rect.bottom - gap)
        if (body.width <= 0f || body.height <= 0f) return@forEachIndexed
        drawEventShape(body, block.fill, block.accent, block.awaitingResponse, corner)
        drawBlockLabel(index, block, page.labels, rect, body, theme, measurer, hourHeightDp, shapeWidth)
    }
    if (CalendarTrace.on) CalendarTrace.frame(drawn, culled, System.nanoTime() - started)
}

/**
 * The title, wrapped over every whole line the block has room for, the time beneath it, and the
 * corner glyphs. A block too short for one line draws none: a title cut through the middle reads as
 * a broken grid, and pinching in brings it back. Its spoken label is unconditional.
 *
 * Laid out against [shapeWidth], frozen for the length of a pinch, and clipped to the live [body]:
 * mid-pinch those differ, so the rectangle tracks the fingers while the text stays as it was.
 */
@Suppress("LongParameterList")
private fun DrawScope.drawBlockLabel(
    index: Int,
    block: BlockPaint,
    labels: BlockLabels,
    rect: Rect,
    body: Rect,
    theme: SurfaceTheme,
    measurer: TextMeasurer,
    hourHeight: Dp,
    shapeWidth: Float,
) {
    // The line box as drawn, which the font scale grows past its nominal dp.
    val line = blockLabelLineHeight(block.minutes).toDp()
    val lines = blockTitleLines(block.minutes, hourHeight, line = line)
    if (lines == 0) return
    val start = EVENT_LABEL_START.toPx()
    val end = EVENT_LABEL_PAD.toPx()
    val room = shapeWidth / block.columns - (rect.width - body.width) - start - end
    val step = floor(room / LABEL_STEP).toInt()
    if (step <= 0) return
    val withTime = blockShowsTime(block.minutes, hourHeight, line)
    val label = labels.held(index, step, lines, withTime)
        ?: layOutBlockLabel(block, measurer, step, lines, withTime).also { labels.hold(index, it) }

    val left = body.left + start
    val textTop = rect.top + blockInset(block.minutes).toPx()
    val textRight = body.right - end
    clipRect(body.left, body.top, body.right, body.bottom) {
        var titleRight = textRight
        if (label.glyphs) {
            titleRight -= label.glyphsWidth + CORNER_GAP.toPx()
            val side = cornerGlyphSide(block)
            val glyphTop = textTop + (blockLabelLineHeight(block.minutes).toPx() - side) / 2f
            drawCornerGlyphs(block, theme, textRight - label.glyphsWidth, glyphTop, side)
        }
        clipRect(left, textTop, titleRight, textTop + label.titleHeight) {
            drawText(label.title, topLeft = Offset(left, textTop))
        }
        label.time?.let { drawText(it, topLeft = Offset(left, textTop + label.titleHeight)) }
    }
}

/** The meeting glyph, then the repeat glyph, from [x] rightwards, in the title's colour. */
private fun DrawScope.drawCornerGlyphs(
    block: BlockPaint,
    theme: SurfaceTheme,
    x: Float,
    top: Float,
    side: Float,
) {
    var at = x
    if (block.hasConference) {
        drawGlyph(theme.camera, at, top, side, block.glyphTint)
        at += side + CORNER_GAP.toPx()
    }
    if (block.repeats) drawGlyph(theme.repeat, at, top, side, block.glyphTint)
}

private fun DrawScope.drawGlyph(glyph: Painter, x: Float, y: Float, side: Float, tint: ColorFilter?) {
    translate(left = x, top = y) {
        with(glyph) { draw(Size(side, side), colorFilter = tint) }
    }
}
