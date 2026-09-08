// The calendar screen: a header, then the time grid, the month, or the agenda.
//
// A page is a **week**, and that is now only what the core is queried in: the grid's own horizontal
// axis is one continuous strip of days that runs straight through a week boundary
// (`CalendarStrip`, and CalendarScreenView.Grid.swift for what a gesture does to it). The zoom
// decides how many columns are on screen. Day, three-day and week are therefore three ZOOM LEVELS of
// one grid, not three views: the days never move, only their width changes.

import Combine
import MailcalBindings
import SwiftUI

/// A delete on one occurrence of a **repeating** event, held until the user says whether they
/// meant that occurrence or the series. Nothing is removed until they answer.
struct PendingSeriesDelete: Identifiable {
    let id = UUID()
    let account: String
    let key: String
    /// The occurrence the user opened, sent on *This event*, withheld on *All events*.
    let occurrence: String
}

/// The event whose detail sheet is open, its account + key, made Identifiable for `.sheet(item:)`.
struct EventRefID: Identifiable {
    let account: String
    let key: String
    /// The occurrence the user opened, as the surface that drew it gave it, or empty when there
    /// is none to name, a one-off event, or an agenda row, which lists the series rather than
    /// any one of its occurrences. Non-empty is what makes a delete **ask** first.
    let occurrence: String
    var id: String { "\(account):\(key):\(occurrence)" }

    /// Whether a write from here has to ask *This event · All events* first, mirrors
    /// `CalendarDragState.asksAboutTheSeries`, because it is the same question.
    var asksAboutTheSeries: Bool { !occurrence.isEmpty }
}

struct CalendarScreenView: View {
    var model: MailboxModel

    @State private var pager: CalendarPager?
    /// The page the **month** is on. The time grid has no pages: it has a strip (`CalendarStrip`).
    @State private var page = 0
    /// Not `private`: CalendarScreenView.Grid.swift lays the grid out from all three.
    @State var zoom: CalendarZoom
    /// The grid's horizontal axis: one continuous strip of days, in weeks from the pager's origin.
    @State var strip = CalendarStrip()
    @State var hourOffset: CGFloat = 0
    @State private var now = Date()
    @State private var managing = false
    /// Bumped whenever the grid should frame itself on today again: a shape from the menu, and
    /// "back to today". Not `private`: CalendarScreenView.Grid.swift watches it.
    @State var recentreToken = 0
    /// Whether a hand has moved the time grid: a scroll, a flick, a pinch or a chevron.
    ///
    /// Until one has, the grid is the app's to frame, and every layout pass re-frames it on today
    /// and now. After one has, nothing but a deliberate seat (a shape from the menu, "back to
    /// today") may move it again. Not `private`: CalendarScreenView.Grid.swift sets it.
    @State var gridHeld = false
    /// The open editor (create or edit), or nil; and the event whose detail is open, or nil.
    @State private var editor: EventEditorState?
    @State private var openEvent: EventRefID?
    /// A settled drag on a **repeating** event, waiting for the user to say which occurrences it
    /// applies to. Nothing is written until they do, and dismissing writes nothing at all.
    @State private var pendingMove: CalendarDragState?
    /// A delete on one occurrence, waiting on the same question. Held apart from the drag because
    /// the two are presented differently, see the alert below.
    @State private var pendingDelete: PendingSeriesDelete?

    private let tick = Timer.publish(every: 60, on: .main, in: .common).autoconnect()

    /// Seeds the zoom from the **core's persisted settings before the first render**, rather than
    /// letting it default and re-seeding in `onAppear`.
    ///
    /// `recentre` computes both offsets from the geometry of the frame it runs on. Defaulting here
    /// and correcting later means the very first frame is laid out at a horizon and a column count
    /// the user never chose, so the grid is framed against geometry that is about to change, and
    /// the offsets it produced are then wrong for the grid the user actually sees. The clamp in
    /// `CalendarGridView` is the net under that; this stops the fall.
    init(model: MailboxModel) {
        self.model = model
        let settings = model.displaySettings
        _zoom = State(
            initialValue: CalendarZoom(
                visibleHours: Int(settings.visibleHours),
                visibleDays: settings.layout.mode.gridColumns
            )
        )
    }

