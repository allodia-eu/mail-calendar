// The time grid: one canvas, one gesture handler, three weeks held in hand.
//
// What used to be a `HorizontalPager` of `CalendarGrid`s, each with its own vertical scroll, its own
// horizontal scroll, a pinch modifier, and one composable per event, is this: a single `Canvas` and
// a single `pointerInput`. The pager's three-live-pages trick survives, because it was right: the
// neighbouring weeks are painted *before* the swipe rather than inside it, so a fling never stops to
// build the page it is flinging towards.
//
// The grid is a PULL, not a pushed snapshot. `pageFor` is a direct, synchronous, argument-taking
// query over the core's in-memory cache, it never touches the store or the network, and the CLIENT
// owns the anchor. That is what makes three live pages possible at all: one snapshot slot cannot hold
// three, and a fire-and-forget dispatch would let two quick swipes race and settle the grid on last
// week after the user had already swiped to next. A pull cannot arrive out of order.
package eu.allodia.mailcal

import android.view.accessibility.AccessibilityManager
import androidx.compose.animation.rememberSplineBasedDecay
import androidx.compose.foundation.Canvas
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.BoxWithConstraints
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.offset
import androidx.compose.foundation.layout.size
import androidx.compose.material3.MaterialTheme
import androidx.compose.runtime.Composable
import androidx.compose.runtime.DisposableEffect
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.SideEffect
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.rememberUpdatedState
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.geometry.Rect
import androidx.compose.ui.hapticfeedback.HapticFeedbackType
import androidx.compose.ui.input.pointer.pointerInput
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.platform.LocalDensity
import androidx.compose.ui.platform.LocalHapticFeedback
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.rememberTextMeasurer
import androidx.compose.ui.unit.Density
import androidx.compose.ui.unit.dp
import java.time.LocalDate
import java.util.Locale
import uniffi.mailcal_bindings.CalendarPage
import uniffi.mailcal_bindings.Swatch

/** The day headings' row: a weekday label over a 28dp date circle, with a little air. */
private val HEADER_HEIGHT = 56.dp
private val DIVIDER = 1.dp

/**
 * How close to a block's top or bottom a press-and-hold has to land to mean "resize" rather than
 * "move".
 *
 * Generous, because the target is a *hidden* edge and a finger is about 9mm wide, and harmless,
 * because it only applies once the block is at least three of these tall (`resizeZoneApplies`).
 * Below that the two zones would meet and every hold on a short event would be a resize of
 * something whose middle you cannot reach.
 */
private val RESIZE_EDGE = 14.dp

/** How fast a flick must be to turn the week on velocity alone, whatever the distance it covered. */
private val MIN_FLING_VELOCITY = 400.dp

/**
 * The paged, zoomable time grid.
 *
 * [anchorFor] maps a week offset to the date that week starts on, the core's `week_start_date`
 * applied at the origin, then whole weeks from there. The core never learns where the user is.
 */
