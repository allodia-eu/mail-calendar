// One week's page, reduced to exactly what a draw call needs, and nothing a zoom can change.
//
// This file is §7 of the calendar contract made structural rather than merely disciplined. A pinch
// moves the hour height on **every frame**, and the old grid rebuilt, per event, per frame: three hex
// colours parsed, a clock formatted, and a localised accessibility string assembled out of Android
// resources. All of it far more expensive than the arithmetic it sat next to, and none of it able to
// change when only the zoom does.
//
// So the split is enforced by the types. Everything here is derived ONCE, when the page (or the
// theme, or the clock format, or the locale) changes. What is left for the renderer is a day index, a
// wall-clock minute and a column fraction, the core's unit-free geometry, multiplied by an hour
// height and a column width. Multiplication is all a frame is allowed to do.
package eu.allodia.mailcal

import android.content.Context
import androidx.compose.runtime.Immutable
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.text.TextStyle
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.TextUnit
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import java.time.LocalDate
import java.util.Locale
import uniffi.mailcal_bindings.CalendarPage

/** The hours a day column holds. The grid is always a whole day tall; the zoom decides how much fits. */
internal const val HOURS_IN_DAY = 24

// The hour ruler's width, and the height of one all-day lane. How tall an *hour* is is not here and
// cannot be: it is the whole scale of the grid, the user changes it by pinching, and the core's
// unit-free geometry only becomes pixels when it is multiplied through.
internal val GUTTER = 52.dp
internal val LANE_HEIGHT = 24.dp
internal val CORNER_RADIUS = 4.dp

// A block's insets: the gap that keeps two adjacent blocks from touching, and the padding inside the
// coloured chip. A short block spends nothing on the inner padding, every dp of it is the difference
// between showing the title and not.
private val BLOCK_GAP = 1.dp
private val BLOCK_PADDING = 1.dp

internal fun blockInset(minutes: Int): Dp =
    if (minutes < 30) BLOCK_GAP else BLOCK_GAP + BLOCK_PADDING

/** The vertical space a `minutes`-long block leaves for its label, at this zoom. */
internal fun blockLabelSpace(minutes: Int, hourHeight: Dp): Dp =
    hourHeight * (minutes / 60f) - blockInset(minutes) * 2

/**
 * The line box a `minutes`-long block's label gets.
 *
 * Material's own `labelSmall` carries a 16sp line box, taller than the ~11dp a quarter-hour block
 * has to offer at the default horizon, so the clip sliced every standup's title through the middle:
 * geometrically perfect, visibly broken.
 */
internal fun blockLabelLineHeight(minutes: Int): TextUnit = if (minutes < 30) 10.sp else 14.sp

/** The type size that goes with [blockLabelLineHeight]. */
internal fun blockLabelFontSize(minutes: Int): TextUnit = if (minutes < 30) 9.sp else 11.sp

/**
 * Whether a block is tall enough to hold its own title **at this zoom**.
 *
 * Zoomed out to the whole day, a 15-minute event is a few pixels tall and *cannot* hold text, so it
 * doesn't get any, rather than getting a title cut through the middle. It stays a coloured block,
 * keeps its full spoken label for a screen reader, and reveals its title when the user pinches in.
 * This is what every good calendar does, and it is why the rule has to be a function of the zoom
 * rather than a constant.
 */
internal fun blockShowsLabel(minutes: Int, hourHeight: Dp): Boolean =
    blockLabelSpace(minutes, hourHeight).value >= blockLabelLineHeight(minutes).value

/** A block only earns a second line (its start time) once there is room for two. */
internal fun blockShowsTime(minutes: Int, hourHeight: Dp): Boolean =
    blockLabelSpace(minutes, hourHeight).value >= blockLabelLineHeight(minutes).value * 2f

/**
 * A column heading: the weekday and the date, already localised.
 *
 * The core emits an ISO date and owns no locale facility at all (AGENTS.md: "Localisation is
 * client-side"), so the short weekday is assembled here, once per page, not once per frame.
 */
