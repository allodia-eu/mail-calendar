// The calendar's toolbar items: system items, so each has the platform's own hit region (44 pt on a
// phone) and grouping, rather than a row of bare glyphs drawn under the bar.
//
// What goes where follows Apple's Human Interface Guidelines ("Toolbars") and Apple's Calendar:
// New event (`+`) and a More menu on the trailing edge; on iPad and the Mac the shapes as a
// segmented control in the centre; on iPad the week arrows beside "back to today". An iPhone has no
// room for either, so its shapes are in the More menu and the week moves by a swipe, on the grid and
// on the month alike (`docs/calendar.md` §12).

import MailcalBindings
import SwiftUI

struct CalendarToolbar: ToolbarContent {
    let state: CalendarHeaderState
    /// Room for the shape picker in the centre: an iPad, or an iPhone with a regular width.
    let regular: Bool

    var body: some ToolbarContent {
        #if os(macOS)
            ToolbarItem(placement: .principal) { modePicker }
            ToolbarItem(placement: .primaryAction) { newEventButton }
            ToolbarItem(placement: .primaryAction) { moreMenu(withModes: false) }
        #else
            if regular {
                // A centre item takes the title's place, so the period moves to the leading edge,
                // where iPadOS shows a title beside the sidebar button: as text, not in the glass
                // a toolbar gives a control, so it does not read as a button.
                if #available(iOS 26, *) {
                    ToolbarItem(placement: .topBarLeading) { title }.sharedBackgroundVisibility(.hidden)
                } else {
                    ToolbarItem(placement: .topBarLeading) { title }
                }
                ToolbarItem(placement: .principal) { modePicker }
            }
            ToolbarItemGroup(placement: .topBarTrailing) {
                CalendarWriteBadge(status: state.writeStatus, onRetry: state.onRefresh)
                if !state.todayVisible {
                    CalendarBackToTodayButton(state: state)
                }
                if regular && state.hasArrows {
                    Button(action: state.onPrevious) {
                        Label(state.previousLabel, systemImage: "chevron.left")
                    }
                    Button(action: state.onNext) {
                        Label(state.nextLabel, systemImage: "chevron.right")
                    }
                }
            }
            if #available(iOS 26, *) {
                ToolbarSpacer(.fixed, placement: .topBarTrailing)
            }
            ToolbarItemGroup(placement: .topBarTrailing) {
                newEventButton
                moreMenu(withModes: !regular)
            }
        #endif
    }

    private var title: some View {
        Text(state.title).font(.headline).fixedSize()
    }

    private var newEventButton: some View {
        Button(action: state.onNewEvent) {
            Label(L10n.action_new_event(), systemImage: "plus")
        }
        .help(L10n.action_new_event())
        // Disabled, not hidden, when no calendar on the page can take a write, so the toolbar
        // keeps its shape (see calendarSupportsNewEvent).
        .disabled(!state.canCreateEvent)
    }

    /// Segmented where all six fit; a menu showing the current shape where they do not, such as an
    /// iPad upright beside the sidebar, rather than a bar that overflows.
    private var modePicker: some View {
        ViewThatFits(in: .horizontal) {
            modes.pickerStyle(.segmented).fixedSize()
            modes.pickerStyle(.menu).fixedSize()
        }
    }

    private var modes: some View {
        Picker(L10n.calendar_view_label(), selection: Binding(get: { state.mode }, set: { state.onMode($0) })) {
            ForEach(CalendarMode.allCases) { mode in
                Text(calendarModeLabel(mode)).tag(mode)
            }
        }
    }

    private func moreMenu(withModes: Bool) -> some View {
        Menu {
            if withModes {
                modes.pickerStyle(.inline)
                Divider()
            }
            Button(L10n.calendar_manage(), action: state.onManage)
            Button(L10n.action_refresh(), action: state.onRefresh)
        } label: {
            Label(L10n.a11y_more_actions(), systemImage: "ellipsis")
        }
        .help(L10n.a11y_more_actions())
    }
}

extension View {
    /// The calendar's toolbar, and on the Mac the same actions for the menu bar
    /// (`MailcalCommands`), withdrawn while something is presented over the calendar.
    func calendarToolbar(_ state: CalendarHeaderState, regular: Bool, covered: Bool) -> some View {
        #if os(macOS)
            toolbar { CalendarToolbar(state: state, regular: regular) }
                .focusedSceneValue(\.calendarCommands, covered ? nil : state)
        #else
            toolbar { CalendarToolbar(state: state, regular: regular) }
        #endif
    }
}
