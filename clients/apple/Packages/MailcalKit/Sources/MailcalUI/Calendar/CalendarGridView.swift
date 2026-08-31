// The time grid: day columns, an hour ruler, positioned event blocks, the all-day banner, and the
// now line.
//
// The core does the layout. It hands back geometry that carries no units at all, a day *index*, a
// wall-clock *minute*, a column *fraction*, so everything here is a multiplication by this client's
// hour height and column width. Nothing in this file decides where an event goes; it decides how big
// an hour is on a phone.
//
// The scroll offsets are held by hand rather than by a `ScrollView`, for one reason: a pinch has to
// MOVE them, to keep the content under the fingers still. A SwiftUI ScrollView will not be told where
// to be mid-gesture, and a zoom that cannot anchor slides the grid out from under the user's hand:
// which is the single thing that made the first Android build feel broken.
//
// The one thing the core deliberately does NOT send is `now`. A red line baked into a snapshot goes
// stale every 60 seconds and would rebuild the page across the FFI every minute, forever, on battery.
// The client has a clock.

import MailcalBindings
import SwiftUI

let calendarGutter: CGFloat = 52
let calendarLaneHeight: CGFloat = 24
let calendarHours = 24

/// One page of the calendar, a whole week, of which the zoom decides how much is on screen.
struct CalendarGridView: View {
    let page: CalendarPage
    let today: Date
    let nowMinutes: Int
    let use24Hour: Bool
    let hourHeight: CGFloat
    let dayWidth: CGFloat
    let calendar: Calendar
    let viewportWidth: CGFloat
    let viewportHeight: CGFloat
    @Binding var dayOffset: CGFloat
    @Binding var hourOffset: CGFloat
    /// Bumped whenever the grid should re-centre on today and now, on open, and on "back to today".
    let recentreToken: Int
    /// A tap on an event block or all-day band opens that event's detail.
    let onOpen: (EventRefID) -> Void
    /// Whether any calendar can take a new event, an empty-grid drag draws one out.
    let canCreateEvent: Bool
    /// A drag ended: move or resize the event it held, or create the slot it drew.
    let onDrop: (CalendarDragState) -> Void

    @Environment(\.colorScheme) private var colorScheme
    /// Not `private`: CalendarGridView.Gesture.swift's `panGesture` reads and sets it too.
    @State var dragStart: CGPoint?
    /// The drag in flight, or nil. One place, written only by the gesture below and read only by
    /// the grid body, a second copy of "what is the pointer doing" is a second thing that can
    /// disagree with the first.
    ///
    /// Not `private`: CalendarGridView.Gesture.swift reads and sets it too.
    @State var drag: CalendarDragState?

    /// Not `private`: CalendarGridView.Gesture.swift's `beginDrag`/`updateDrag` read it too.
    var days: [Date] {
        page.days.compactMap { parseISODate($0.date, calendar: calendar) }
    }

    private var todayIndex: Int? {
        days.firstIndex { calendar.isDate($0, inSameDayAs: today) }
    }

    /// The week's whole width at this zoom. When the zoom shows all seven days this equals the
    /// viewport and the horizontal pan has nowhere to go, the week is the boundary.
    private var weekWidth: CGFloat { dayWidth * CGFloat(days.count) }
    private var contentHeight: CGFloat { hourHeight * CGFloat(calendarHours) }
    /// Not `private`: CalendarGridView.Gesture.swift's `panGesture` reads it too.
    var maxDayOffset: CGFloat {
        calendarMaxDayOffset(dayWidth: dayWidth, dayCount: days.count, viewportWidth: viewportWidth)
    }
    /// Not `private`, see `maxDayOffset`.
    var maxHourOffset: CGFloat {
        calendarMaxHourOffset(hourHeight: hourHeight, gridHeight: gridHeight)
    }

    /// What is left for the hour grid once the headings and the banner have taken their share.
    private var gridHeight: CGFloat {
        max(viewportHeight - headerHeight - bannerHeight, 1)
    }

    private var headerHeight: CGFloat { 56 }
    private var bannerHeight: CGFloat {
        page.allDayLanes > 0
            ? calendarLaneHeight * CGFloat(
                allDayBannerLanes(lanes: Int(page.allDayLanes), expanded: false)
            )
            : 0
    }