@Immutable
internal data class DayHeading(
    val date: LocalDate,
    val weekday: String,
    val dayOfMonth: String,
)

/**
 * One timed event, ready to draw.
 *
 * [day], [startMinutes], [column] and [columns] are the core's geometry, untouched: an index, a
 * wall-clock minute, and this event's share of its day. The renderer multiplies them by the zoom and
 * does not otherwise think.
 *
 * The two [TextStyle]s carry their own colour, so drawing a block copies nothing. A `style.copy(color
 * = …)` in the draw path would allocate per block, per frame, which is the very cost this type
 * exists to remove.
 */
@Immutable
internal data class BlockPaint(
    /** The owning account and the event's provider key, so a tap can open the event's detail. */
    val account: String,
    val event: String,
    val day: Int,
    val column: Int,
    val columns: Int,
    val startMinutes: Int,
    val endMinutes: Int,
    val title: String,
    val clock: String,
    /** Unconditional: a block too short to show its title still speaks it. */
    val spoken: String,
    val background: Color,
    val border: Color,
    val titleStyle: TextStyle,
    val clockStyle: TextStyle,
    /**
     * An invitation this account has not answered, drawn as a provisional hold rather than a
     * commitment (CalendarParticipation.kt). Resolved here, once, because a pinch must not re-derive
     * it; `DECLINED` never arrives at all, the core hides those.
     */
    val awaitingResponse: Boolean,
    /**
     * Whether this block may be **dragged** to a new time, the core's answer, not ours: a writable
     * calendar *and* an event that is the user's own (their appointment, or a meeting they
     * organise). Strictly narrower than "can be edited".
     */
    val canMove: Boolean,
    /**
     * This occurrence's own start, as the core minted it, empty when the event does not recur.
     * Non-empty means a drag must ask "this event, or all of them?" before it writes.
     */
    val occurrenceStart: String,
    /**
     * Whether the event runs past this column in either direction, so the rectangle on screen is a
     * *clip* of it rather than the whole thing. Undraggable for that reason (CalendarDrag.kt).
     */
    val clipped: Boolean,
) {
    val minutes: Int get() = endMinutes - startMinutes
}

/** One all-day or multi-day bar, ready to draw. [span] is the core's stacking, untouched. */
@Immutable
internal data class BandPaint(
    /** The owning account and the event's provider key, so a tap can open the event's detail. */
    val account: String,
    val event: String,
    val span: BandSpan,
    val title: String,
    val spoken: String,
    val background: Color,
    val titleStyle: TextStyle,
    /** As [BlockPaint.awaitingResponse]: an unanswered hold, drawn provisionally. */
    val awaitingResponse: Boolean,
    /** The bar's own edge colour, a bar has no border until it is a hold and needs a dashed one. */
    val border: Color,
    /**
     * This occurrence's own start, as the core minted it, empty when the event does not recur.
     * Non-empty means a write from this bar must ask "this event, or all of them?" first. A bar
     * cannot be dragged (docs/calendar.md → Known gaps), so today the only such write is a delete
     * from the detail a tap opens.
     */
    val occurrenceStart: String,
)

/**
 * A whole week, drawable.
 *
 * Built once per (page, theme, clock format, locale) and then held across every frame of a pinch and
 * every pixel of a swipe.
 */
@Immutable
internal data class PagePaint(
    val headings: List<DayHeading>,
    val weekNumber: Int,
    val weekSpoken: String,
    val blocks: List<BlockPaint>,
    val bands: List<BandPaint>,
    /** The true lane count the core stacked the bands into, **not** the number the banner shows. */
    val lanes: Int,
    /**
     * What each day column's collapsed banner is hiding, and the chip that says so.
     *
     * Precomputed for the **collapsed** banner only, because an expanded one hides nothing by
     * definition, so this never depends on the banner's live state, and never costs a frame.
     *
     * The counts are per column, and a hidden multi-day bar counts against *every* day it covers: a
     * three-day offsite pushed out of view adds one to three different columns. A "+1" that should
     * say "+2" is a lie the user cannot see through, they tap, find an event nobody told them about,
     * and stop trusting the banner.
     */
    val hiddenPerDay: List<Int>,
    val moreLabels: List<String>,
    val moreSpoken: List<String>,
    /**
     * **`false` does not mean "no events".** It means the engine has not expanded this far yet, and
     * the page must say so in words rather than render a confidently empty week (§4).
     */
    val isMaterialized: Boolean,
) {
    val days: List<LocalDate> get() = headings.map { it.date }
}