    /// A Microsoft account connected before calendar support (or with revoked consent) syncs no
    /// calendar until it re-authenticates, prompt for it here, where its absence is felt. Mail is
    /// unaffected, so it uses an informational style, not the error one. "Reconnect" re-runs that
    /// account's sign-in (with its address as the login hint), upgrading its token in place with
    /// the calendar scope; the banner clears once the calendar connects.
    @ViewBuilder private var reauthBanner: some View {
        let emails = model.calendarReauthEmails
        if !emails.isEmpty {
            HStack(spacing: 8) {
                Image(systemName: "exclamationmark.triangle.fill").foregroundStyle(.orange)
                Text(L10n.calendar_reauth_prompt(accounts: emails.joined(separator: ", ")))
                    .font(.callout).fixedSize(horizontal: false, vertical: true)
                Spacer()
                // One button re-auths the first affected account; when several are affected the
                // banner re-renders after each clears, walking through them one sign-in at a time.
                Button(L10n.calendar_reauth_action()) {
                    if let email = emails.first { model.signInWithMicrosoft(loginHint: email) }
                }
                .buttonStyle(.borderless)
                .disabled(model.microsoftSigningIn)
            }
            .padding(.horizontal, 14)
            .padding(.vertical, 9)
            .background(.thinMaterial)
            .overlay(alignment: .bottom) { Divider() }
        }
    }

