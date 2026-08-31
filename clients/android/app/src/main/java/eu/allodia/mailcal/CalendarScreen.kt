// The calendar: the header, and the choice of what sits under it, the time grid (CalendarSurface),
// the month, or the agenda. The bottom-navigation host is AppNavScaffold.kt.
//
// The grid is a PULL, not a pushed snapshot. Every other screen here waits for `surfaceChanged` and
// then reads one immutable snapshot slot, but the grid holds three weeks at once (the one in view
// and its neighbours, so a swipe never stops to build the page it is flinging towards), and one slot
// cannot hold three. Worse, `dispatch` is fire-and-forget on a multi-threaded runtime, so two quick
// swipes would race and the grid could settle on *last* week after the user had already swiped to
// next.
//
// So the client owns the anchor (CalendarPager) and asks the core for exactly the page it wants.
// `Surface.CALENDAR` is demoted to a cache-invalidation signal: it bumps [calendarVersion], which
// re-keys the page pulls. A pull cannot arrive out of order.
//
// The **month** still pages with a `HorizontalPager`; the time grid does not, and has no pager state
// at all. It owns its whole pointer stream, pan, page and zoom, because four handlers reading one
// finger is not a thing that can be tuned (see CalendarSurfaceGesture).
//
// What the client does NOT decide: which day the week starts on, and whether times read 14:05 or
// 2:05 PM. Those are persisted core settings ([DisplaySettings]), three clients disagreeing about
// them is not a cosmetic bug. What it DOES decide is how tall an hour is, because the core's
// geometry is unit-free and an hour has no height until a client multiplies (see CalendarZoom).
package eu.allodia.mailcal

import androidx.activity.compose.BackHandler
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.pager.HorizontalPager
import androidx.compose.foundation.pager.rememberPagerState
import androidx.compose.material3.HorizontalDivider
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableIntStateOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalConfiguration
import androidx.compose.ui.platform.LocalContext
import java.time.LocalDate
import java.time.LocalDateTime
import java.util.Locale
import kotlinx.coroutines.delay
import uniffi.mailcal_bindings.CalendarLayout
import uniffi.mailcal_bindings.CalendarPage
import uniffi.mailcal_bindings.CalendarWriteStatus
import uniffi.mailcal_bindings.DisplaySettings
import uniffi.mailcal_bindings.EventDetail
import uniffi.mailcal_bindings.SeriesEditWarning
import uniffi.mailcal_bindings.EventRow
import uniffi.mailcal_bindings.MonthPage
import uniffi.mailcal_bindings.TimeFormat
import uniffi.mailcal_bindings.WeekStart


/**
 * The calendar: a header, then either the paged time grid or the agenda list.
 *
 * [pageFor] is the core's page query, passed as a lambda rather than the `MailcalApp` itself so the
 * whole screen can be driven from a JVM test with synthetic pages, nothing here loads the cdylib.
 *
 * [calendarVersion] changes whenever the core signals `Surface.CALENDAR`; it re-keys the page pulls.
 * [clock] is injectable for the same reason: a test pins "now" instead of racing the wall clock.
 */