@Composable
@Suppress("LongParameterList")
internal fun CalendarSurface(
    state: CalendarSurfaceState,
    pageFor: (from: LocalDate, columns: Int) -> CalendarPage,
    anchorFor: (week: Int) -> LocalDate,
    /** The date the origin week starts on. The cache is keyed on it: move the origin, a view switch,
     *  a jump home, and every page held here is about a different week. */
    origin: LocalDate,
    calendarVersion: Int,
    today: LocalDate,
    weekStart: LocalDate,
    nowMinutes: Int,
    use24Hour: Boolean,
    locale: Locale,
    /** The settled shape, so a recentre frames the work week from Monday and everything else on today
     *  ([framingColumn]). Only read when the recentre effect fires, a zoom does not re-frame. */
    mode: CalendarMode,
    recentreToken: Int,
    onZoomSettled: (CalendarMode) -> Unit,
    /** A tap on an event block or all-day band opens that event's detail. */
    onOpenEvent: (EventOpen) -> Unit,
    /** Whether any calendar can take a new event, a press-and-hold on empty grid draws one out. */
    canCreateEvent: Boolean,
    /** The swatch of the calendar a new event would be filed on, which is what a drawn slot wears. */
    createSwatch: Swatch,
    /** A press-and-hold ended: move or resize the event it held, or create the slot it drew. */
    onDrop: (CalendarDragState) -> Unit,
    modifier: Modifier = Modifier,
) {
    val ctx = LocalContext.current
    val dark = LocalAppDark.current
    val theme = surfaceTheme(createSwatch)
    val strings = remember(ctx, use24Hour) { SurfaceStrings.of(ctx, use24Hour) }
    val measurer = rememberTextMeasurer(cacheSize = TEXT_CACHE)

    // The three live weeks. Keyed on everything a page's *paint* depends on and nothing else, a zoom
    // is deliberately not in that list, which is the whole point: pinching cannot invalidate this.
    //
    // Held in a map across week changes rather than rebuilt from scratch, so turning the page builds
    // only the week that just came into reach; the other two are already painted.
    val cache = remember(origin, calendarVersion, dark, use24Hour, locale, theme) {
        HashMap<Int, PagePaint>()
    }
    // Two either side, not one. A flick banks its week immediately and lets the pixels catch up, so at
    // full tilt the grid is sliding *through* a week on its way to the one it has already landed on:
    // and a week it does not hold is a gap on the glass. Two pages of lag is the ceiling
    // ([MAX_PAGE_LAG]), and two pages either side is exactly what draws it.
    val pages = remember(cache, state.week) {
        val live = (state.week - LIVE_PAGES)..(state.week + LIVE_PAGES)
        cache.keys.retainAll { it in live }
        live.associateWith { week ->
            cache.getOrPut(week) {
                CalendarTrace.painted()
                pageFor(anchorFor(week), DAYS_IN_WEEK).toPaint(ctx, theme, dark, use24Hour, locale)
            }
        }
    }
    val current = pages[state.week] ?: EMPTY_PAGE_PAINT

    BoxWithConstraints(modifier = modifier.fillMaxSize()) {
        val density = LocalDensity.current
        val viewport = with(density) {
            SurfaceViewport(
                width = maxWidth.toPx(),
                height = maxHeight.toPx(),
                gutter = GUTTER.toPx(),
                headerHeight = HEADER_HEIGHT.toPx(),
                laneHeight = LANE_HEIGHT.toPx(),
                dividerHeight = DIVIDER.toPx(),
                lanes = current.lanes,
            )
        }
        // Read fresh on every pointer event, and **never captured**. The gesture handler is launched
        // once and lives across every swipe and every zoom: anything it closed over at launch, the
        // page, its lane count, the callbacks, would still be last week's by the time a finger
        // landed. The viewport carries the live lane count, so the handler needs nothing else.
        val live by rememberUpdatedState(viewport)
        val settle by rememberUpdatedState(onZoomSettled)
        // The gesture handler is launched once and never restarted, so, like `live`, the current
        // page's paint and the open-event callback must be read fresh, or a tap would hit-test last
        // week's blocks and route to a stale handler.
        val livePaint by rememberUpdatedState(current)
        val openEvent by rememberUpdatedState(onOpenEvent)
        val drop by rememberUpdatedState(onDrop)
        val canCreate by rememberUpdatedState(canCreateEvent)
        val metrics = state.metrics(viewport)
        val haptics = LocalHapticFeedback.current
        val resizeEdge = with(density) { RESIZE_EDGE.toPx() }

        // **Every bound the scroll clamps against is a function of the zoom and the banner, and both
        // move without a finger on the glass.**
        //
        // A pinch clamps as it goes, so it was easy to believe this was covered. It was not. Pick a
        // shape from the *menu*, three days, day-scrolled deep into the week, then "Week", and the
        // columns get wider, `maxDayX` collapses to zero, and nothing re-clamps `dayX`: the week is
        // drawn a thousand pixels off to the left and the grid is simply **blank**. No columns, no
        // headings. Same shape of bug when a swipe lands on a week with fewer all-day lanes: the grid
        // grows taller, `maxScrollY` shrinks, and the day is left scrolled past its own midnight.
        //
        // So the clamp belongs *here*, after every composition, against whatever the metrics have
        // just become, and not at the half-dozen call sites that can move them. It is a no-op in the
        // steady state, and during a pinch it re-does what the pinch already did.
        SideEffect { state.clampScroll(metrics) }

        val scope = rememberCoroutineScope()
        val decay = rememberSplineBasedDecay<Float>()
        val driver = remember(state, scope, decay) { CalendarSurfaceDriver(state, scope, decay) }
        val minFling = with(density) { MIN_FLING_VELOCITY.toPx() }

        // Open at roughly now, with a little context above it. Landing on the right week but the wrong
        // end of it is only half a jump home, so the day axis frames on today too, except the work
        // week, which always opens Mon–Fri (see [framingColumn]).
        LaunchedEffect(Unit) {
            val m = state.metrics(live)
            state.scrollTo(m.hourHeight * ((nowMinutes - 90).coerceAtLeast(0) / 60f), m)
        }
        // Keyed on the token ALONE. Keying it on the column width too would re-fire on every frame of
        // a pinch, that is what a pinch changes, and drag the grid home under the user's fingers.
        LaunchedEffect(recentreToken) {
            val m = state.metrics(live)
            state.scrollDaysTo(m.dayWidth * framingColumn(mode, today, weekStart), m)
        }

        Canvas(
            modifier = Modifier
                .fillMaxSize()
                .pointerInput(state, driver, minFling) {
                    calendarSurfaceGestures(
                        state = state,
                        driver = driver,
                        viewport = { live },
                        minFlingVelocity = minFling,
                        onZoomSettled = {
                            val settled = state.settleZoom(live)
                            CalendarTrace.settled(state.settledHours(), settled.columns)
                            settle(settled)
                        },
                        onTap = { at ->
                            // A tap on an event opens its detail. Hit-test with the renderer's own
                            // geometry (§7, the same rects SurfaceSemantics places nodes at), so a
                            // tap lands on exactly what is drawn.
                            val m = state.metrics(live)
                            val hit = livePaint.eventAt(at, m, state)
                            if (hit != null) {
                                openEvent(hit)
                            } else {
                                // Otherwise the banner is the one thing you can tap, and only when it
                                // is actually hiding something. `live.lanes` is *this* week's lane
                                // count, not the one the week had when this handler was launched.
                                val inBanner = at.y >= m.viewport.headerHeight && at.y < m.contentTop
                                if (inBanner && allDayOverflows(live.lanes)) state.toggleBanner()
                            }
                        },
                        dragAt = { at ->
                            // Same geometry as the tap and the semantics overlay, for the same
                            // reason: a finger, a screen reader and the pixels must agree.
                            val m = state.metrics(live)
                            livePaint.dragAt(
                                at = at,
                                m = m,
                                dayX = state.dayX,
                                scrollY = state.scrollY,
                                resizeEdge = resizeEdge,
                                canCreate = canCreate,
                            )?.also {
                                // The one thing that tells a user the grid has taken hold of the
                                // block, before a single pixel has moved.
                                haptics.performHapticFeedback(HapticFeedbackType.LongPress)
                            }
                        },
                        onDrop = { drop(it) },
                    )
                },
        ) {
            val scroll = SurfaceScroll(state.scrollY, state.dayX, state.pageOffset)
            // The live width at rest; the width the pinch began at while it is in flight. Text is laid
            // out against this and clipped to the real rectangle, see CalendarSurfaceState.
            val shapeWidth =
                if (state.shapedDayWidth > 0f) state.shapedDayWidth else metrics.dayWidth
            for (relative in -LIVE_PAGES..LIVE_PAGES) {
                val page = pages[state.week + relative] ?: continue
                // A neighbour peeking in during a turn is drawn from its own FIRST day, not the current
                // week's scroll offset, a turn opens the new week on its first day, so the week
                // sliding into view is already framed there. Only the current week keeps the live
                // offset; in whole-week zoom there is no day-scroll, so every page is 0 (a no-op).
                val pageDayX = if (relative == 0) state.dayX else 0f
                drawCalendarPage(
                    page = page,
                    m = metrics,
                    theme = theme,
                    strings = strings,
                    measurer = measurer,
                    scroll = scroll.copy(dayX = pageDayX),
                    pageX = relative * metrics.width + scroll.pageOffset,
                    expanded = state.bannerExpanded,
                    // The now line only appears when one of THIS page's columns actually is today, so
                    // paging away from this week hides it rather than drawing it on some Wednesday.
                    todayIndex = page.days.indexOf(today),
                    nowMinutes = nowMinutes,
                    shapeWidth = shapeWidth,
                    // Only the week under the finger draws the drag. A neighbour peeking in during
                    // a turn is a different week, and painting the preview on it would show the
                    // event in two places at once.
                    drag = if (relative == 0) state.drag else null,
                )
            }
        }

        // A canvas speaks to nobody. The blocks keep their full spoken labels, §4 is not negotiable:
        // but the nodes that carry them are materialized ONLY when a screen reader is actually
        // listening: they cost layout, and a pinch would pay it sixty times a second for a service
        // that is not running.
        if (rememberTouchExploration()) {
            SurfaceSemantics(current, metrics, state, strings, today, nowMinutes, density)
        }
    }
}

