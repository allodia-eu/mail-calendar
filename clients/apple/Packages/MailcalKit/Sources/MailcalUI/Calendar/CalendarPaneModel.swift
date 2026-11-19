// What the calendar pane asks the core (`docs/calendar-pane.md`): where the grid is, and the month
// the person paged the mini month to. The one piece of the pane a client keeps; everything drawn
// comes back from `calendar_pane`.
//
// No SwiftUI here, so the rule that a moved grid takes the paged month away is testable without a
// window.

import Foundation
import MailcalBindings

/// Where the grid beside the pane is resting.
struct CalendarPaneGrid: Equatable {
    /// What the grid draws.
    var layout: CalendarLayout
    /// The first day it shows, `YYYY-MM-DD`; any day of the month for the month layout.
    var anchor: String
    /// How many days it shows.
    var columns: Int
}

/// The pane's inputs, as the screens beside it last set them.
struct CalendarPaneModel: Equatable {
    /// The grid's resting place, or nil before the calendar has drawn.
    private(set) var grid: CalendarPaneGrid?
    /// The month the person paged to with the arrows; nil follows the grid.
    private(set) var month: String?

    /// The grid came to rest. A change of the days it shows takes the paged month away, so the mini
    /// month snaps back to follow it.
    mutating func follow(_ next: CalendarPaneGrid) {
        guard next != grid else { return }
        grid = next
        month = nil
    }

    /// An arrow was pressed: `first` is the `MiniMonth.previous` or `next` the core gave.
    mutating func page(to first: String) {
        month = first
    }

    /// The question for the core, with `today` (`YYYY-MM-DD`) from the client's clock.
    func query(today: String) -> CalendarPaneQuery {
        CalendarPaneQuery(
            today: today,
            layout: grid?.layout ?? .week,
            anchor: grid?.anchor ?? today,
            columns: UInt32(clamping: grid?.columns ?? 7),
            month: month
        )
    }
}

/// A pressed mini-month day, for the calendar screen to apply. Each press is its own request, so
/// pressing the same day again after scrolling away still moves the grid.
struct CalendarSeatRequest: Equatable {
    let seat: CalendarSeat
    let id = UUID()
}
