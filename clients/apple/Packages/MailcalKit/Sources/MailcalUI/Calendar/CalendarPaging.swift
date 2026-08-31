// What the calendar is showing and where, the state a swipe, a zoom, and "back to today" all move.
//
// The Rust core never tracks where the user is: `calendarRange(from:columns:)` is a pull with an
// argument, so the *client* owns the anchor. This is the whole navigation model, and it is a plain
// type with no SwiftUI in it so the page↔date mapping is testable without a view.
//
// The model is the same one the Android client uses, deliberately: **a page is a week**. That week is
// the boundary a horizontal scroll cannot cross, and the thing a swipe pages between. Zooming does
// not switch to a differently-anchored "view"; it only changes how many of the week's seven columns
// are on screen, so the days never move.
//
// The alternative, snapping a zoom to a Monday-aligned week view, cannot work, and it is worth
// saying why so nobody re-introduces it: a Monday-aligned week cannot contain an arbitrary three-day
// window. A user reading Sunday, Monday and Tuesday who zoomed out would be shown the *previous*
// Monday-to-Sunday, and two of the three days they were reading would vanish.

import Foundation
import MailcalBindings

/// The days in a page. A page is a week.
let daysInWeek = 7

/// The shape the calendar is drawn in.
///
/// The four grid shapes are **not four views**. They are four zoom levels of one grid, the page they
/// sit on is always the same week.
enum CalendarMode: String, CaseIterable, Identifiable {
    case day
    case threeDay
    case workWeek
    case week
    case month
    case agenda

    var id: String { rawValue }

    /// Whether this is the month grid, a different layout, with no hour axis and no zoom.
    var isMonth: Bool { self == .month }

    /// Whether this is the time grid, at any zoom.
    var isGrid: Bool { CalendarMode.gridModes.contains(self) }

    /// How many of the week's seven columns this zoom level shows.
    var columns: Int {
        switch self {
        case .day: return 1
        case .threeDay: return 3
        case .workWeek: return 5
        case .week: return 7
        case .month, .agenda: return 0
        }
    }

    /// The columns to seed the day-axis zoom with.
    ///
    /// The month and the agenda have no columns of their own (`columns` is 0, and dividing the
    /// viewport by it is an infinity). But the grid is still there, one menu tap away, so it is
    /// seeded with the whole week, which is what it should show when the user comes back to it.
    var gridColumns: Int { isGrid ? columns : daysInWeek }

    /// The persisted core setting this mode is stored as, so the calendar reopens as it was left.
    var layout: CalendarLayout {
        switch self {
        case .day: return .day
        case .threeDay: return .threeDay
        case .workWeek: return .workWeek
        case .week: return .week
        case .month: return .month
        case .agenda: return .agenda
        }
    }

    static let gridModes: [CalendarMode] = [.day, .threeDay, .workWeek, .week]
}

extension CalendarLayout {
    /// The mode a persisted layout restores to.
    var mode: CalendarMode {
        switch self {
        case .day: return .day
        case .threeDay: return .threeDay
        case .workWeek: return .workWeek
        case .week: return .week
        case .month: return .month
        case .agenda: return .agenda
        }
    }
}

/// The zoom level showing `columns` of the week's days, the inverse of `CalendarMode.columns`.
///
/// A settled pinch often lands exactly between two rungs (two columns is as close to one as to
/// three). The tie is broken **towards more days**, deliberately: showing a day the user didn't ask
/// for is a smaller sin than hiding one they did.
func modeForColumns(_ columns: Int) -> CalendarMode {
    CalendarMode.gridModes.min { a, b in
        let da = abs(a.columns - columns)
        let db = abs(b.columns - columns)
        return da == db ? a.columns > b.columns : da < db
    } ?? .threeDay
}

/// Maps pager pages to anchor dates, and moves the origin when the user switches shape or jumps home.
///
/// `origin` is the date the middle page shows. It moves only on a deliberate jump, a shape change or
/// "back to today", never on a swipe or a zoom, because a swipe is just a different page over the
/// same origin, and a zoom must leave the days exactly where they are.
struct CalendarPager {
    /// The shape being drawn.
    private(set) var mode: CalendarMode

    /// The date the middle page shows. For a grid this is a week's first day.
    private(set) var origin: Date

    /// The calendar the day arithmetic runs in, the display zone, not the device's.
    private let calendar: Calendar

    init(origin: Date, mode: CalendarMode = .threeDay, calendar: Calendar) {
        self.origin = origin
        self.mode = mode
        self.calendar = calendar
    }

    /// The anchor date `page` shows, the first day the core's query is asked for.
    ///
    /// A grid page is a **whole week**, whatever the zoom. The month is the one shape a day-stride
    /// cannot express: months are 28–31 days long, so striding by a constant would drift off the
    /// month within a year. It pages by calendar month instead, from the 1st, adding months from
    /// (say) the 31st would clamp to the 28th in February and then walk backwards from there.
    func anchor(forPage page: Int) -> Date {
        if mode.isMonth {
            let firstOfMonth = calendar.date(
                from: calendar.dateComponents([.year, .month], from: origin)
            ) ?? origin
            return calendar.date(byAdding: .month, value: page, to: firstOfMonth) ?? firstOfMonth
        }
        return calendar.date(byAdding: .day, value: page * daysInWeek, to: origin) ?? origin
    }

    /// Switches shape, keeping the period the user is looking at.
    mutating func setMode(_ next: CalendarMode, currentPage: Int) {
        guard next != mode else { return }
        origin = anchor(forPage: currentPage)
        mode = next
    }

    /// Changes the **zoom level** without touching the origin.
    ///
    /// The difference from `setMode` matters: that one re-origins on the page you are on, which is
    /// right for a menu choice and wrong for a pinch. A zoom must leave the week exactly where it is:
    /// the columns only get wider.
    mutating func setZoom(_ next: CalendarMode) {
        guard next != mode, next.isGrid else { return }
        mode = next
    }

    /// Re-centres on `date`, "back to today".
    mutating func jump(to date: Date) {
        origin = date
    }
}
