// The commands for what the toolbar does: the Mac's menu bar, and on iPad and iPhone with a
// hardware keyboard, the shortcut list ⌘ brings up and iPadOS's own menu bar.
//
// Apple's Human Interface Guidelines ask that every toolbar item on the Mac also be a menu command,
// so it can be found in the menu bar and reached from the keyboard, and that an iPad app answer the
// same shortcuts as its Mac version. The commands act on the window in front: the mail surface and the calendar each publish what their toolbar would do as a
// focused scene value, and a command whose surface is not on screen, or whose toolbar item is
// disabled or absent, is disabled.
//
// The shortcuts are the ones Apple's Mail and Calendar use: ⌘N for a new message, ⌘1 onwards for
// the calendar's shapes, ⌘T for today, ⌘← and ⌘→ on the Mac to move through time, and Mail's ⇧⌘N to sync,
// because ⌘R, Calendar's refresh, is Mail's Reply.

import MailcalBindings
import SwiftUI

/// What the mail surface's toolbar does, for the menu bar.
struct MailCommandActions {
    let newMail: () -> Void
    /// `nil` off the mail surface, where its Sync button is not on screen.
    let sync: (() -> Void)?
}

extension FocusedValues {
    @Entry var mailCommands: MailCommandActions?
    /// The calendar's header while the calendar is on screen and nothing is presented over it:
    /// ⌘← and ⌘→ belong to a text field in the event editor while that is open.
    @Entry var calendarCommands: CalendarHeaderState?
}

public struct MailcalCommands: Commands {
    @FocusedValue(\.mailCommands) private var mail
    @FocusedValue(\.calendarCommands) private var calendar

    public init() {}

    public var body: some Commands {
        // In place of File ▸ New Window, which would open a second main window and with it a
        // second core (`AllodiaApp`).
        CommandGroup(replacing: .newItem) {
            Button(L10n.action_compose()) { mail?.newMail() }
                .keyboardShortcut("n")
                .disabled(mail == nil)
            Button(L10n.action_new_event()) { calendar?.onNewEvent() }
                .keyboardShortcut("n", modifiers: [.command, .option])
                .disabled(calendar?.canCreateEvent != true)
        }
        CommandGroup(before: .toolbar) {
            ForEach(Array(CalendarMode.allCases.enumerated()), id: \.element) { index, mode in
                Toggle(
                    calendarMenuLabel(mode),
                    isOn: Binding(get: { calendar?.mode == mode }, set: { _ in calendar?.onMode(mode) })
                )
                .keyboardShortcut(KeyEquivalent(Character(String(index + 1))))
                .disabled(calendar == nil)
            }
            Divider()
            Button(L10n.menu_back_to_today()) { calendar?.onBackToToday() }
                .keyboardShortcut("t")
                .disabled(calendar.map { $0.todayVisible } ?? true)
            Button(calendar?.mode.isMonth == true ? L10n.menu_previous_month() : L10n.menu_previous_week()) {
                calendar?.onPrevious()
            }
            // The Mac only: iPadOS keeps the arrow keys for its own keyboard navigation, and a ⌘←
            // here is listed in its shortcuts and never fires. The iPad moves by the swipe and the
            // toolbar's arrows.
            #if os(macOS)
            .keyboardShortcut(.leftArrow)
            #endif
            .disabled(calendar?.hasArrows != true)
            Button(calendar?.mode.isMonth == true ? L10n.menu_next_month() : L10n.menu_next_week()) {
                calendar?.onNext()
            }
            #if os(macOS)
            .keyboardShortcut(.rightArrow)
            #endif
            .disabled(calendar?.hasArrows != true)
            Divider()
            Button(L10n.menu_manage_calendars()) { calendar?.onManage() }
                .disabled(calendar == nil)
            // The surface in front decides what syncs, as its own Sync button does.
            Button(L10n.action_refresh()) {
                if let calendar { calendar.onRefresh() } else { mail?.sync?() }
            }
                .keyboardShortcut("n", modifiers: [.command, .shift])
                .disabled(calendar == nil && mail?.sync == nil)
            Divider()
        }
    }

    /// A shape's name in title case, as a Mac menu item's is.
    private func calendarMenuLabel(_ mode: CalendarMode) -> String {
        switch mode {
        case .threeDay: return L10n.menu_calendar_three_day()
        case .workWeek: return L10n.menu_calendar_work_week()
        default: return calendarModeLabel(mode)
        }
    }
}
