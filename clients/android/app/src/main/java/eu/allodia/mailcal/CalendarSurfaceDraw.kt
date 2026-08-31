// The grid, drawn.
//
// One page, headings, all-day banner, hour ruler, gridlines, event blocks, the now line, is a
// sequence of draw calls into a single canvas, where it used to be a tree of composables with one
// `Box` per event. The old grid's sixty-odd blocks were laid out and recomposed on **every frame of a
// pinch**, three pages of them, on the UI thread. Nothing here allocates: every colour, string and
// text style arrived precomputed in [PagePaint], and all a frame does is multiply the core's unit-free
// geometry, a day index, a wall-clock minute, a column fraction, by an hour height and a column
// width, and cull whatever falls outside the viewport.
//
// A page's stride is the whole surface width, hour ruler included, because a page **is** a week and
// the ruler belongs to it: turning the week slides the ruler out with its own days.
package eu.allodia.mailcal

import androidx.compose.ui.geometry.CornerRadius
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.geometry.Rect
import androidx.compose.ui.geometry.Size
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.drawscope.DrawScope
import androidx.compose.ui.graphics.drawscope.clipRect
import androidx.compose.ui.graphics.drawscope.translate
import androidx.compose.ui.text.TextLayoutResult
import androidx.compose.ui.text.TextMeasurer
import androidx.compose.ui.text.TextStyle
import androidx.compose.ui.text.drawText
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.Constraints
import androidx.compose.ui.unit.dp
import kotlin.math.ceil
import kotlin.math.floor

/** Where the grid is scrolled, in pixels, the three numbers a frame reads. */
internal data class SurfaceScroll(
    val scrollY: Float,
    val dayX: Float,
    val pageOffset: Float,
)

/** The circle behind today's date, and the dot on the now line. */
private val TODAY_DIAMETER = 28.dp
private val NOW_DOT = 8.dp

/**
 * Draws one week at [pageX], the page's own left edge, in surface coordinates.
 *
 * [todayIndex] is `-1` unless one of *this page's* columns actually is today: paging away from this
 * week must hide the now line, not draw it on an arbitrary Wednesday.
 */
@Suppress("LongParameterList")
internal fun DrawScope.drawCalendarPage(
    page: PagePaint,
    m: SurfaceMetrics,
    theme: SurfaceTheme,
    strings: SurfaceStrings,
    measurer: TextMeasurer,
    scroll: SurfaceScroll,
    pageX: Float,
    expanded: Boolean,
    todayIndex: Int,
    nowMinutes: Int,
    /**
     * The column width the **text** is laid out against, the live one at rest, and a frozen one for
     * the length of a pinch. Never the geometry: every rectangle below still scales every frame.
     */
    shapeWidth: Float,
    /** The drag in flight on **this** page, or `null`. */
    drag: CalendarDragState? = null,
) {
    if (page.headings.isEmpty()) return
    // Entirely off the side: a swipe only ever shows two of the three pages we hold.
    if (pageX >= size.width || pageX + m.width <= 0f) return

    val bannerLanes = allDayBannerLanes(page.lanes, expanded)
    val contentTop = m.contentTopFor(page.lanes, expanded)
    val bannerTop = m.viewport.headerHeight
    val bannerHeight = m.viewport.laneHeight * bannerLanes

    translate(left = pageX, top = 0f) {
        drawHeader(page, m, theme, strings, measurer, scroll.dayX, todayIndex, shapeWidth)
        if (bannerLanes > 0) {
            drawBanner(
                page, m, theme, strings, measurer,
                scroll.dayX, bannerTop, bannerHeight, expanded, shapeWidth,
            )
        }
        // The seam between the chrome and the grid.
        drawLine(
            color = theme.outlineVariant,
            start = Offset(0f, contentTop),
            end = Offset(m.width, contentTop),
            strokeWidth = 1f,
        )
        drawHourRuler(m, theme, strings, measurer, scroll.scrollY, contentTop)
        clipRect(left = m.gutter, top = contentTop, right = m.width, bottom = m.height) {
            translate(left = m.gutter - scroll.dayX, top = contentTop - scroll.scrollY) {
                drawGridLines(page.headings.size, m, theme)
                drawBlocks(page, m, theme, measurer, scroll, contentTop, shapeWidth, drag)
                if (todayIndex >= 0) drawNowLine(m, theme, todayIndex, nowMinutes)
                if (drag != null) {
                    drawDrag(drag, page, m, theme, strings, measurer, shapeWidth, scroll, contentTop)
                }
            }
        }
        // **`isMaterialized == false` does not mean "no events".** It means the engine has not looked
        // this far yet, and a confidently empty week is a lie that looks exactly like a real answer.
        if (!page.isMaterialized) {
            drawLoading(m, theme, strings, measurer, contentTop)
        }
    }
}