/** How many laid-out lines the measurer holds. A busy week at a readable zoom is well under this. */
private const val TEXT_CACHE = 256

/**
 * The event a tap at [at] falls on, an all-day band in the banner, or a timed block in the grid:
 * with the occurrence it drew, or `null` for empty space.
 *
 * This mirrors [SurfaceSemantics]'s geometry **exactly** (§7): a finger and a screen reader must both
 * agree with the pixels, so both place from the renderer's own `bandRect`/`blockRect`. The band and
 * grid regions are checked separately by the tap's `y`, so a block scrolled up under the banner can't
 * steal a banner tap.
 */
internal fun PagePaint.eventAt(
    at: androidx.compose.ui.geometry.Offset,
    m: SurfaceMetrics,
    state: CalendarSurfaceState,
): EventOpen? {
    val header = m.viewport.headerHeight
    if (at.y >= header && at.y < m.contentTop) {
        val drawn = allDayDrawnLanes(lanes, state.bannerExpanded)
        val banner = androidx.compose.ui.geometry.Offset(m.gutter - state.dayX, header)
        bands.forEach { band ->
            if (band.span.lane < drawn && m.bandRect(band.span).translate(banner).contains(at)) {
                return EventOpen(band.account, band.event, band.occurrenceStart)
            }
        }
    } else if (at.y >= m.contentTop) {
        val grid =
            androidx.compose.ui.geometry.Offset(m.gutter - state.dayX, m.contentTop - state.scrollY)
        blocks.forEach { block ->
            if (m.blockRect(block).translate(grid).contains(at)) {
                return EventOpen(block.account, block.event, block.occurrenceStart)
            }
        }
    }
    return null
}