/** An empty page, for a week the core has not answered for yet. */
internal val EMPTY_PAGE_PAINT = PagePaint(
    headings = emptyList(),
    weekNumber = 0,
    weekSpoken = "",
    blocks = emptyList(),
    bands = emptyList(),
    lanes = 0,
    hiddenPerDay = emptyList(),
    moreLabels = emptyList(),
    moreSpoken = emptyList(),
    isMaterialized = false,
)

/**
 * Everything the renderer draws with that is not a page: the theme's colours and its type.
 *
 * Read out of `MaterialTheme` once, in the composition, because a `DrawScope` has no access to it:
 * and because looking it up per frame is exactly the kind of per-frame derivation §7 forbids.
 */
@Immutable
internal data class SurfaceTheme(
    val primary: Color,
    val outlineVariant: Color,
    val error: Color,
    val surface: Color,
    /** The base a block's label is built from, its colour comes from the event's own swatch. */
    val blockBase: TextStyle,
    val weekday: TextStyle,
    val weekdayToday: TextStyle,
    val dayNumber: TextStyle,
    val dayNumberToday: TextStyle,
    val hour: TextStyle,
    val weekLabel: TextStyle,
    val weekNumber: TextStyle,
    val allDay: TextStyle,
    val more: TextStyle,
    val loading: TextStyle,
    /**
     * The fill and border a **new** slot is drawn out in, the swatch of the calendar it would be
     * filed on, not the app's accent.
     *
     * A slot drawn in the accent colour is the one block on the grid whose colour means nothing:
     * every other block says which calendar it belongs to, and the one being created, the only one
     * whose calendar is still a choice, said "purple" while it was on its way to a red calendar.
     */
    val dragFill: Color,
    val dragBorder: Color,
    /** The floating readout's own colours, Material's tooltip pair, so it reads in either theme. */
    val pillFill: Color,
    val pillLabel: TextStyle,
)

/**
 * The chrome's own words, the ones that belong to the grid rather than to any week in it.
 *
 * Localised here, once, for the same reason as everything else in this file: reading them out of the
 * resource catalogue inside a frame is a cost that buys nothing, since a zoom cannot change them.
 */
@Immutable
internal data class SurfaceStrings(
    val weekShort: String,
    val allDay: String,
    val loading: String,
    val now: String,
    /**
     * The ruler's 24 labels. Midnight's is empty, it would collide with the day headings directly
     * above, and the top gridline is unambiguous without it.
     */
    val hours: List<String>,
    /** The clock the drag label formats against, the *user's* setting, not the device's. */
    val use24Hour: Boolean,
) {
    companion object {
        fun of(ctx: Context, use24Hour: Boolean) = SurfaceStrings(
            use24Hour = use24Hour,
            weekShort = L10n.calendar_week_short(ctx),
            allDay = L10n.calendar_all_day(ctx),
            loading = L10n.calendar_loading_range(ctx),
            now = L10n.calendar_now(ctx),
            hours = (0 until HOURS_IN_DAY).map { hourLabel(it, use24Hour) },
        )
    }
}

/**
 * Turns one of the core's pages into everything a frame needs.
 *
 * Deliberately takes a [Context]: the spoken labels are localised strings out of the resource
 * catalogue, and assembling them here, once, is what keeps them out of the pinch.
 */
