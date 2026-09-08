// The time grid: day columns, a pinned hour ruler, positioned event blocks, the all-day banner, and
// the now line.
//
// The core does the layout. It hands back geometry that carries no units at all, a day *index*, a
// wall-clock *minute*, a column *fraction*, so everything here is a multiplication by this client's
// hour height and column width. Nothing in this file decides where an event goes; it decides how big
// an hour is on a phone.
//
// **The days are one strip, not a stack of week pages.** The weeks are laid end to end with no
// gutter between them and the hour ruler is drawn once, pinned to the left edge, so a grid resting on
// Wednesday-to-Tuesday is showing seven days rather than half of each of two pages. Where the strip
// is, and what a gesture does to it, is `CalendarStrip`; this file only multiplies.
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
/// The day headings' band, above the all-day banner.
let calendarHeaderHeight: CGFloat = 56

/// One week of the strip: which week it is, the core's page for it, and its dates.
struct CalendarStripWeek: Identifiable {
    let index: Int
    let page: CalendarPage
    let days: [Date]

    var id: Int { index }
}

/// The grid, drawn across however many weeks the strip currently has on screen.
///
/// `Animatable` on the strip position, which is what makes a landing correct rather than merely
/// smooth: SwiftUI re-evaluates this body per frame with the interpolated value, so the weeks it
/// *pulls and draws* are the ones the grid is sliding through. Interpolating only the offsets would
/// draw a hole where the week arriving from the right should be.
struct CalendarGridView: View, Animatable {
    /// The week at an index, or nil when the core has no page for it. Called from inside the body,
    /// deliberately: see the note on `Animatable` above.
    let weekAt: (Int) -> CalendarStripWeek?
    /// Bumped by the core whenever a sync changes what a page holds. Read as a plain property, not
    /// applied as an `.id`: replacing the view on every sync would drop a drag in flight, re-run its
    /// framing, and kill a landing part-way through it.
    let calendarVersion: Int
    /// `nonisolated` because `animatableData` is: SwiftUI interpolates it off the main actor, and
    /// the strip is a pair of numbers, so there is nothing here for an actor to protect.
    nonisolated var strip: CalendarStrip
    let today: Date
    let nowMinutes: Int
    let use24Hour: Bool
    let hourHeight: CGFloat
    let dayWidth: CGFloat
    let calendar: Calendar
    /// The width the day columns scroll through: the surface, less the pinned hour ruler.
    let dayViewport: CGFloat
    let viewportHeight: CGFloat
    let hourOffset: CGFloat
    /// A pointer moved: how far the content travelled with it, on each axis.
    let onScroll: (CGFloat, CGFloat) -> Void
    /// The pointer let go: how much further its momentum would carry, on each axis. The day axis
    /// lands on a day from there.
    let onScrollEnded: (CGFloat, CGFloat) -> Void
    /// A tap on an event block or all-day band opens that event's detail.
    let onOpen: (EventRefID) -> Void
    /// Whether any calendar can take a new event, an empty-grid drag draws one out.
    let canCreateEvent: Bool
    /// A drag ended in a week: move or resize the event it held, or create the slot it drew.
    let onDrop: (Int, CalendarDragState) -> Void

    @Environment(\.colorScheme) private var colorScheme
    /// Where a one-finger pan had got to, so each frame reports its own delta rather than the whole
    /// translation. iOS/iPadOS only; macOS scrolls (`CalendarScrollGesture`).
    @State var panned: CGSize?
    /// The drag in flight, and the week it began in. One place, written only by the gesture and read
    /// only by the body: a second copy of "what is the pointer doing" is a second thing that can
    /// disagree with the first.
    ///
    /// Not `private`: CalendarGridView.Gesture.swift reads and sets both.
    @State var drag: CalendarDragState?
    @State var dragWeek: Int?

    nonisolated var animatableData: CGFloat {
        get { strip.weeks }
        set { strip.weeks = newValue }
    }

    private var weekWidth: CGFloat { dayWidth * CGFloat(daysInWeek) }
    private var contentHeight: CGFloat { hourHeight * CGFloat(calendarHours) }

    /// The banner's height is the largest of the weeks **on screen**, not the anchor week's own.
    ///
    /// This is what a pinned ruler costs: the grid's `00:00` is where the ruler's `00:00` is, so a
    /// seam with a three-lane week on one side and an empty one on the other must still have one
    /// content top, or the hour lines would meet the ruler on one side and miss it on the other.
    private func bannerLanes(_ weeks: [CalendarStripWeek]) -> Int {
        weeks.map { Int($0.page.allDayLanes) }.max() ?? 0
    }