/**
 * Weeks held painted either side of the one in view.
 *
 * One would do if the grid only ever moved a page at a time. It does not: a flick banks its week the
 * instant it is decided, so two fast flicks leave the pixels two pages behind the truth and the grid
 * slides *through* the week between. Hold what it slides through, or draw a hole. Each page is a paint
 * the pull already cached, and only the newly-reachable week is built on a turn.
 */
private const val LIVE_PAGES = 2

/** The theme's colours and type, read once, a `DrawScope` cannot see `MaterialTheme`. */
@Composable
private fun surfaceTheme(createSwatch: Swatch): SurfaceTheme {
    val colors = MaterialTheme.colorScheme
    val type = MaterialTheme.typography
    return remember(colors, type, createSwatch) {
        SurfaceTheme(
            primary = colors.primary,
            outlineVariant = colors.outlineVariant,
            error = colors.error,
            surface = colors.surface,
            blockBase = type.labelSmall,
            weekday = type.labelSmall.copy(color = colors.onSurfaceVariant),
            weekdayToday = type.labelSmall.copy(color = colors.primary),
            dayNumber = type.titleSmall.copy(color = colors.onSurface),
            dayNumberToday = type.titleSmall.copy(color = colors.onPrimary),
            hour = type.labelSmall.copy(color = colors.onSurfaceVariant),
            weekLabel = type.labelSmall.copy(color = colors.onSurfaceVariant),
            weekNumber = type.labelLarge.copy(color = colors.onSurfaceVariant),
            allDay = type.labelSmall.copy(color = colors.onSurfaceVariant),
            more = type.labelSmall.copy(color = colors.onSurfaceVariant),
            loading = type.labelSmall.copy(color = colors.onSurfaceVariant),
            dragFill = parseHexColor(createSwatch.background),
            dragBorder = parseHexColor(createSwatch.border),
            pillFill = colors.inverseSurface,
            pillLabel = type.labelMedium.copy(color = colors.inverseOnSurface),
        )
    }
}

