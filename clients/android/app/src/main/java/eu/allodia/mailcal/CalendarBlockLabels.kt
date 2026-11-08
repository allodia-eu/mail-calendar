// A block's label: its title wrapped by word over every whole line the block has room for, the time
// beneath it, and the meeting and repeat glyphs in the top-right corner (docs/calendar.md §4).
//
// Laid out once per page and width, never per frame (§7). Each block keeps the last label it was
// given with the room and line count it was made for, and a frame asking for the same is handed it
// back. A pinch freezes the width text is shaped against, so a zoom lays a title out again only when
// it crosses a whole line, a handful of times per gesture.
package eu.allodia.mailcal

import androidx.compose.ui.text.TextLayoutResult
import androidx.compose.ui.text.TextMeasurer
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.Constraints
import androidx.compose.ui.unit.Density
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.isSpecified
import androidx.compose.ui.unit.sp
import kotlin.math.floor

/** The narrowest a title may be squeezed to make room for the corner glyphs. */
internal val MIN_TITLE_WIDTH = 36.dp

/** Between two corner glyphs, and between the glyphs and the title. */
internal val CORNER_GAP = 3.dp

/** The step a label's room is floored to, so a fraction of a pixel of zoom finds the same label. */
internal const val LABEL_STEP = 8f

/**
 * How many lines a block's title may wrap over: every whole line it has room for, less the one its
 * time takes when it draws one. At least one wherever a label fits at all, zero where none does.
 */
internal fun blockTitleLines(minutes: Int, hourHeight: Dp, withTime: Boolean = true): Int {
    if (!blockShowsLabel(minutes, hourHeight)) return 0
    val whole = floor(
        blockLabelSpace(minutes, hourHeight).value / blockLabelLineHeight(minutes).value,
    ).toInt()
    val time = withTime && blockShowsTime(minutes, hourHeight)
    return maxOf(whole - if (time) 1 else 0, 1)
}

/** The width of [count] corner glyphs of side [glyph], side by side, [gap] apart. */
internal fun cornerGlyphsWidth(count: Int, glyph: Float, gap: Float): Float =
    if (count <= 0) 0f else count * glyph + (count - 1) * gap

/**
 * Whether the corner glyphs are drawn in a label [width] wide: only while the title keeps at least
 * [minTitle] of its own beside them, since a glyph is worth less than the name of the event.
 */
internal fun cornerGlyphsFit(width: Float, glyphsWidth: Float, gap: Float, minTitle: Float): Boolean =
    glyphsWidth > 0f && width - glyphsWidth - gap >= minTitle

/**
 * Whether a title laid out with lines ending at [lineEnds] broke a word across two lines. A break
 * after a space or a hyphen is a wrap; one between two letters or digits is a broken word, which
 * reads as a broken grid.
 */
internal fun breaksAWord(text: String, lineEnds: List<Int>): Boolean = lineEnds.any { end ->
    end in 1 until text.length && text[end - 1].isLetterOrDigit() && text[end].isLetterOrDigit()
}

/** One block's label, laid out for one room and line count. */
internal class BlockLabel(
    val step: Int,
    val lines: Int,
    val withTime: Boolean,
    val title: TextLayoutResult,
    /** Down to the bottom of the last whole line drawn, where the title is cut and the time begins. */
    val titleHeight: Float,
    /** The range where it fits, the start alone where it does not, or `null` with no time line. */
    val time: TextLayoutResult?,
    /** Whether the corner glyphs are drawn. */
    val glyphs: Boolean,
    val glyphsWidth: Float,
)

/** The labels of one page's blocks, by the block's index in the page. Drawn on the UI thread only. */
internal class BlockLabels(count: Int) {
    private val held = arrayOfNulls<BlockLabel>(count)

    fun held(index: Int, step: Int, lines: Int, withTime: Boolean): BlockLabel? =
        held.getOrNull(index)?.takeIf { it.step == step && it.lines == lines && it.withTime == withTime }

    fun hold(index: Int, label: BlockLabel) {
        if (index in held.indices) held[index] = label
    }
}

/**
 * Lays [block]'s label out for a room of [step] × [LABEL_STEP] pixels and [lines] title lines.
 *
 * The title wraps by word over all its lines and is cut after the last whole one the block has room
 * for; where a word is wider than the room it would be broken, so the title runs on as one line cut
 * at the block's edge instead.
 */
@Suppress("LongParameterList")
internal fun Density.layOutBlockLabel(
    block: BlockPaint,
    measurer: TextMeasurer,
    step: Int,
    lines: Int,
    withTime: Boolean,
): BlockLabel {
    val room = step * LABEL_STEP
    val gap = CORNER_GAP.toPx()
    val glyphsWidth = cornerGlyphsWidth(block.cornerGlyphs, cornerGlyphSide(block), gap)
    val glyphs = cornerGlyphsFit(room, glyphsWidth, gap, MIN_TITLE_WIDTH.toPx())
    val available = if (glyphs) room - glyphsWidth - gap else room
    val wrapWidth = (floor(available / LABEL_STEP) * LABEL_STEP).toInt().coerceAtLeast(0)

    val wrapped = if (wrapWidth > 0) {
        CalendarTrace.measured()
        measurer.measure(
            text = block.title,
            style = block.titleStyle,
            overflow = TextOverflow.Clip,
            softWrap = true,
            constraints = Constraints(maxWidth = wrapWidth),
        ).takeUnless { layout ->
            breaksAWord(block.title, (0 until layout.lineCount).map { layout.getLineEnd(it) })
        }
    } else {
        null
    }
    val oneLine = wrapped == null
    val title = wrapped ?: run {
        CalendarTrace.measured()
        measurer.measure(
            text = block.title,
            style = block.titleStyle,
            overflow = TextOverflow.Clip,
            softWrap = false,
            maxLines = 1,
        )
    }
    val shown = if (oneLine) 1 else minOf(lines, title.lineCount)
    val titleHeight = title.getLineBottom((shown - 1).coerceAtLeast(0))

    val time = if (withTime) {
        CalendarTrace.measured()
        val range = measurer.measure(block.range, block.clockStyle, softWrap = false, maxLines = 1)
        if (range.size.width <= room) range else measurer.line(block.clock, block.clockStyle, room)
    } else {
        null
    }
    return BlockLabel(step, lines, withTime, title, titleHeight, time, glyphs, glyphsWidth)
}

/** A corner glyph's side: the title's own type size, so it reads as part of the line. */
internal fun Density.cornerGlyphSide(block: BlockPaint): Float {
    val size = block.titleStyle.fontSize
    return (if (size.isSpecified) size else 11.sp).toPx()
}
