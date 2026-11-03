// An event's title on a block or bar, with the camera after it when the event has a video meeting
// (docs/conferencing.md, rule 7). The title gives up the camera's room first, so the camera stays
// in sight; a block too narrow for both draws the title alone, and its spoken label still says
// "video meeting".
package eu.allodia.mailcal

import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.geometry.Size
import androidx.compose.ui.graphics.ColorFilter
import androidx.compose.ui.graphics.drawscope.DrawScope
import androidx.compose.ui.graphics.drawscope.translate
import androidx.compose.ui.graphics.painter.Painter
import androidx.compose.ui.text.TextLayoutResult
import androidx.compose.ui.text.TextMeasurer
import androidx.compose.ui.text.TextStyle
import androidx.compose.ui.text.drawText
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.isSpecified
import androidx.compose.ui.unit.sp

/** Below this much room for the title itself, the camera gives way to the title. */
private val MIN_TITLE_ROOM = 16.dp

/** Between the end of the title and the camera. */
private val GLYPH_GAP = 3.dp

/** A title, measured, and the camera that follows it when there is one and room for it. */
internal class TitleLine(
    val layout: TextLayoutResult,
    val camera: Painter?,
    val style: TextStyle,
)

/**
 * Measures [text] into [room], leaving the camera's width free when [camera] is set: the caller
 * passes it only for an event that has a meeting.
 */
@Suppress("LongParameterList")
internal fun DrawScope.titleLine(
    measurer: TextMeasurer,
    text: String,
    style: TextStyle,
    room: Float,
    maxLines: Int = 1,
    camera: Painter? = null,
): TitleLine {
    val reserve = glyphSize(style) + GLYPH_GAP.toPx()
    val drawn = camera?.takeIf { room - reserve >= MIN_TITLE_ROOM.toPx() }
    val layout = measurer.line(text, style, if (drawn != null) room - reserve else room, maxLines)
    return TitleLine(layout, drawn, style)
}

/** Draws [line] at [topLeft], the camera directly after its last line as drawn. */
internal fun DrawScope.drawTitleLine(line: TitleLine, topLeft: Offset) {
    drawText(line.layout, topLeft = topLeft)
    val camera = line.camera ?: return
    val layout = line.layout
    val last = layout.lineCount - 1
    val glyph = glyphSize(line.style)
    // After the text as drawn, which an ellipsis has already shortened to fit.
    val x = topLeft.x + layout.getLineRight(last) + GLYPH_GAP.toPx()
    val y = topLeft.y + (layout.getLineTop(last) + layout.getLineBottom(last) - glyph) / 2f
    translate(left = x, top = y) {
        with(camera) {
            draw(Size(glyph, glyph), colorFilter = ColorFilter.tint(line.style.color))
        }
    }
}

// The camera at the title's own type size, so it reads as part of the line.
private fun DrawScope.glyphSize(style: TextStyle): Float =
    (if (style.fontSize.isSpecified) style.fontSize else 11.sp).toPx()