/** Whether a screen reader is actually exploring by touch right now. */
@Composable
private fun rememberTouchExploration(): Boolean {
    val ctx = LocalContext.current
    val manager = remember(ctx) { ctx.getSystemService(AccessibilityManager::class.java) }
    var enabled by remember { mutableStateOf(manager?.isTouchExplorationEnabled == true) }
    DisposableEffect(manager) {
        val listener = AccessibilityManager.TouchExplorationStateChangeListener { enabled = it }
        manager?.addTouchExplorationStateChangeListener(listener)
        onDispose { manager?.removeTouchExplorationStateChangeListener(listener) }
    }
    return enabled
}

/**
 * The spoken grid: an invisible, focusable node over every drawn thing that has something to say.
 *
 * Only the page in view, a screen reader cannot reach the week that is half off the side, and
 * offering it a node it cannot see is worse than not offering one at all.
 */
@Composable
private fun SurfaceSemantics(
    page: PagePaint,
    m: SurfaceMetrics,
    state: CalendarSurfaceState,
    strings: SurfaceStrings,
    today: LocalDate,
    nowMinutes: Int,
    density: Density,
) {
    val lane = m.viewport.laneHeight
    val drawn = allDayDrawnLanes(page.lanes, state.bannerExpanded)
    // The banner sits under the headings and does not scroll with the hours; the grid does. Both are
    // offset sideways by the same day scroll. The rectangles themselves come from the *renderer's*
    // own functions, so a node can never land somewhere its event is not drawn.
    val banner = Offset(m.gutter - state.dayX, m.viewport.headerHeight)
    val grid = Offset(m.gutter - state.dayX, m.contentTop - state.scrollY)

    Box(modifier = Modifier.fillMaxSize()) {
        Node(page.weekSpoken, Rect(0f, 0f, m.gutter, m.viewport.headerHeight), density)
        page.bands.forEach { band ->
            if (band.span.lane >= drawn) return@forEach
            Node(band.spoken, m.bandRect(band.span).translate(banner), density)
        }
        if (!state.bannerExpanded) {
            page.moreSpoken.forEachIndexed { day, spoken ->
                if (spoken.isEmpty()) return@forEachIndexed
                Node(spoken, m.moreRect(day, drawn).translate(banner), density)
            }
        }
        page.blocks.forEach { block ->
            Node(block.spoken, m.blockRect(block).translate(grid), density)
        }
        // Only on a page that actually contains today, paging away hides the line, so it must hide
        // what the line says too.
        val todayIndex = page.days.indexOf(today)
        if (todayIndex >= 0) {
            val y = m.contentTop + m.hourHeight * (nowMinutes / 60f) - state.scrollY
            Node(strings.now, Rect(m.gutter, y - 1f, m.width, y + 1f), density)
        }
        if (!page.isMaterialized) {
            Node(
                strings.loading,
                Rect(m.gutter, m.contentTop, m.width, m.contentTop + lane),
                density,
            )
        }
    }
}

/** One invisible node: a rectangle that says what is drawn under it. */
@Composable
private fun Node(label: String, rect: Rect, density: Density) {
    if (label.isEmpty() || rect.width <= 0f || rect.height <= 0f) return
    with(density) {
        Box(
            modifier = Modifier
                .offset(x = rect.left.toDp(), y = rect.top.toDp())
                .size(width = rect.width.toDp(), height = rect.height.toDp())
                .semantics { contentDescription = label },
        )
    }
}