    private var dark: Bool { colorScheme == .dark }

    var body: some View {
        // Resolved **once** per frame and handed down, never re-read from a computed property.
        // Every entry is a page pulled across the FFI, and a landing evaluates this body per frame
        // (see `Animatable` above), so a computed `weeks` read by the headings, the banner, its lane
        // count, the loading row and the grid rebuilds the same week five times for one picture.
        let weeks = strip.visibleWeeks(dayViewport: dayViewport, dayWidth: dayWidth)
            .compactMap(weekAt)
        let lanes = bannerLanes(weeks)

        return VStack(spacing: 0) {
            CalendarDayHeader(
                weeks: weeks,
                strip: strip,
                today: today,
                dayWidth: dayWidth,
                calendar: calendar
            )
            if lanes > 0 {
                CalendarAllDayBanner(
                    weeks: weeks,
                    strip: strip,
                    lanes: lanes,
                    dayWidth: dayWidth,
                    dark: dark,
                    onOpen: onOpen
                )
            }
            Divider()
            if weeks.contains(where: { !$0.page.isMaterialized }) {
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
                // Chrome, not content: drawn once and pinned, with the days scrolling past it. A
                // ruler that belonged to a page would slide out with it, which is what made a week
                // boundary the only frame the grid could explain.
                CalendarHourRuler(use24Hour: use24Hour, hourHeight: hourHeight)
                    .offset(y: -hourOffset)
                    .frame(width: calendarGutter, alignment: .top)
                    .clipped()
                gridBody(weeks)
            }
            .frame(maxHeight: .infinity, alignment: .top)
            .clipped()
            .contentShape(Rectangle())
            .gesture(gridGesture)
            // A desktop scrolls a calendar with the wheel or two fingers, not by dragging it, and so
            // does an iPad with a trackpad. The offsets are held by hand (see the file header), so
            // there is no ScrollView to do this for us: CalendarScrollGesture reads the raw events
            // and reports them here.
            .modifier(CalendarScrollGesture(onScroll: onScroll, onScrollEnded: onScrollEnded))
        }
    }

    private func gridBody(_ weeks: [CalendarStripWeek]) -> some View {
        ZStack(alignment: .topLeading) {
            // The strip's own space, so a week only has to say where it begins.
            Color.clear
            ForEach(weeks) { week in
                weekBody(week)
                    .frame(width: weekWidth, height: contentHeight, alignment: .topLeading)
                    .offset(x: strip.origin(ofWeek: week.index, dayWidth: dayWidth), y: -hourOffset)
            }
            if let drag, let dragWeek {
                CalendarDragBlock(
                    drag: drag,
                    page: weekAt(dragWeek)?.page,
                    dayWidth: dayWidth,
                    hourHeight: hourHeight,
                    use24Hour: use24Hour,
                    dark: dark
                )
                .frame(width: weekWidth, height: contentHeight, alignment: .topLeading)
                .offset(x: strip.origin(ofWeek: dragWeek, dayWidth: dayWidth), y: -hourOffset)
            }
        }
        .frame(maxWidth: .infinity, maxHeight: .infinity, alignment: .topLeading)
        .clipped()
    }

    /// One week's content, in its own coordinates: the strip decides where that lands.
    @ViewBuilder
    private func weekBody(_ week: CalendarStripWeek) -> some View {
        let todayColumn = week.days.firstIndex { calendar.isDate($0, inSameDayAs: today) }
        ZStack(alignment: .topLeading) {
            CalendarGridLines(
                dayCount: week.days.count, dayWidth: dayWidth, hourHeight: hourHeight
            )
            ForEach(week.page.timed, id: \.rowID) { segment in
                // The block being dragged is drawn once, by `CalendarDragBlock`, where the pointer
                // has it, never twice. Leaving the original in place reads as a duplicate event.
                if !isHeld(segment, in: week.index) {
                    CalendarTimedBlock(
                        segment: segment,
                        calendars: week.page.calendars,
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
            if let todayColumn {
                CalendarNowLine(
                    nowMinutes: nowMinutes,
                    todayIndex: todayColumn,
                    dayWidth: dayWidth,
                    hourHeight: hourHeight,
                    weekWidth: weekWidth
                )
            }
        }
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