// The column headings: the ISO week number in the gutter (a Dutch/German convention worth keeping),
// then each day's weekday and date, with today's number circled.
@Suppress("LongParameterList")
private fun DrawScope.drawHeader(
    page: PagePaint,
    m: SurfaceMetrics,
    theme: SurfaceTheme,
    strings: SurfaceStrings,
    measurer: TextMeasurer,
    dayX: Float,
    todayIndex: Int,
    shapeWidth: Float,
) {
    val header = m.viewport.headerHeight
    val wk = measurer.line(strings.weekShort, theme.weekLabel, m.gutter)
    val number = measurer.line("${page.weekNumber}", theme.weekNumber, m.gutter)
    val stack = wk.size.height + number.size.height
    var y = (header - stack) / 2f
    drawText(wk, topLeft = Offset((m.gutter - wk.size.width) / 2f, y))
    y += wk.size.height
    drawText(number, topLeft = Offset((m.gutter - number.size.width) / 2f, y))

    clipRect(left = m.gutter, top = 0f, right = m.width, bottom = header) {
        translate(left = m.gutter - dayX, top = 0f) {
            page.headings.forEachIndexed { index, heading ->
                if (!columnVisible(index, m, dayX)) return@forEachIndexed
                val today = index == todayIndex
                val centre = m.dayWidth * index + m.dayWidth / 2f
                val weekday = measurer.line(
                    heading.weekday,
                    if (today) theme.weekdayToday else theme.weekday,
                    shapeWidth,
                )
                val date = measurer.line(
                    heading.dayOfMonth,
                    if (today) theme.dayNumberToday else theme.dayNumber,
                    shapeWidth,
                )
                val diameter = TODAY_DIAMETER.toPx()
                val block = weekday.size.height + diameter
                val top = (m.viewport.headerHeight - block) / 2f
                drawText(weekday, topLeft = Offset(centre - weekday.size.width / 2f, top))
                val circle = top + weekday.size.height + diameter / 2f
                if (today) {
                    drawCircle(theme.primary, radius = diameter / 2f, center = Offset(centre, circle))
                }
                drawText(
                    date,
                    topLeft = Offset(
                        centre - date.size.width / 2f,
                        circle - date.size.height / 2f,
                    ),
                )
            }
        }
    }
}

// The band above the grid: all-day and multi-day events, each spanning whole day columns. Past the
// cap the last row is given over to a per-day "+N" chip, so a busy week never grows a banner that
// swallows the grid, and nothing is hidden without saying so.
@Suppress("LongParameterList")
private fun DrawScope.drawBanner(
    page: PagePaint,
    m: SurfaceMetrics,
    theme: SurfaceTheme,
    strings: SurfaceStrings,
    measurer: TextMeasurer,
    dayX: Float,
    top: Float,
    height: Float,
    expanded: Boolean,
    shapeWidth: Float,
) {
    val label = measurer.line(strings.allDay, theme.allDay, m.gutter - 6.dp.toPx())
    drawText(
        label,
        topLeft = Offset(
            m.gutter - label.size.width - 6.dp.toPx(),
            top + (height - label.size.height) / 2f,
        ),
    )

    val drawn = allDayDrawnLanes(page.lanes, expanded)
    clipRect(left = m.gutter, top = top, right = m.width, bottom = top + height) {
        translate(left = m.gutter - dayX, top = top) {
            page.bands.forEach { band ->
                if (band.span.lane >= drawn) return@forEach
                val rect = m.bandRect(band.span)
                if (rect.right <= dayX || rect.left >= dayX + m.dayViewport) return@forEach
                drawChip(
                    rect, shapeWidth * band.span.days, band.background, band.title,
                    band.titleStyle, measurer, band.awaitingResponse, band.border,
                )
            }
            if (!expanded) {
                page.moreLabels.forEachIndexed { day, text ->
                    if (text.isEmpty()) return@forEachIndexed
                    val rect = m.moreRect(day, drawn)
                    if (rect.right <= dayX || rect.left >= dayX + m.dayViewport) return@forEachIndexed
                    drawChip(rect, shapeWidth, Color.Transparent, text, theme.more, measurer)
                }
            }
        }
    }
}