    private var dark: Bool { colorScheme == .dark }

    var body: some View {
        VStack(spacing: 0) {
            CalendarDayHeader(
                days: days,
                todayIndex: todayIndex,
                dayWidth: dayWidth,
                weekWidth: weekWidth,
                dayOffset: dayOffset,
                calendar: calendar
            )
            if page.allDayLanes > 0 {
                CalendarAllDayBanner(
                    page: page,
                    dayCount: days.count,
                    dayWidth: dayWidth,
                    weekWidth: weekWidth,
                    dayOffset: dayOffset,
                    dark: dark,
                    onOpen: onOpen
                )
            }
            Divider()
            if !page.isMaterialized {
                // `isMaterialized == false` does NOT mean "no events", it means the engine has not
                // expanded this far yet. Drawing a confidently empty week here would be a lie that
                // looks exactly like a real answer, so the page says so, in words.
                HStack(spacing: 6) {
                    ProgressView().controlSize(.small)
                    Text(L10n.calendar_loading_range())
                        .font(.caption2)
                        .foregroundStyle(.secondary)
                }
                .padding(.vertical, 3)
            }
            HStack(alignment: .top, spacing: 0) {
                CalendarHourRuler(use24Hour: use24Hour, hourHeight: hourHeight)
                    .offset(y: -hourOffset)
                    .frame(width: calendarGutter, alignment: .top)
                    .clipped()
                gridBody
            }
            .frame(maxHeight: .infinity, alignment: .top)
            .clipped()
            .contentShape(Rectangle())
            .gesture(gridGesture)
            // A desktop scrolls a calendar with the wheel or two fingers, not by dragging it. The
            // offsets are held by hand (see the file header), so there is no ScrollView to do this
            // for us, CalendarScrollGesture reads the raw events and clamps exactly as the pan does.
            #if os(macOS)
            .modifier(
                CalendarScrollGesture(
                    dayOffset: $dayOffset,
                    hourOffset: $hourOffset,
                    maxDayOffset: maxDayOffset,
                    maxHourOffset: maxHourOffset
                )
            )
            #endif
        }
        .onAppear { recentre() }
        .onChange(of: recentreToken) { _, _ in recentre() }
        // **The offsets have to be put back inside the content when the content changes size.**
        //
        // They are held by hand rather than by a `ScrollView` (see the file header), and every other
        // clamp in this file lives inside a *gesture*, so until the user touched the grid, an offset
        // could stay parked outside the content it addresses, and the whole week was scrolled off the
        // screen. What was left looked like a calendar that had failed to load: the gutter, the
        // all-day band, and nothing else.
        //
        // Three things resize the content under a settled offset, and none of them is a gesture:
        // **the window** (shrink it and the hour height shrinks with it), **the zoom arriving late**
        // (the persisted horizon and column count are read from the core, so a client that renders
        // before they land recentres against the defaults and then re-seeds), and **the all-day
        // banner** growing a lane. A `ScrollView` would do this for us; holding the offsets means
        // doing it ourselves, which is what Android's `clampScroll(metrics)` and Windows'
        // viewport-guarded `ApplyRecentre` already do. This is the Apple half of that contract.
        .onChange(of: maxDayOffset) { _, limit in
            dayOffset = dayOffset.clamped(to: 0...limit)
        }
        .onChange(of: maxHourOffset) { _, limit in
            hourOffset = hourOffset.clamped(to: 0...limit)
        }
    }

    /// Opens on **today**, scrolled to roughly now.
    ///
    /// The page is a whole week, and at any zoom below seven columns today may be scrolled off the
    /// side of it, landing on the right week but looking at Monday when it is Sunday is not landing
    /// anywhere useful. So the day axis scrolls to today's column, and the hour axis to just before
    /// now, with a little context above it.
    private func recentre() {
        withAnimation(.easeOut(duration: 0.2)) {
            hourOffset = (CGFloat(max(nowMinutes - 90, 0)) / 60 * hourHeight)
                .clamped(to: 0...maxHourOffset)
            if let todayIndex {
                dayOffset = (CGFloat(todayIndex) * dayWidth).clamped(to: 0...maxDayOffset)
            }
        }
    }