@Composable
internal fun CalendarScreen(
    pageFor: (from: LocalDate, columns: Int) -> CalendarPage,
    monthFor: (anchor: LocalDate) -> MonthPage,
    // Where a week begins. The CORE owns this, it applies the user's week-start setting, so three
    // clients cannot disagree about which day a week starts on.
    weekStartFor: (LocalDate) -> LocalDate,
    display: DisplaySettings,
    calendarVersion: Int,
    events: List<EventRow>,
    writeStatus: CalendarWriteStatus,
    activeZoneId: String?,
    palette: List<String>,
    onRefreshCalendar: () -> Unit,
    onDeleteEvent: (account: String, key: String, occurrence: String?) -> Unit,
    onCreateEvent: (CreateArgs) -> Unit,
    onUpdateEvent: (UpdateArgs) -> Unit,
    // A drag on the grid: move or resize an event the user owns (`Intent.MoveEvent`).
    onMoveEvent: (MoveArgs) -> Unit,
    // A synchronous detail pull (App.eventDetail): opens the detail sheet and prefills the editor.
    eventDetailFor: (account: String, key: String, occurrence: String) -> EventDetail?,
    seriesWarningFor: (UpdateArgs) -> SeriesEditWarning?,
    // The device's IANA zone, so a created timed event is created in it (not UTC).
    deviceZoneId: String,
    onSetVisibleHours: (Int) -> Unit,
    onSetLayout: (CalendarLayout) -> Unit,
    onSetCalendarVisible: (account: String, calendar: String, visible: Boolean) -> Unit,
    onSetCalendarColor: (account: String, calendar: String, hex: String?) -> Unit,
    // Opens Settings straight on the Calendar category, the settings this screen is governed by
    // (first day of the week, default zoom, default calendar) are three taps away from the hub, and
    // the calendar is where a user is standing when they want them.
    onOpenCalendarSettings: () -> Unit = {},
    // Emails of Microsoft accounts whose calendar is withheld for lack of the calendar OAuth scope,
    // and the action to re-authenticate one (re-runs its sign-in with the calendar scope).
    calendarReauthEmails: List<String> = emptyList(),
    onReconnectCalendar: (email: String) -> Unit = {},
    clock: () -> LocalDateTime = { LocalDateTime.now() },
) {
    val ctx = LocalContext.current
    val configuration = LocalConfiguration.current
    val locale = remember(configuration) {
        configuration.locales.takeIf { !it.isEmpty }?.get(0) ?: Locale.getDefault()
    }
    // The 12/24-hour clock is the user's SETTING, not the device's, so mail and calendar cannot
    // disagree with each other, and neither can two of our clients.
    val use24Hour = display.timeFormat == TimeFormat.TWENTY_FOUR_HOUR

    // The now line and the "back to today" glyph both need a live clock, the core deliberately
    // never sends `now` (it would go stale every 60s and rebuild the page across the FFI forever).
    var now by remember { mutableStateOf(clock()) }
    LaunchedEffect(Unit) {
        while (true) {
            // Tick on the minute boundary rather than every 60s from launch, so the line moves when
            // the clock does.
            delay(60_000L - System.currentTimeMillis() % 60_000L)
            now = clock()
        }
    }
    val today = now.toLocalDate()
    val nowMinutes = now.hour * 60 + now.minute

    // The grid's origin is a WEEK, the boundary a horizontal scroll cannot cross, and what a
    // sideways swipe pages between.
    // Seeded from the CORE's persisted layout, so the calendar reopens in the shape it was left in:
    // and reopens the same way on every client, rather than each one keeping its own idea.
    val pager = remember { CalendarPager(weekStartFor(today), display.layout.toMode()) }
    val pagerState = rememberPagerState(initialPage = CALENDAR_PAGE_ORIGIN) { CALENDAR_PAGE_COUNT }
    // The open editor (create or edit), or null; and the event whose detail is open, or null.
    var editor by remember { mutableStateOf<EventEditorState?>(null) }
    var openEventRef by remember { mutableStateOf<EventOpen?>(null) }
    var managing by remember { mutableStateOf(false) }
    // A delete on one **occurrence** of a series, waiting for the same question the drag asks.
    // Nothing is written until it is answered.
    var pendingDelete by remember { mutableStateOf<EventOpen?>(null) }
    // A settled drag on a **repeating** event, waiting for the user to say which occurrences it
    // applies to. The core will not guess, and neither will this: dragging one Tuesday standup is
    // not the same as rewriting every Tuesday to eternity.
    var pendingMove by remember { mutableStateOf<CalendarDragState?>(null) }
    val weekStartsMonday = display.weekStart == WeekStart.MONDAY

    // The grid's whole navigation model, where it is scrolled, how far it is zoomed, and which week.
    // Seeded from the CORE's persisted settings, so the calendar reopens in the shape it was left in,
    // and reopens the same way on every client rather than each one keeping its own idea.
    val surface = remember {
        CalendarSurfaceState(display.visibleHours.toInt(), display.layout.toMode().gridColumns)
    }
    LaunchedEffect(display.visibleHours) { surface.resetHours(display.visibleHours.toInt()) }
    // A shape chosen from the MENU re-seeds the day axis. A pinch does not, it drives `visibleDays`
    // itself, and re-seeding from the mode it settled on would snap the columns back to a whole
    // number mid-gesture. (The first composition is a no-op: the zoom was seeded from the same
    // persisted layout this reads.)
    LaunchedEffect(pager.mode) {
        if (pager.mode.isGrid) surface.resetDays(pager.mode.columns)
    }

    // Which page the user is on. The month keeps a `HorizontalPager`; the grid owns its own week, and
    // no longer has one at all.
    val currentPageIndex =
        if (pager.mode.isGrid) CALENDAR_PAGE_ORIGIN + surface.week else pagerState.currentPage

    // The week offsets the grid pages by, mapped onto dates. Stable across recompositions, the
    // surface holds its three painted weeks against it, and a fresh lambda each frame would throw
    // them away and repaint the lot.
    val anchorFor = remember(pager) { { week: Int -> pager.anchorFor(CALENDAR_PAGE_ORIGIN + week) } }

    // The origin moved (a view switch, or "back to today"): re-centre on it.
    LaunchedEffect(pager.resetToken) {
        surface.resetWeek()
        pagerState.scrollToPage(CALENDAR_PAGE_ORIGIN)
    }

    // The settled page drives the header, its title, and whether "back to today" is worth offering.
    // Pulling it again here (the grid pulls its own) costs an in-memory read: the query never touches
    // the store or the network.
    val anchor = pager.anchorFor(currentPageIndex)
    // A grid page is always the whole week, whatever the zoom, the zoom only decides how many of its
    // columns fit on screen. So this query never changes as the user pinches, and the days under
    // their fingers cannot move.
    val currentPage = remember(pager.mode, pager.origin, currentPageIndex, calendarVersion) {
        if (pager.mode.isGrid) pageFor(anchor, DAYS_IN_WEEK) else null
    }
    // The month is a different query, and it is also the one view whose title comes from the ANCHOR
    // rather than from the days on screen: a month grid deliberately shows a few days either side, so
    // titling it from its columns would name June for a July page.
    val currentMonth = remember(pager.mode, pager.origin, currentPageIndex, calendarVersion) {
        if (pager.mode.isMonth) monthFor(anchor) else null
    }
    val visibleDays = remember(currentPage) {
        currentPage?.days.orEmpty().map { parseIsoDate(it.date) }
    }
    val onAgenda = pager.mode == CalendarMode.AGENDA
    val title = when {
        onAgenda -> L10n.nav_calendar(ctx)
        pager.mode.isMonth -> monthTitle(anchor, locale)
        else -> periodTitle(visibleDays, locale)
    }
    // Every page lists every calendar across every account, even when it holds no events, so any
    // page answers both the manager and "can anything take a new event?". The agenda composes no
    // page of its own, so it pulls one just for that list (an in-memory read, like the pulls above).
    val calendars = currentPage?.calendars
        ?: currentMonth?.calendars
        ?: remember(calendarVersion, today) { pageFor(weekStartFor(today), DAYS_IN_WEEK).calendars }

    // The detail of the open event, re-pulled when the calendar changes so an edit refreshes it.
    val openDetail = remember(openEventRef, calendarVersion) {
        openEventRef?.let { eventDetailFor(it.account, it.key, it.occurrenceStart) }
    }
    // A tap on any event, a grid block, an agenda row, a month chip, opens its detail.
    val openEvent: (EventOpen) -> Unit = { openEventRef = it }
    // Where a new event goes, and so the colour a drawn slot wears. The core resolves this, the
    // user's chosen calendar while it exists and can be written to, else the first writable one:
    // so there is no fallback rule here to drift from Settings' idea of the same thing.
    val defaultCalendarRow = remember(calendars) { calendars.firstOrNull { it.isDefault } }
    val defaultCalendar = remember(defaultCalendarRow) {
        defaultCalendarRow?.let { CalendarChoice(it.account, it.id, it.name) }
    }
    // A slot drawn out on the grid wears the colour of the calendar it is on its way to. Derived from
    // the same row as `defaultCalendar` rather than looked up again, so the block on screen and the
    // calendar the editor opens on cannot drift apart.
    val dark = LocalAppDark.current
    val createSwatch = remember(defaultCalendarRow, dark) { defaultCalendarRow.swatchOrFallback(dark) }

    // Bumped by "back to today". The grid watches it, because only the grid knows a column's width.
    var recentreToken by remember { mutableIntStateOf(0) }

    // The manager, the detail and the editor are opaque Surfaces composed OVER the grid, not
    // windows of their own, so unlike a Dialog or a bottom sheet, nothing hands them the system
    // back press. Unwind them here, topmost first, in the order they are drawn below. Once the grid
    // is bare the handler goes quiet and the press reaches AppNavScaffold, which takes it home.
    BackHandler(enabled = editor != null || openEventRef != null || managing) {
        when {
            editor != null -> editor = null
            openEventRef != null -> openEventRef = null
            else -> managing = false
        }
    }

    Column(modifier = Modifier.fillMaxSize()) {
        CalendarHeader(
            title = title,
            mode = pager.mode,
            onModeChange = {
                pager.setMode(it, currentPageIndex)
                onSetLayout(it.toLayout())
            },
            today = today,
            // The agenda always contains today, so the jump would be a no-op there.
            todayVisible = when {
                onAgenda -> true
                pager.mode.isMonth -> anchor.year == today.year && anchor.month == today.month
                else -> today in visibleDays
            },
            writeStatus = writeStatus,
            // New events need somewhere to go: some calendar, on some account, that the provider
            // lets us write to. An empty list (nothing synced yet) is the same "no".
            canCreateEvent = calendars.any { it.canWrite },
            onBackToToday = {
                pager.jumpTo(if (pager.mode.isMonth) today else weekStartFor(today))
                // Today's column may be scrolled off the side of its own week, so put it back in
                // view too, landing on the right week but looking at the wrong end of it is only
                // half a jump home. The scroll happens in the grid, which is the only thing here
                // that knows how wide a column is.
                recentreToken += 1
            },
            onNewEvent = { editor = EventEditorState.create(defaultCalendar, deviceZoneId, now) },
            onRefresh = onRefreshCalendar,
            onManageCalendars = { managing = true },
            onOpenCalendarSettings = onOpenCalendarSettings,
        )
        HorizontalDivider()
        // A Microsoft account connected before calendar support (or with revoked consent) syncs no
        // calendar until it re-authenticates, prompt for it here, where its absence is felt.
        CalendarReauthBanner(calendarReauthEmails, onReconnectCalendar, ctx)
        if (pager.mode.isMonth) {
            HorizontalPager(
                state = pagerState,
                flingBehavior = calendarFlingBehavior(pagerState),
                // The neighbouring months, composed before the swipe rather than during it.
                beyondViewportPageCount = 1,
                modifier = Modifier.weight(1f),
            ) { page ->
                val monthPage = remember(pager.mode, pager.origin, page, calendarVersion) {
                    monthFor(pager.anchorFor(page))
                }
                CalendarMonthGrid(
                    page = monthPage,
                    today = today,
                    locale = locale,
                    weekStartsMonday = weekStartsMonday,
                    onOpenEvent = openEvent,
                )
            }
        } else if (onAgenda) {
            AgendaList(
                events = events,
                activeZoneId = activeZoneId,
                use24Hour = use24Hour,
                // An agenda row *is* the series, one row per event, not per occurrence, so a
                // swipe there names no occurrence and removes the whole thing, with nothing to ask.
                onDeleteEvent = { account, key -> onDeleteEvent(account, key, null) },
                onOpenEvent = openEvent,
                modifier = Modifier.weight(1f),
            )
        } else {
            // One canvas, one gesture handler. The hour height is the viewport over the horizon and
            // the column width the viewport over the visible days, so "12 hours, 3 days" means the
            // same span on a phone and a tablet, the cells just get bigger. Both live in the
            // surface's own state, because the core's geometry is unit-free and an hour has no height
            // until a client multiplies.
            CalendarSurface(
                state = surface,
                pageFor = pageFor,
                anchorFor = anchorFor,
                origin = pager.origin,
                calendarVersion = calendarVersion,
                today = today,
                weekStart = weekStartFor(today),
                nowMinutes = nowMinutes,
                use24Hour = use24Hour,
                locale = locale,
                mode = pager.mode,
                recentreToken = recentreToken,
                onZoomSettled = { settled ->
                    // Persist only once the fingers lift: a save per frame would push a preference
                    // write across the FFI dozens of times a second.
                    onSetVisibleHours(surface.settledHours())
                    // `setZoom`, NOT `setMode`: the origin must not move, the week stays exactly
                    // where it is, and the columns have just finished changing width.
                    pager.setZoom(settled)
                    onSetLayout(settled.toLayout())
                },
                onOpenEvent = openEvent,
                canCreateEvent = calendars.any { it.canWrite },
                createSwatch = createSwatch,
                onDrop = { drag ->
                    when {
                        // A slot drawn out on empty grid opens the same editor "New event" does,
                        // prefilled with what the hand actually described.
                        drag.kind == DragKind.CREATE -> {
                            val preview = drag.preview()
                            visibleDays.getOrNull(preview.day)?.let { date ->
                                editor = EventEditorState.create(
                                    default = defaultCalendar,
                                    zone = deviceZoneId,
                                    now = date.atStartOfDay()
                                        .plusMinutes(preview.startMinutes.toLong()),
                                    minutes = preview.minutes,
                                    exact = true,
                                )
                            }
                        }
                        // A repeating event has to be asked about before anything is written.
                        drag.asksAboutTheSeries() -> pendingMove = drag
                        else -> drag.moveArgs(thisOccurrenceOnly = false)?.let(onMoveEvent)
                    }
                },
                modifier = Modifier.weight(1f),
            )
        }
    }

    // "This event, or all of them?", the one question a drag on a series must ask, and the reason
    // the core exposes an occurrence token at all. Dismissing writes nothing.
    pendingMove?.let { drag ->
        EventSeriesScopeDialog(
            title = L10n.event_series_scope_title(LocalContext.current),
            onDismiss = { pendingMove = null },
            onThisEvent = {
                drag.moveArgs(thisOccurrenceOnly = true)?.let(onMoveEvent)
                pendingMove = null
            },
            onAllEvents = {
                drag.moveArgs(thisOccurrenceOnly = false)?.let(onMoveEvent)
                pendingMove = null
            },
        )
    }

    // The same question on a delete: removing this Tuesday and cancelling the standup are
    // different requests, and only the user knows which they meant. Dismissing writes nothing.
    pendingDelete?.let { ref ->
        EventSeriesScopeDialog(
            title = L10n.event_series_scope_delete_title(LocalContext.current),
            onDismiss = { pendingDelete = null },
            onThisEvent = {
                onDeleteEvent(ref.account, ref.key, ref.occurrenceStart)
                pendingDelete = null
            },
            onAllEvents = {
                onDeleteEvent(ref.account, ref.key, null)
                pendingDelete = null
            },
        )
    }

    if (managing) {
        CalendarManagerScreen(
            calendars = calendars,
            palette = palette,
            onSetVisible = onSetCalendarVisible,
            onSetColor = onSetCalendarColor,
            onBack = { managing = false },
        )
    }

    // Tapping an event opens its detail; Edit there opens the editor prefilled, Delete removes it.
    openDetail?.let { detail ->
        EventDetailScreen(
            detail = detail,
            calendars = calendars,
            onBack = { openEventRef = null },
            onEdit = {
                val name = calendars
                    .firstOrNull { it.account == detail.account && it.id == detail.calendar }
                    ?.name
                    .orEmpty()
                editor = EventEditorState.edit(detail, name)
                openEventRef = null
            },
            onDelete = {
                openEventRef = null
                // Which occurrence is read off the **detail**, not off the reference that opened
                // it: the detail names what the core actually resolved, so a token gone stale
                // asks nothing and removes the series, which is what its times describe.
                if (detail.occurrenceStart.isNotEmpty()) {
                    pendingDelete = EventOpen(detail.account, detail.key, detail.occurrenceStart)
                } else {
                    onDeleteEvent(detail.account, detail.key, null)
                }
            },
            asksAboutTheSeries = detail.occurrenceStart.isNotEmpty(),
        )
    }

    // The create/edit editor (over the detail, so an Edit lands here). One form, both jobs.
    editor?.let { open ->
        EventEditorScreen(
            editor = open,
            calendars = calendars,
            onCancel = { editor = null },
            onCreate = {
                onCreateEvent(it)
                editor = null
            },
            onUpdate = {
                onUpdateEvent(it)
                editor = null
            },
            warningFor = seriesWarningFor,
        )
    }
}
