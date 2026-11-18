// The calendar pane (`docs/calendar-pane.md`): beside the calendar on the Mac and the iPad, the
// pane's column holds the mini month and every calendar under its account in place of the folder
// tree, with the destinations still pinned beneath. Everything drawn is the core's `calendar_pane`.

import MailcalBindings
import SwiftUI

extension ContentView {
    /// What the pane's column draws above the pinned destinations: the calendar's own pane while
    /// the calendar is showing in a column that has one, the folder tree otherwise.
    @ViewBuilder
    func sidebarColumn(showsPane: Bool) -> some View {
        if showsPane, model.destination == .calendar {
            calendarPane
        } else {
            sidebarTree
        }
    }

    private var calendarPane: some View {
        let calendar = model.gridCalendar
        let pane = model.calendarPane()
        return VStack(spacing: 0) {
            if let pane {
                CalendarPaneMonth(
                    month: pane.month,
                    calendar: calendar,
                    onPage: { model.calendarPaneState.page(to: $0) },
                    onDay: { model.seatCalendar(on: $0) }
                )
                .padding(.horizontal, 10)
                .padding(.top, 8)
                .padding(.bottom, 6)
                Divider()
                CalendarPaneList(
                    model: model, groups: pane.groups,
                    toggleChevron: { AnyView(sidebarChevron(expanded: $0)) }
                )
            } else {
                Spacer()
            }
        }
        #if os(macOS)
        // One surface for the whole column, the material the pinned bar beneath is drawn on, so the
        // mini month, the calendars and the destinations read as one pane. Up under the title bar
        // too, as the mailbox's command bar runs, so the column starts at the window's top edge.
        .background { SidebarMaterial().ignoresSafeArea(edges: .top) }
        #endif
    }
}

/// A calendar row's height: the folder pane's on a Mac, and a finger's target on an iPad.
#if os(macOS)
let calendarPaneRowHeight = paneRowHeight
#else
let calendarPaneRowHeight: CGFloat = 44
#endif

/// The calendars under their accounts, toggled and coloured in place.
struct CalendarPaneList: View {
    var model: MailboxModel
    let groups: [CalendarGroup]
    let toggleChevron: (Bool) -> AnyView

    @State private var picking: PickedCalendar?

    var body: some View {
        // A scroll view rather than a sidebar `List`: the list paints a surface of its own, a shade
        // apart from the material the rest of the column is drawn on.
        ScrollView {
            LazyVStack(alignment: .leading, spacing: 0) {
                if groups.isEmpty {
                    Text(L10n.calendar_manage_empty())
                        .foregroundStyle(.secondary)
                        .frame(minHeight: calendarPaneRowHeight)
                }
                ForEach(groups, id: \.account) { group in
                    heading(group)
                    if group.expanded {
                        ForEach(group.calendars, id: \.rowIdentity) { row(group, $0) }
                    }
                }
            }
            .padding(.horizontal, 10)
            .padding(.vertical, 6)
        }
        .accessibilityElement(children: .contain)
        .accessibilityLabel(L10n.calendar_pane_calendars())
        .sheet(item: $picking) { picked in
            CalendarColorPicker(
                calendar: picked.row,
                palette: model.calendarPalette(),
                onPick: { hex in
                    model.setCalendarColor(picked.row.account, picked.row.id, hex)
                    picking = nil
                }
            )
        }
    }

    /// An account's heading, drawn like the folder pane's account row: heavier, with its chevron.
    private func heading(_ group: CalendarGroup) -> some View {
        Button {
            model.setCalendarGroupExpanded(group.account, !group.expanded)
        } label: {
            HStack(spacing: 0) {
                toggleChevron(group.expanded)
                Text(group.address)
                    .fontWeight(.semibold)
                    .lineLimit(1)
                    .truncationMode(.tail)
                Spacer(minLength: 0)
            }
            .frame(minHeight: calendarPaneRowHeight)
            .contentShape(Rectangle())
        }
        .buttonStyle(.plain)
        .accessibilityValue(group.expanded ? L10n.a11y_collapse_folder() : L10n.a11y_expand_folder())
        #if os(macOS)
        .help(group.address)
        #endif
    }

    /// One calendar: the colour check, the name, and a lock where it cannot be written. The whole row
    /// toggles, and redraws from the core's next answer.
    private func row(_ group: CalendarGroup, _ calendar: CalendarRow) -> some View {
        // The palette colour itself, as the grid's accent bar draws it, in both themes.
        let color = parseHexColor(calendar.color.hex)
        return Button {
            model.setCalendarVisible(calendar.account, calendar.id, !calendar.visible)
        } label: {
            HStack(spacing: 8) {
                CalendarPaneCheck(color: color, on: calendar.visible)
                Text(calendar.name).lineLimit(1).truncationMode(.tail)
                Spacer(minLength: 0)
                if !calendar.canWrite {
                    Image(systemName: "lock")
                        .font(.caption)
                        .foregroundStyle(.secondary)
                        .accessibilityLabel(L10n.calendar_pane_read_only())
                }
            }
            .padding(.leading, chevronTargetWidth)
            .frame(minHeight: calendarPaneRowHeight)
            .contentShape(Rectangle())
        }
        .buttonStyle(.plain)
        .accessibilityElement(children: .combine)
        .accessibilityLabel(L10n.a11y_calendar_pane_calendar(name: calendar.name, account: group.address))
        .accessibilityAddTraits(.isToggle)
        .accessibilityAddTraits(calendar.visible ? .isSelected : [])
        .accessibilityIdentifier("CalendarPaneCalendar_\(calendar.account)_\(calendar.id)")
        .contextMenu {
            Button(L10n.calendar_pane_color()) { picking = PickedCalendar(row: calendar) }
            Button(L10n.calendar_color_reset()) {
                model.setCalendarColor(calendar.account, calendar.id, nil)
            }
        }
    }
}

/// The round check a calendar row leads with: filled with the colour and ticked when shown, a ring
/// in the colour when hidden, so the state does not rest on colour alone.
struct CalendarPaneCheck: View {
    let color: Color
    let on: Bool

    var body: some View {
        ZStack {
            if on {
                Circle().fill(color)
                Image(systemName: "checkmark")
                    .font(.system(size: 8, weight: .bold))
                    .foregroundStyle(.white)
            } else {
                Circle().strokeBorder(color, lineWidth: 1.5)
            }
        }
        .frame(width: 14, height: 14)
        .accessibilityHidden(true)
    }
}