    private var gridBody: some View {
        ZStack(alignment: .topLeading) {
            CalendarGridLines(dayCount: days.count, dayWidth: dayWidth, hourHeight: hourHeight)
            ForEach(page.timed, id: \.rowID) { segment in
                // The block being dragged is drawn once, by `CalendarDragBlock`, where the pointer
                // has it, never twice. Leaving the original in place reads as a duplicate event.
                if !isHeld(segment) {
                    CalendarTimedBlock(
                        segment: segment,
                        calendars: page.calendars,
                        dayWidth: dayWidth,
                        hourHeight: hourHeight,
                        use24Hour: use24Hour,
                        dark: dark,
                        onOpen: {
                            onOpen(
                                EventRefID(
                                    account: segment.account,
                                    key: segment.event,
                                    occurrence: segment.occurrenceStart
                                )
                            )
                        }
                    )
                }
            }
            if let todayIndex {
                CalendarNowLine(
                    nowMinutes: nowMinutes,
                    todayIndex: todayIndex,
                    dayWidth: dayWidth,
                    hourHeight: hourHeight,
                    weekWidth: weekWidth
                )
            }
            if let drag {
                CalendarDragBlock(
                    drag: drag,
                    page: page,
                    dayWidth: dayWidth,
                    hourHeight: hourHeight,
                    use24Hour: use24Hour,
                    dark: dark
                )
            }
        }
        .frame(width: weekWidth, height: contentHeight, alignment: .topLeading)
        .offset(x: -dayOffset, y: -hourOffset)
        .frame(maxWidth: .infinity, maxHeight: .infinity, alignment: .topLeading)
        .clipped()
    }

}

/// The block a drag has in its hand, drawn where the pointer has it, with the time it would land on
/// written on it.
///
/// The label is the point: a quarter-hour snap is invisible on a zoomed-out grid (the block moves
/// three points and the user has no way to know whether that was 15 minutes or 30).
struct CalendarDragBlock: View {
    let drag: CalendarDragState
    let page: CalendarPage
    let dayWidth: CGFloat
    let hourHeight: CGFloat
    let use24Hour: Bool
    let dark: Bool

    var body: some View {
        // Two currencies, on purpose. The block is drawn from `livePreview()`, which follows the
        // pointer to the minute, so the motion is smooth instead of stepping a quarter-hour at a
        // time. The readout is drawn from `preview()`, which is snapped, it is the number that will
        // be written, and a readout agreeing with the pixels instead would quote a minute the drop
        // cannot honour.
        let live = drag.livePreview()
        let settled = drag.preview()
        // A held block takes its whole column: the core's lane packing describes where it *was*, and
        // re-solving overlaps per frame is work a gesture must not do.
        let held = page.timed.first { segment in
            guard let subject = drag.subject else { return false }
            return segment.account == subject.account && segment.event == subject.event
                && Int(segment.day) == subject.day
        }
        // A new slot wears the calendar it would be filed on; a held one keeps its own colours. The
        // accent it used to fall back to is the one colour on the grid that means nothing: every
        // other block says which calendar it belongs to, and the one being created, the only one
        // whose calendar is still a choice, said "accent" on its way to a red calendar.
        let swatch = held.flatMap { segment in
            page.calendars.row(account: segment.account, calendar: segment.calendar)
        }?.swatchOrFallback(dark: dark)
            ?? page.calendars.first { $0.isDefault }?.color.swatch(dark: dark)

        let blockWidth = max(dayWidth - 2, 0)
        let blockHeight = max(hourHeight * CGFloat(live.minutes) / 60 - 2, 0)
        let blockX = dayWidth * CGFloat(live.day) + 1
        let blockY = hourHeight * CGFloat(live.startMinutes) / 60 + 1

        ZStack(alignment: .topLeading) {
            RoundedRectangle(cornerRadius: 4)
                .fill(swatch.map { parseHexColor($0.background) } ?? Color.accentColor)
                .overlay(
                    RoundedRectangle(cornerRadius: 4)
                        .strokeBorder(
                            swatch.map { parseHexColor($0.border) } ?? Color.accentColor,
                            lineWidth: 2
                        )
                )
                .frame(width: blockWidth, height: blockHeight)
                .offset(x: blockX, y: blockY)

            // The time floats beside the block rather than being written inside it. Inside, it was
            // dropped exactly when it was needed most: a fifteen-minute slot at a zoomed-out horizon
            // is a few points tall, and the one label that tells a 15-minute snap from a 30-minute
            // one did not fit in it.
            CalendarDragReadout(
                text: timeRange(settled.startMinutes, settled.endMinutes, use24Hour: use24Hour),
                dark: dark
            )
            .offset(x: blockX, y: max(blockY - dragReadoutGap, 0))
        }
        .allowsHitTesting(false)
    }
}