    var body: some View {
        let calendar = model.gridCalendar
        let today = now
        // The fallback is the shape the core persisted, not a literal: `seed` has not run on the
        // first evaluation, and a hardcoded mode renders one frame in a shape the user did not
        // choose, with `recentre` measuring against it.
        let pager = pager ?? CalendarPager(
            origin: model.weekStart(of: today),
            mode: model.calendarLayout.mode,
            calendar: calendar
        )
        // A grid has no page: its anchor is whichever week the strip's left edge is in, which
        // moves as the user scrolls and moves the grid by exactly nothing when it does.
        let anchor = pager.anchor(forPage: pager.mode.isGrid ? strip.anchorWeek : page)

        VStack(spacing: 0) {
            reauthBanner
            CalendarHeaderBar(
                title: title(pager: pager, anchor: anchor, calendar: calendar),
                mode: pager.mode,
                today: today,
                todayVisible: todayVisible(pager: pager, anchor: anchor, calendar: calendar),
                calendar: calendar,
                writeStatus: model.calendarWriteStatus,
                canCreateEvent: calendarSupportsNewEvent(currentCalendars(pager: pager, anchor: anchor)),
                onMode: { setMode($0, calendar: calendar) },
                onBackToToday: { backToToday(today: today, calendar: calendar) },
                onNewEvent: {
                    let cals = currentCalendars(pager: pager, anchor: anchor)
                    let def = cals.first { $0.canWrite }
                        .map { CalendarChoice(account: $0.account, id: $0.id, name: $0.name) }
                    editor = EventEditorState.create(
                        default: def, zone: TimeZone.current.identifier, now: today
                    )
                },
                onManage: { managing = true },
                onRefresh: { model.showCalendar() },
                onPrevious: { step(-1, mode: pager.mode) },
                onNext: { step(1, mode: pager.mode) }
            )
            Divider()
            content(pager: pager, anchor: anchor, calendar: calendar, today: today)
        }
        .onAppear { seed(calendar: calendar, today: today) }
        .onReceive(tick) { now = $0 }
        .onChange(of: model.displaySettings.visibleHours) { _, hours in
            zoom.resetHours(Int(hours))
            // A horizon arriving from the core (or changed in Settings) resizes the content under
            // the hour offset. While the grid is still the app's to frame, that is a re-frame.
            if !gridHeld { recentreToken += 1 }
        }
        .sheet(isPresented: $managing) {
            CalendarManagerView(model: model, calendars: currentCalendars(pager: pager, anchor: anchor))
        }
        // Tap an event -> its detail; Edit there opens the editor prefilled, Delete removes it.
        .sheet(item: $openEvent) { ref in
            if let detail = model.eventDetail(ref.account, ref.key, ref.occurrence) {
                EventDetailView(
                    detail: detail,
                    calendars: currentCalendars(pager: pager, anchor: anchor),
                    onEdit: {
                        let name = currentCalendars(pager: pager, anchor: anchor)
                            .row(account: detail.account, calendar: detail.calendar)?.name ?? ""
                        editor = EventEditorState.edit(detail, calendarName: name)
                        openEvent = nil
                    },
                    onDelete: {
                        openEvent = nil
                        delete(detail)
                    },
                    asksAboutTheSeries: !detail.occurrenceStart.isEmpty
                )
            }
        }
        // The create/edit editor. One form, both jobs.
        //
        // `sheetItemBinding` for both halves of the presentation lifecycle: the read falls back to
        // the value `.sheet(item:)` handed us, because SwiftUI evaluates this closure on the frame
        // where `editor` has just been cleared and a `Binding($editor)!` traps there (which is what
        // made "Edit" kill the app); and the write refuses to run once `editor` is nil, because the
        // dismissing content still writes and handing that value back re-presented the sheet.
        .sheet(item: $editor) { presented in
            EventEditorView(
                editor: sheetItemBinding($editor, presented: presented),
                calendars: currentCalendars(pager: pager, anchor: anchor),
                onCancel: { editor = nil },
                onCreate: { model.createEvent($0); editor = nil },
                onUpdate: { model.updateEvent($0); editor = nil },
                warningFor: { model.seriesEditWarning($0) }
            )
        }
        // "This event, or all of them?", the one question a drag on a series must ask, and the
        // reason the core exposes an occurrence token at all. Cancelling writes nothing.
        .confirmationDialog(
            L10n.event_series_scope_title(),
            isPresented: Binding(get: { pendingMove != nil }, set: { if !$0 { pendingMove = nil } }),
            titleVisibility: .visible
        ) {
            Button(L10n.event_series_scope_this()) {
                if let args = pendingMove?.moveArgs(thisOccurrenceOnly: true) { model.moveEvent(args) }
                pendingMove = nil
            }
            Button(L10n.event_series_scope_all()) {
                if let args = pendingMove?.moveArgs(thisOccurrenceOnly: false) { model.moveEvent(args) }
                pendingMove = nil
            }
            Button(L10n.action_cancel(), role: .cancel) { pendingMove = nil }
        }
        // The same question on a delete, and an `alert` rather than a confirmation dialog for the
        // reason `EventDetailView`'s own delete confirmation is one: iPadOS draws a confirmation
        // dialog as a popover, and a popover drops the `.cancel`-role button, which would leave a
        // destructive question with no way out. `presenting:` so the buttons act on the delete the
        // alert was raised for.
        .alert(
            L10n.event_series_scope_delete_title(),
            isPresented: Binding(
                get: { pendingDelete != nil }, set: { if !$0 { pendingDelete = nil } }
            ),
            presenting: pendingDelete
        ) { pending in
            Button(L10n.event_series_scope_this(), role: .destructive) {
                model.deleteEvent(pending.account, pending.key, occurrence: pending.occurrence)
                pendingDelete = nil
            }
            Button(L10n.event_series_scope_all(), role: .destructive) {
                model.deleteEvent(pending.account, pending.key)
                pendingDelete = nil
            }
            Button(L10n.action_cancel(), role: .cancel) { pendingDelete = nil }
        }
    }

    /// Opens an event's detail sheet.
    func open(_ ref: EventRefID) {
        openEvent = ref
    }

    /// Deletes an event, asking first when the user opened one occurrence of a series.
    ///
    /// Which occurrence is read off the **detail**, not off the reference that opened it: the
    /// detail names what the core actually resolved, so a token that has gone stale asks nothing
    /// and removes the series, which is what its times say it is describing.
    private func delete(_ detail: EventDetail) {
        guard !detail.occurrenceStart.isEmpty else {
            model.deleteEvent(detail.account, detail.key)
            return
        }
        pendingDelete = PendingSeriesDelete(
            account: detail.account, key: detail.key, occurrence: detail.occurrenceStart
        )
    }