// One all-day bar (or "+N" chip): a rounded fill with a single line of text inside it. A bar for an
// unanswered invitation gains the hold treatment, a dashed edge and a hatched leading gutter, which
// it has none of otherwise (a commitment's bar carries no border at all).
@Suppress("LongParameterList")
private fun DrawScope.drawChip(
    rect: Rect,
    shapeWidth: Float,
    fill: Color,
    text: String,
    style: TextStyle,
    measurer: TextMeasurer,
    awaiting: Boolean = false,
    edge: Color = Color.Transparent,
) {
    val pad = 1.dp.toPx()
    val inner = 5.dp.toPx()
    if (fill != Color.Transparent) {
        drawRoundRect(
            color = fill,
            topLeft = Offset(rect.left + pad, rect.top + pad),
            size = Size(rect.width - pad * 2, rect.height - pad * 2),
            cornerRadius = CornerRadius(CORNER_RADIUS.toPx()),
        )
    }
    drawHold(
        rect = Rect(rect.left + pad, rect.top + pad, rect.right - pad, rect.bottom - pad),
        color = edge,
        corner = CORNER_RADIUS.toPx(),
        awaiting = awaiting,
    )
    val room = shapeWidth - pad * 2 - inner * 2
    if (room <= 0f) return
    val line = measurer.line(text, style, room)
    clipRect(rect.left + pad, rect.top + pad, rect.right - pad, rect.bottom - pad) {
        drawText(
            line,
            topLeft = Offset(
                rect.left + pad + inner,
                rect.top + (rect.height - line.size.height) / 2f,
            ),
        )
    }
}

// The hour ruler. Each label straddles its gridline (lifted half a line box) so "09" reads as the
// 09:00 boundary rather than as a name for the band beneath it. Midnight is left blank, its label
// would collide with the day headings directly above.
private fun DrawScope.drawHourRuler(
    m: SurfaceMetrics,
    theme: SurfaceTheme,
    strings: SurfaceStrings,
    measurer: TextMeasurer,
    scrollY: Float,
    contentTop: Float,
) {
    clipRect(left = 0f, top = contentTop, right = m.gutter, bottom = m.height) {
        translate(left = 0f, top = contentTop - scrollY) {
            val first = floor(scrollY / m.hourHeight).toInt().coerceAtLeast(1)
            val last = ceil((scrollY + m.contentHeight) / m.hourHeight).toInt()
                .coerceAtMost(HOURS_IN_DAY - 1)
            for (hour in first..last) {
                val text = strings.hours[hour]
                if (text.isEmpty()) continue
                val line = measurer.line(text, theme.hour, m.gutter - 6.dp.toPx())
                drawText(
                    line,
                    topLeft = Offset(
                        m.gutter - line.size.width - 6.dp.toPx(),
                        m.hourHeight * hour - line.size.height / 2f,
                    ),
                )
            }
        }
    }
}

// The hour lines and the day dividers, 23 and 6 of them, drawn unconditionally and left to the clip.
// Culling them would cost more arithmetic than the lines themselves; the blocks are where culling
// earns its keep, because a block carries text.
private fun DrawScope.drawGridLines(days: Int, m: SurfaceMetrics, theme: SurfaceTheme) {
    for (hour in 1 until HOURS_IN_DAY) {
        val y = m.hourHeight * hour
        drawLine(theme.outlineVariant, Offset(0f, y), Offset(m.weekWidth, y), strokeWidth = 1f)
    }
    for (day in 1 until days) {
        val x = m.dayWidth * day
        drawLine(theme.outlineVariant, Offset(x, 0f), Offset(x, m.gridHeight), strokeWidth = 1f)
    }
}