/// The gap between the block and the readout floating above it.
private let dragReadoutGap: CGFloat = 22

/// The time a drag would land on, in a floating pill.
///
/// The inverse of the surface it sits on, the way a tooltip is, stated as two hexes rather than
/// taken from a system colour, because the pair has to exist on macOS and iOS alike and the
/// platform-specific ones do not.
struct CalendarDragReadout: View {
    let text: String
    let dark: Bool

    var body: some View {
        Text(text)
            .font(.system(size: 12, weight: .medium))
            .lineLimit(1)
            .fixedSize()
            .padding(.horizontal, 8)
            .padding(.vertical, 4)
            .foregroundStyle(parseHexColor(dark ? "#1b1b1b" : "#f2f2f2"))
            .background(parseHexColor(dark ? "#e3e3e3" : "#303030"), in: Capsule())
    }
}

/// The hour ruler. Each label straddles its gridline so "09" reads as the 09:00 boundary rather than
/// as a name for the band beneath it.
struct CalendarHourRuler: View {
    let use24Hour: Bool
    let hourHeight: CGFloat

    var body: some View {
        VStack(spacing: 0) {
            ForEach(0..<calendarHours, id: \.self) { hour in
                ZStack(alignment: .topTrailing) {
                    Color.clear
                    Text(hourLabel(hour, use24Hour: use24Hour))
                        .font(.caption2)
                        .foregroundStyle(.secondary)
                        .offset(y: -6)
                        .padding(.trailing, 6)
                }
                .frame(width: calendarGutter, height: hourHeight)
            }
        }
    }
}

/// The hour lines and the day dividers, drawn once rather than as ~30 composed views.
struct CalendarGridLines: View {
    let dayCount: Int
    let dayWidth: CGFloat
    let hourHeight: CGFloat

    var body: some View {
        Canvas { context, size in
            let line = Color.secondary.opacity(0.25)
            for hour in 1..<calendarHours {
                let y = hourHeight * CGFloat(hour)
                var path = Path()
                path.move(to: CGPoint(x: 0, y: y))
                path.addLine(to: CGPoint(x: size.width, y: y))
                context.stroke(path, with: .color(line), lineWidth: 0.5)
            }
            guard dayCount > 1 else { return }
            for day in 1..<dayCount {
                let x = dayWidth * CGFloat(day)
                var path = Path()
                path.move(to: CGPoint(x: x, y: 0))
                path.addLine(to: CGPoint(x: x, y: size.height))
                context.stroke(path, with: .color(line), lineWidth: 0.5)
            }
        }
    }
}

/// The red now line, with a dot on today's column.
struct CalendarNowLine: View {
    let nowMinutes: Int
    let todayIndex: Int
    let dayWidth: CGFloat
    let hourHeight: CGFloat
    let weekWidth: CGFloat

    var body: some View {
        ZStack(alignment: .topLeading) {
            Rectangle()
                .fill(Color.red)
                .frame(width: weekWidth, height: 1.5)
            Circle()
                .fill(Color.red)
                .frame(width: 8, height: 8)
                .offset(x: dayWidth * CGFloat(todayIndex) - 4, y: -3)
        }
        .offset(y: hourHeight * (CGFloat(nowMinutes) / 60))
        .accessibilityLabel(L10n.calendar_now())
    }
}

extension TimedSegment {
    /// A stable identity: an event key is unique only within its account, and one event can produce
    /// several segments (one per day it crosses).
    var rowID: String { "\(account):\(event):\(day)" }
}

extension AllDayBand {
    var rowID: String { "\(account):\(event):\(day)" }
}