    /// A drag on the grid settled: create the slot it drew, move what it held, or, on a repeating
    /// event, ask which occurrences it meant before writing anything.
    func drop(_ drag: CalendarDragState, week: Int, pager: CalendarPager) {
        let anchor = pager.anchor(forPage: week)
        if drag.kind == .create {
            let preview = drag.preview()
            let days = model.calendarPage(from: anchor, columns: daysInWeek)?.days ?? []
            let cals = currentCalendars(pager: pager, anchor: anchor)
            let def = cals.first { $0.canWrite }
                .map { CalendarChoice(account: $0.account, id: $0.id, name: $0.name) }
            guard
                preview.day >= 0, preview.day < days.count,
                let date = parseISODate(days[preview.day].date, calendar: Calendar.current)
            else { return }
            editor = EventEditorState.create(
                default: def,
                zone: TimeZone.current.identifier,
                // The user drew the time out by hand, so it is taken verbatim, rounding a start
                // they described to the next whole hour would throw the gesture away.
                now: date.addingTimeInterval(TimeInterval(preview.startMinutes) * 60),
                minutes: preview.minutes,
                exact: true
            )
            return
        }
        // The core will not guess whether one occurrence or a whole series was meant, and neither
        // will this: dragging one Tuesday standup is not the same as rewriting every Tuesday.
        if drag.asksAboutTheSeries {
            pendingMove = drag
        } else if let args = drag.moveArgs(thisOccurrenceOnly: false) {
            model.moveEvent(args)
        }
    }

    // MARK: - Content

    @ViewBuilder
    private func content(
        pager: CalendarPager, anchor: Date, calendar: Calendar, today: Date
    ) -> some View {
        switch pager.mode {
        case .agenda:
            CalendarAgendaList(model: model, onOpen: open)
        case .month:
            if let month = model.monthPage(anchor: anchor) {
                CalendarMonthView(
                    page: month,
                    today: today,
                    calendar: calendar,
                    weekStartsMonday: model.weekStartsMonday,
                    use24Hour: model.use24Hour,
                    onOpen: open
                )
                .id(model.calendarVersion)
            }
        default:
            gridContent(pager: pager, calendar: calendar, today: today)
        }
    }

    // MARK: - State

    /// The fingers lifted. Persist both axes, and snap the columns onto a rung.
    ///
    /// Snapping is not cosmetic: the page always holds all seven days, so a column count that does
    /// not divide the week leaves part of it hanging off the side of the screen, and that overhang
    /// is a scroll that competes with the gesture above it. It snaps to the settled *level's*
    /// columns rather than to `settledDays`, because a pinch outwards from the week lands on ~6.4,
    /// which rounds to 6 while the level it maps to is the whole week, of 7.
    func settleZoom() {
        gridHeld = true
        model.setVisibleHours(zoom.settledHours)
        zoom.settleDays()
        let settled = modeForColumns(zoom.settledDays)
        setZoomMode(settled)
        model.setCalendarLayout(settled.layout)
        // A pinch is an input like any other, so it comes to rest on a day too. The columns have
        // just changed width, so wherever the fingers left the strip is almost never a column edge.
        withAnimation(.easeOut(duration: 0.2)) { strip.weeks = strip.nearestDay }
    }

    private func seed(calendar: Calendar, today: Date) {
        // Both axes come from the CORE's persisted settings, so the calendar reopens exactly as it
        // was left, and reopens the same way on the phone. The default is the whole week, which is
        // not an arbitrary preference: a page IS a week, so any narrower shape leaves the rest of it
        // hanging off the side of the screen.
        let mode = model.calendarLayout.mode
        if pager == nil {
            pager = CalendarPager(
                origin: model.weekStart(of: today), mode: mode, calendar: calendar
            )
        }
        zoom.resetHours(Int(model.displaySettings.visibleHours))
        zoom.resetDays(mode.gridColumns)
    }