// The events. `column`/`columns` are the core's overlap solution: an event in a cluster of three sits
// in lane `column` of `columns`, so its width is that share of the day. The client never re-packs:
// if it did, two clients could column identical data differently.
@Suppress("LongParameterList")
private fun DrawScope.drawBlocks(
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
    // twice. Leaving the original in place as a ghost was tried and reads as a duplicate event.
    val held = drag?.subject
    val hourHeightDp = m.hourHeight.toDp()
    val top = scroll.scrollY
    val bottom = scroll.scrollY + (m.height - contentTop)
    val left = scroll.dayX
    val right = scroll.dayX + m.dayViewport
    val started = if (CalendarTrace.on) System.nanoTime() else 0L
    var drawn = 0
    var culled = 0

    page.blocks.forEach { block ->
        if (held != null && block.account == held.account && block.event == held.event &&
            block.day == held.day
        ) {
            return@forEach
        }
        val rect = m.blockRect(block)
        if (rect.bottom <= top || rect.top >= bottom) {
            culled++
            return@forEach
        }
        if (rect.right <= left || rect.left >= right) {
            culled++
            return@forEach
        }
        drawn++

        val x = rect.left
        val y = rect.top
        val gap = 1.dp.toPx()
        val inset = blockInset(block.minutes).toPx()
        val w = rect.width - gap * 2
        val h = rect.height - gap * 2
        if (w <= 0f || h <= 0f) return@forEach

        val corner = CornerRadius(CORNER_RADIUS.toPx())
        drawRoundRect(block.background, Offset(x + gap, y + gap), Size(w, h), corner)
        // An unanswered hold is dashed and hatched instead of hairlined, one call, shared with the
        // all-day bar, the month chip and the invitation card's preview so they cannot diverge.
        drawRoundRect(
            color = block.border,
            topLeft = Offset(x + gap, y + gap),
            size = Size(w, h),
            cornerRadius = corner,
            style = participationStroke(block.awaitingResponse),
        )
        drawHoldHatch(
            rect = Rect(x + gap, y + gap, x + gap + w, y + gap + h),
            color = block.border,
            awaiting = block.awaitingResponse,
        )

        // Zoomed out, the block is a few pixels tall and simply cannot hold text. Drawing a title it
        // has no room for is what sliced every standup's name in half; drawing none is honest, and
        // pinching in brings it back. The block keeps its full spoken label either way.
        if (!blockShowsLabel(block.minutes, hourHeightDp)) return@forEach
        // Shaped against the frozen width, drawn and CLIPPED against the live one. Mid-pinch those
        // differ, and that is the whole point: the rectangle tracks the fingers, the shaper sleeps.
        val room = shapeWidth / block.columns - gap * 2 - 4.dp.toPx() * 2
        if (room <= 0f) return@forEach
        val showsTime = blockShowsTime(block.minutes, hourHeightDp)

        clipRect(x + gap, y + gap, x + gap + w, y + gap + h) {
            val textLeft = x + gap + 4.dp.toPx()
            var textTop = y + gap + inset
            val title = measurer.line(
                text = block.title,
                style = block.titleStyle,
                maxWidth = room,
                maxLines = if (showsTime || block.minutes < 30) 1 else 2,
            )
            drawText(title, topLeft = Offset(textLeft, textTop))
            if (showsTime) {
                textTop += title.size.height
                val clock = measurer.line(block.clock, block.clockStyle, room)
                drawText(clock, topLeft = Offset(textLeft, textTop))
            }
        }
    }
    if (CalendarTrace.on) CalendarTrace.frame(drawn, culled, System.nanoTime() - started)
}

// The red now line, across the whole week with a dot on today's column, the standard affordance, and
// the reason the grid always opens scrolled to roughly now.
private fun DrawScope.drawNowLine(
    m: SurfaceMetrics,
    theme: SurfaceTheme,
    todayIndex: Int,
    nowMinutes: Int,
) {
    val y = m.hourHeight * (nowMinutes / 60f)
    drawLine(theme.error, Offset(0f, y), Offset(m.weekWidth, y), strokeWidth = 2.dp.toPx())
    drawCircle(
        color = theme.error,
        radius = NOW_DOT.toPx() / 2f,
        center = Offset(m.dayWidth * todayIndex, y),
    )
}

// "Loading this period…", the honest answer for a week the engine has not expanded yet.
private fun DrawScope.drawLoading(
    m: SurfaceMetrics,
    theme: SurfaceTheme,
    strings: SurfaceStrings,
    measurer: TextMeasurer,
    contentTop: Float,
) {
    val line = measurer.line(strings.loading, theme.loading, m.dayViewport)
    val height = line.size.height + 12.dp.toPx()
    drawRect(
        color = theme.surface,
        topLeft = Offset(m.gutter, contentTop),
        size = Size(m.dayViewport, height),
    )
    drawText(
        line,
        topLeft = Offset(
            m.gutter + (m.dayViewport - line.size.width) / 2f,
            contentTop + (height - line.size.height) / 2f,
        ),
    )
    drawLine(
        color = theme.primary,
        start = Offset(m.gutter, contentTop + height),
        end = Offset(m.width, contentTop + height),
        strokeWidth = 2.dp.toPx(),
    )
}

/** Whether day column [index] has any part of it on screen. */
private fun columnVisible(index: Int, m: SurfaceMetrics, dayX: Float): Boolean {
    val x = m.dayWidth * index
    return x + m.dayWidth > dayX && x < dayX + m.dayViewport
}

/**
 * One line of text, measured.
 *
 * The width is floored to a multiple of [MEASURE_BUCKET] before it reaches the measurer's cache.
 * A pinch changes a column's width by a fraction of a pixel per frame, and an exact width would miss
 * the cache on **every** frame, re-shaping every visible label sixty times a second, which is the
 * cost this whole file exists to remove. Flooring (never rounding up) keeps the measured line no
 * wider than the room it has, so a title still ellipsises inside its block rather than spilling out
 * of it.
 */
internal fun TextMeasurer.line(
    text: String,
    style: TextStyle,
    maxWidth: Float,
    maxLines: Int = 1,
): TextLayoutResult {
    CalendarTrace.measured()
    return measure(
        text = text,
        style = style,
        overflow = TextOverflow.Ellipsis,
        softWrap = maxLines > 1,
        maxLines = maxLines,
        constraints = Constraints(
            maxWidth = (floor(maxWidth / MEASURE_BUCKET) * MEASURE_BUCKET).toInt().coerceAtLeast(0),
        ),
    )
}

private const val MEASURE_BUCKET = 8f
