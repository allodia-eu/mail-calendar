// The calendar manager: which calendars are drawn, and in what colour. And the agenda list.
//
// Grouped by account, because a calendar id is unique only within its account, two accounts can each
// have a "Work", and a flat list would leave the user unable to tell which is which. Every toggle and
// colour is persisted by the core and applied at page-read time, so the grid redraws immediately with
// no sync and no network.

import MailcalBindings
import SwiftUI

struct CalendarManagerView: View {
    var model: MailboxModel
    let calendars: [CalendarRow]

    @Environment(\.dismiss) private var dismiss
    @Environment(\.colorScheme) private var colorScheme
    @State private var picking: PickedCalendar?

    private var byAccount: [(account: String, rows: [CalendarRow])] {
        var order: [String] = []
        var grouped: [String: [CalendarRow]] = [:]
        for row in calendars {
            if grouped[row.account] == nil { order.append(row.account) }
            grouped[row.account, default: []].append(row)
        }
        return order.map { ($0, grouped[$0] ?? []) }
    }

    var body: some View {
        NavigationStack {
            List {
                if calendars.isEmpty {
                    Text(L10n.calendar_manage_empty()).foregroundStyle(.secondary)
                }
                ForEach(byAccount, id: \.account) { group in
                    Section(group.account) {
                        ForEach(group.rows, id: \.rowIdentity) { calendar in
                            row(calendar)
                        }
                    }
                }
            }
            .navigationTitle(L10n.calendar_manage())
            .toolbar {
                ToolbarItem(placement: .confirmationAction) {
                    Button(L10n.action_done()) { dismiss() }
                }
            }
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
        .frame(minWidth: 340, minHeight: 320)
    }

    private func row(_ calendar: CalendarRow) -> some View {
        HStack(spacing: 10) {
            Button {
                picking = PickedCalendar(row: calendar)
            } label: {
                Circle()
                    .fill(parseHexColor(calendar.color.swatch(dark: colorScheme == .dark).background))
                    .frame(width: 18, height: 18)
                    .overlay(Circle().strokeBorder(.secondary.opacity(0.4), lineWidth: 0.5))
            }
            .buttonStyle(.plain)
            .accessibilityLabel(L10n.calendar_pick_color(name: calendar.name))

            Text(calendar.name).lineLimit(1)
            Spacer()
            Toggle(
                "",
                isOn: Binding(
                    get: { calendar.visible },
                    set: { model.setCalendarVisible(calendar.account, calendar.id, $0) }
                )
            )
            .labelsHidden()
        }
    }
}

/// The palette, as swatches. The colours come from the core, a client cannot invent one, and Allodia
/// Orange is deliberately absent because it means "action" in this product.
struct CalendarColorPicker: View {
    let calendar: CalendarRow
    let palette: [String]
    let onPick: (String?) -> Void

    @Environment(\.dismiss) private var dismiss

    private let columns = Array(repeating: GridItem(.flexible(), spacing: 12), count: 5)

    var body: some View {
        NavigationStack {
            VStack(spacing: 16) {
                LazyVGrid(columns: columns, spacing: 12) {
                    ForEach(palette, id: \.self) { hex in
                        let selected = hex.caseInsensitiveCompare(calendar.color.hex) == .orderedSame
                        Circle()
                            .fill(parseHexColor(hex))
                            .frame(width: 38, height: 38)
                            .overlay(
                                Circle().strokeBorder(
                                    selected ? Color.primary : Color.clear, lineWidth: 3
                                )
                            )
                            .onTapGesture { onPick(hex) }
                            .accessibilityLabel(hex)
                    }
                }
                // Clearing the override hands the calendar back to whatever colour its server sent.
                Button(L10n.calendar_color_reset()) { onPick(nil) }
                Spacer()
            }
            .padding()
            .navigationTitle(calendar.name)
            .toolbar {
                ToolbarItem(placement: .confirmationAction) {
                    Button(L10n.action_close()) { dismiss() }
                }
            }
        }
        .frame(minWidth: 300, minHeight: 260)
    }
}

/// The agenda: the flat, soonest-first event list. Deliberately NOT the time grid with one column:
/// it is an unbounded list over the engine's own ordering, so forcing it through the grid's layout
/// solver would buy nothing.
struct CalendarAgendaList: View {
    var model: MailboxModel
    /// A tap on a row opens the event's detail.
    let onOpen: (EventRefID) -> Void

    var body: some View {
        List(model.events, id: \.rowID) { event in
            HStack(spacing: 8) {
                Image(systemName: "calendar").foregroundStyle(.secondary)
                VStack(alignment: .leading, spacing: 1) {
                    Text(event.title.isEmpty ? L10n.event_no_title() : event.title).lineLimit(1)
                    // A list row has no border to dash and no gutter to hatch, so the hold says
                    // itself in words, which is the disclosure the dashes only stand in for
                    // anyway (docs/calendar.md §4).
                    if isAwaitingResponse(event.participation) {
                        Text(L10n.a11y_invitation_awaiting_response())
                            .font(.caption2)
                            .foregroundStyle(.secondary)
                    }
                }
                Spacer()
                Text(localDateTime(event.start, in: model.activeZone))
                    .font(.caption2)
                    .foregroundStyle(.secondary)
            }
            .padding(.vertical, 3)
            .contentShape(Rectangle())
            // An agenda row is the **event**, not one of its occurrences, the list holds one row
            // per series, so it names none, and a write from here is a series write.
            .onTapGesture {
                onOpen(EventRefID(account: event.account, key: event.key, occurrence: ""))
            }
            .swipeActions(edge: .trailing, allowsFullSwipe: true) {
                // Hidden, not disabled, on a row whose account cannot write: a swipe action
                // that can never fire is just a mystery (see EventRow.offersDelete).
                if event.offersDelete {
                    Button(role: .destructive) {
                        model.deleteEvent(event.account, event.key)
                    } label: {
                        Label(L10n.action_delete(), systemImage: "trash")
                    }
                }
            }
        }
    }
}

/// A calendar's identity for a list or a sheet.
///
/// `CalendarRow` already carries an `id`, the calendar's provider key, but that is unique only
/// WITHIN its account. Two accounts can each have a `work`, and keying a list on the bare id would
/// collide them: SwiftUI would reuse one row's state for the other. So the identity is the pair.
struct PickedCalendar: Identifiable {
    let row: CalendarRow
    var id: String { calendarRowID(row) }
}

extension CalendarRow {
    /// The identity of a calendar row: its account AND its id.
    var rowIdentity: String { "\(account):\(id)" }
}

/// The identity of a calendar row: its account AND its id.
func calendarRowID(_ row: CalendarRow) -> String { row.rowIdentity }