    /// The height the hour grid gets once the headings and the banner have taken their share.
    ///
    /// Shared with `CalendarGridView`, which draws against the same number: two copies of it would
    /// be two chances for the hour clamp to disagree with the hours on screen.
    func calendarGridHeight(viewportHeight: CGFloat, lanes: Int) -> CGFloat {
        let banner = lanes > 0
            ? calendarLaneHeight * CGFloat(allDayBannerLanes(lanes: lanes, expanded: false))
            : 0
        return max(viewportHeight - calendarHeaderHeight - banner, 1)
    }

    /// One step of the header's chevrons: a week along the strip, or a month of the month grid.
    private func step(_ direction: Int, mode: CalendarMode) {
        if mode.isGrid {
            // From wherever the strip is, so a chevron pressed mid-seam keeps the day it is framed
            // on rather than re-aligning the week under the user.
            gridHeld = true
            withAnimation(.easeOut(duration: 0.25)) { strip.weeks += CGFloat(direction) }
        } else {
            page += direction
        }
    }

    private func setMode(_ next: CalendarMode, calendar: Calendar) {
        guard var current = pager else { return }
        model.setCalendarLayout(next.layout)
        current.setMode(next, currentPage: current.mode.isGrid ? strip.anchorWeek : page)
        // Picking a shape from the MENU is where week alignment happens, not on a zoom, which must
        // leave the days exactly where they are.
        if next.isGrid {
            current.jump(to: model.weekStart(of: current.origin))
            zoom.resetDays(next.columns)
        }
        pager = current
        page = 0
        recentreToken += 1
    }

    /// A settled pinch: change the zoom level WITHOUT moving the origin. The week stays exactly where
    /// it is; the columns just finished changing width.
    private func setZoomMode(_ next: CalendarMode) {
        guard var current = pager, current.mode != next else { return }
        current.setZoom(next)
        pager = current
    }

    private func backToToday(today: Date, calendar: Calendar) {
        guard var current = pager else { return }
        current.jump(to: current.mode.isMonth ? today : model.weekStart(of: today))
        pager = current
        page = 0
        // The strip runs on for ever, so a jump home is a real journey: `recentre` frames it, and
        // landing on the right week while looking at the wrong end of it is only half a jump.
        recentreToken += 1
    }

    func nowMinutes(calendar: Calendar) -> Int {
        let parts = calendar.dateComponents([.hour, .minute], from: now)
        return (parts.hour ?? 0) * 60 + (parts.minute ?? 0)
    }

    private func title(pager: CalendarPager, anchor: Date, calendar: Calendar) -> String {
        switch pager.mode {
        case .agenda:
            return L10n.nav_calendar()
        case .month:
            return monthTitle(anchor, calendar: calendar)
        default:
            let days = (0..<daysInWeek).compactMap {
                calendar.date(byAdding: .day, value: $0, to: anchor)
            }
            return periodTitle(days: days, calendar: calendar)
        }
    }

    private func todayVisible(pager: CalendarPager, anchor: Date, calendar: Calendar) -> Bool {
        switch pager.mode {
        case .agenda:
            return true
        case .month:
            return calendar.isDate(anchor, equalTo: now, toGranularity: .month)
        default:
            // The columns actually on screen, which is the only reading of "in view" a strip has:
            // its days do not belong to a page, so there is no week for today to be inside of.
            // `zoom.visibleDays` is the count the columns were laid out from, so this needs no
            // viewport of its own.
            guard let column = calendar.dateComponents(
                [.day], from: pager.anchor(forPage: 0), to: calendar.startOfDay(for: now)
            ).day else { return false }
            let left = strip.weeks * CGFloat(daysInWeek)
            return CGFloat(column) >= left.rounded(.down) && CGFloat(column) < left + zoom.visibleDays
        }
    }

    func currentCalendars(pager: CalendarPager, anchor: Date) -> [CalendarRow] {
        if pager.mode.isMonth {
            return model.monthPage(anchor: anchor)?.calendars ?? []
        }
        return model.calendarPage(from: anchor, columns: daysInWeek)?.calendars ?? []
    }
}