internal fun CalendarPage.toPaint(
    ctx: Context,
    theme: SurfaceTheme,
    dark: Boolean,
    use24Hour: Boolean,
    locale: Locale,
): PagePaint {
    val headings = days.map { day ->
        val date = parseIsoDate(day.date)
        DayHeading(
            date = date,
            weekday = weekdayShort(date, locale),
            dayOfMonth = "${date.dayOfMonth}",
        )
    }
    val noTitle = L10n.event_no_title(ctx)
    val allDayWord = L10n.calendar_all_day(ctx)

    val blocks = timed.map { segment ->
        val start = segment.startMinutes.toInt()
        val end = segment.endMinutes.toInt()
        val calendar = calendars.rowFor(segment.account, segment.calendar)
        val swatch = calendar.swatchOrFallback(dark)
        val title = segment.title.ifEmpty { noTitle }
        val text = parseHexColor(swatch.text)
        // Two rungs, and only two: a quarter-hour block gets type that fits the ~11dp it has, and
        // everything else gets the normal label. Material's own `labelSmall` carries a 16sp line box,
        // which is taller than a short block has to offer, so the clip sliced every standup's title
        // through the middle. Geometrically perfect, visibly broken.
        val minutes = end - start
        val style = theme.blockBase.copy(
            color = text,
            fontSize = blockLabelFontSize(minutes),
            lineHeight = blockLabelLineHeight(minutes),
        )
        val awaiting = isAwaitingResponse(segment.participation)
        BlockPaint(
            account = segment.account,
            event = segment.event,
            day = segment.day.toInt(),
            column = segment.column.toInt(),
            columns = segment.columns.toInt(),
            startMinutes = start,
            endMinutes = end,
            title = title,
            clock = clockTime(start, use24Hour),
            spoken = calendarEventLabel(
                ctx,
                title,
                timeRange(start, end, use24Hour),
                calendar?.name.orEmpty(),
                segment.participation,
            ),
            background = parseHexColor(swatch.background).holdFill(awaiting),
            border = parseHexColor(swatch.border),
            titleStyle = style,
            clockStyle = style,
            awaitingResponse = awaiting,
            canMove = segment.canMove,
            occurrenceStart = segment.occurrenceStart,
            clipped = segment.continuesBefore || segment.continuesAfter,
        )
    }

    val bands = allDay.map { band ->
        val calendar = calendars.rowFor(band.account, band.calendar)
        val swatch = calendar.swatchOrFallback(dark)
        val title = band.title.ifEmpty { noTitle }
        val awaiting = isAwaitingResponse(band.participation)
        BandPaint(
            account = band.account,
            event = band.event,
            span = BandSpan(
                day = band.day.toInt(),
                days = band.days.toInt(),
                lane = band.lane.toInt(),
            ),
            title = title,
            spoken = calendarEventLabel(
                ctx,
                title,
                allDayWord,
                calendar?.name.orEmpty(),
                band.participation,
            ),
            background = parseHexColor(swatch.background).holdFill(awaiting),
            titleStyle = theme.allDay.copy(color = parseHexColor(swatch.text)),
            awaitingResponse = awaiting,
            border = parseHexColor(swatch.border),
            occurrenceStart = band.occurrenceStart,
        )
    }

    // The "+N" a *collapsed* banner would show. An expanded one hides nothing, so there is no second
    // case to compute, and none of this ever runs inside a frame.
    val hidden = allDayOverflowPerDay(
        bands = bands.map { it.span },
        dayCount = headings.size,
        drawnLanes = allDayDrawnLanes(allDayLanes.toInt(), expanded = false),
    )
    val weekNumber = headings.firstOrNull()?.date?.let(::isoWeekNumber) ?: 0

    return PagePaint(
        headings = headings,
        weekNumber = weekNumber,
        weekSpoken = L10n.calendar_week_number(ctx, weekNumber.toString()),
        blocks = blocks,
        bands = bands,
        lanes = allDayLanes.toInt(),
        hiddenPerDay = hidden,
        moreLabels = hidden.map { if (it > 0) L10n.calendar_all_day_more(ctx, it) else "" },
        moreSpoken = hidden.map { if (it > 0) L10n.calendar_all_day_expand(ctx, it) else "" },
        isMaterialized = isMaterialized,
    )
}
