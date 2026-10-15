// The calendar's header: the period title, the shape picker, "back to today", and the week arrows.
//
// Laid out the way Apple's own Calendar is on each platform (Apple's Human Interface Guidelines,
// "Toolbars"). On iPhone and iPad all of it is in the navigation bar, as system toolbar items, with
// the period as the bar's title (CalendarToolbar.swift). On the Mac the window toolbar carries New
// event, the shape picker and the More menu, and the content keeps a row of its own for the period
// and for moving through it, the title and `‹ ›` that Calendar on the Mac keeps in its content.
//
// The "back to today" affordance is the one Samsung gets right and most calendars get wrong: a
// calendar glyph with *today's date number inside it*, shown only when today is not on screen. It
// tells you where you'd land, and disappears once you're there, so it never sits in the bar as dead
// chrome.

import MailcalBindings
import SwiftUI

/// What the header shows and what its controls do, read once by the screen and handed to the
/// toolbar and, on the Mac, to the row above the grid.
struct CalendarHeaderState {
    let title: String
    let mode: CalendarMode
    let today: Date
    let todayVisible: Bool
    let calendar: Calendar
    let writeStatus: CalendarWriteStatus
    let canCreateEvent: Bool
    let onMode: (CalendarMode) -> Void
    let onBackToToday: () -> Void
    let onNewEvent: () -> Void
    let onManage: () -> Void
    let onRefresh: () -> Void
    let onPrevious: () -> Void
    let onNext: () -> Void

    /// What the arrows step by, said: a month on the month grid, a week everywhere else.
    var previousLabel: String { mode.isMonth ? L10n.calendar_prev_month() : L10n.calendar_prev_week() }
    var nextLabel: String { mode.isMonth ? L10n.calendar_next_month() : L10n.calendar_next_week() }
}

/// The Mac's row above the grid: the period, what the last write did, and moving through time.
struct CalendarHeaderBar: View {
    let state: CalendarHeaderState

    var body: some View {
        HStack(spacing: 8) {
            Text(state.title).font(.headline)
            Spacer(minLength: 8)

            // The result of the user's last create/edit/delete: a spinner while it settles, a
            // check when saved, a tap-to-retry warning when it could not be confirmed. The retry
            // is a refresh, a full sync reconciles the local view (see CalendarWriteStatus).
            CalendarWriteBadge(status: state.writeStatus, onRetry: state.onRefresh)

            if !state.todayVisible {
                CalendarBackToTodayButton(state: state).buttonStyle(.plain)
            }

            Button(action: state.onPrevious) { Image(systemName: "chevron.left") }
                .buttonStyle(.borderless)
                .help(state.previousLabel)
                .accessibilityLabel(state.previousLabel)
            Button(action: state.onNext) { Image(systemName: "chevron.right") }
                .buttonStyle(.borderless)
                .help(state.nextLabel)
                .accessibilityLabel(state.nextLabel)
        }
        .padding(.horizontal, 12)
        .padding(.vertical, 8)
    }
}

/// Today's date in a calendar glyph, which takes the calendar back to today.
///
/// The system's own numbered symbol (`15.calendar`) where the OS has it: borderless, so it sits in a
/// toolbar's container as the guidelines ask. Before iOS and macOS 26 there is no such symbol, and
/// the date is drawn in a box instead.
struct CalendarBackToTodayButton: View {
    let state: CalendarHeaderState

    var body: some View {
        let day = state.calendar.component(.day, from: state.today)
        Button(action: state.onBackToToday) {
            if #available(iOS 26, macOS 26, *) {
                Label(L10n.calendar_back_to_today(), systemImage: "\(day).calendar")
                    .labelStyle(.iconOnly)
            } else {
                Text("\(day)")
                    .font(.caption.weight(.semibold))
                    .frame(width: 22, height: 22)
                    .overlay(RoundedRectangle(cornerRadius: 5).strokeBorder(lineWidth: 1.5))
            }
        }
        .help(L10n.calendar_back_to_today())
        .accessibilityLabel(L10n.calendar_back_to_today())
    }
}

/// A shape's name, as the picker and the menu say it.
func calendarModeLabel(_ mode: CalendarMode) -> String {
    switch mode {
    case .day: return L10n.calendar_view_day()
    case .threeDay: return L10n.calendar_view_three_day()
    case .workWeek: return L10n.calendar_view_work_week()
    case .week: return L10n.calendar_view_week()
    case .month: return L10n.calendar_view_month()
    case .agenda: return L10n.calendar_view_agenda()
    }
}
