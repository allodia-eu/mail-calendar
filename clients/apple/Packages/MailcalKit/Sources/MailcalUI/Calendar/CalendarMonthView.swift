// The month grid: six weeks of day cells, each listing what happens that day.
//
// A different layout from the time grid, not the same one with more columns, a cell has no hour axis
// and no overlap solving, only a list. The core hands back every event on every day; how many chips
// fit is a question of how tall a cell is on *this* screen, so the cap and the "+N more" are computed
// here (the same division of labour as the all-day banner).

import MailcalBindings
import SwiftUI

private let monthWeeks = 6
private let monthChipHeight: CGFloat = 15

/// How many chips fit in the space a cell leaves below its date number.
func monthChipCapacity(_ chipArea: CGFloat) -> Int {
    max(Int((chipArea + 2) / monthChipHeight), 0)
}

/// How many chips a cell actually draws, given what fits.
///
/// The subtlety: if a cell has exactly one more event than fits, drawing "+N more" in the last slot
/// *costs* a slot, so it would hide two to report one. In that case draw the event instead. The
/// overflow row only earns its place when it stands for more than it displaces.
func monthChipsShown(total: Int, capacity: Int) -> Int {
    total <= capacity ? total : max(capacity - 1, 0)
}

struct CalendarMonthView: View {
    let page: MonthPage
    let today: Date
    let calendar: Calendar
    let weekStartsMonday: Bool
    /// Only reaches the chips' spoken labels, a month chip is too small to print a time.
    let use24Hour: Bool
    /// A tap on a chip opens the event's detail.
    let onOpen: (EventRefID) -> Void

    @Environment(\.colorScheme) private var colorScheme

    var body: some View {
        VStack(spacing: 0) {
            weekdayHeader
            Divider()
            GeometryReader { geometry in
                let cellHeight = geometry.size.height / CGFloat(monthWeeks)
                VStack(spacing: 0) {
                    ForEach(0..<monthWeeks, id: \.self) { week in
                        HStack(spacing: 0) {
                            ForEach(0..<7, id: \.self) { column in
                                let index = week * 7 + column
                                if index < page.cells.count {
                                    cell(page.cells[index], height: cellHeight)
                                }
                            }
                        }
                        .frame(height: cellHeight)
                        Divider()
                    }
                }
            }
        }
    }

    /// The weekday headings, in the locale's own abbreviations and starting on the user's chosen day.
    private var weekdayHeader: some View {
        // A reference week: 2026-07-06 is a Monday, 2026-07-05 a Sunday.
        let first = referenceWeekStart
        return HStack(spacing: 0) {
            ForEach(0..<7, id: \.self) { offset in
                if let date = calendar.date(byAdding: .day, value: offset, to: first) {
                    Text(weekdayShort(date, calendar: calendar))
                        .font(.caption2)
                        .foregroundStyle(.secondary)
                        .frame(maxWidth: .infinity)
                }
            }
        }
        .padding(.vertical, 4)
    }

    private var referenceWeekStart: Date {
        var parts = DateComponents()
        parts.year = 2026
        parts.month = 7
        parts.day = weekStartsMonday ? 6 : 5
        return calendar.date(from: parts) ?? today
    }

    @ViewBuilder
    private func cell(_ cell: MonthCell, height: CGFloat) -> some View {
        let date = parseISODate(cell.date, calendar: calendar)
        let isToday = date.map { calendar.isDate($0, inSameDayAs: today) } ?? false
        // How many chips fit is decided from the cell's real height, the core does not guess at a
        // phone's row height, so it hands back every event and lets this cap them.
        let capacity = monthChipCapacity(height - 22)
        let shown = monthChipsShown(total: cell.chips.count, capacity: capacity)
        let hidden = cell.chips.count - shown

        VStack(spacing: 1) {
            Text(date.map { "\(calendar.component(.day, from: $0))" } ?? "")
                .font(.caption2.weight(isToday ? .bold : .regular))
                .foregroundStyle(dayColor(isToday: isToday, inMonth: cell.inMonth))
                .frame(width: 18, height: 18)
                .background(isToday ? Color.accentColor : Color.clear, in: Circle())

            ForEach(cell.chips.prefix(shown), id: \.rowID) { chip in
                let calendarRow = page.calendars.row(account: chip.account, calendar: chip.calendar)
                let swatch = calendarRow.swatchOrFallback(dark: colorScheme == .dark)
                let title = chip.title.isEmpty ? L10n.event_no_title() : chip.title
                Text(title)
                    .font(.system(size: 9))
                    .lineLimit(1)
                    .foregroundStyle(parseHexColor(swatch.text))
                    .padding(.horizontal, 2)
                    .frame(maxWidth: .infinity, alignment: .leading)
                    .participationFill(chip.participation, color: parseHexColor(swatch.background))
                    .clipShape(RoundedRectangle(cornerRadius: 2))
                    // A month chip is a few points tall, so the hatch does the work the dashes cannot
                    // at this size, and the spoken label says it either way.
                    .holdHatch(chip.participation, color: parseHexColor(swatch.text), cornerRadius: 2, width: 3)
                    .holdBorder(chip.participation, color: parseHexColor(swatch.text), cornerRadius: 2)
                    .contentShape(Rectangle())
                    .onTapGesture {
                        onOpen(
                            EventRefID(
                                account: chip.account,
                                key: chip.event,
                                occurrence: chip.occurrenceStart
                            )
                        )
                    }
                    .accessibilityLabel(
                        calendarEventLabel(
                            title: title,
                            time: chip.allDay
                                ? L10n.calendar_all_day()
                                : clockTime(Int(chip.startMinutes), use24Hour: use24Hour),
                            calendar: calendarRow?.name ?? "",
                            participation: chip.participation
                        )
                    )
            }
            if hidden > 0 {
                Text(L10n.calendar_all_day_more(count: hidden))
                    .font(.system(size: 9))
                    .foregroundStyle(.secondary)
                    .frame(maxWidth: .infinity, alignment: .leading)
                    .padding(.horizontal, 2)
            }
            Spacer(minLength: 0)
        }
        .frame(maxWidth: .infinity, maxHeight: .infinity, alignment: .top)
        .padding(.horizontal, 1)
    }

    /// Days of the neighbouring months are dimmed, without that, the 1st of next month reads as part
    /// of this one and the user taps into the wrong month without noticing.
    private func dayColor(isToday: Bool, inMonth: Bool) -> Color {
        if isToday { return .white }
        return inMonth ? .primary : .secondary.opacity(0.5)
    }
}

extension MonthChip {
    var rowID: String { "\(account):\(event)" }
}
