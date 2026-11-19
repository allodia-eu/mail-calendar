// The calendar pane's mini month (`docs/calendar-pane.md`, rules 2 to 5 and 11): the core's six
// weeks drawn as buttons. Which month, which days are banded and the week numbers are all the
// core's; this view only multiplies the pane's width into a cell size.

import MailcalBindings
import SwiftUI

struct CalendarPaneMonth: View {
    let month: MiniMonth
    /// The calendar dates are read and worded in: the display zone's.
    let calendar: Calendar
    /// An arrow was pressed: `MiniMonth.previous` or `next`.
    let onPage: (String) -> Void
    /// A day was pressed.
    let onDay: (String) -> Void

    @State private var width: CGFloat = 0
    @FocusState private var focused: String?
    /// The day under the pointer, which a Mac answers with a soft circle.
    @State private var hovered: String?

    /// A leading column of week numbers and seven days share the width the column offers, capped so a
    /// wide pane keeps the circles round and small.
    private var cell: CGFloat { min((width / 8).rounded(.down), 30) }

    private var days: [MiniDay] { month.weeks.flatMap(\.days) }

    var body: some View {
        // A geometry reader is as wide as the column offers, whatever the days inside it take, so the
        // cell size cannot feed back into the width it was measured from. Its height is the grid's.
        GeometryReader { proxy in
            grid
                .frame(width: proxy.size.width)
                .onAppear { width = proxy.size.width }
                .onChange(of: proxy.size.width) { _, next in width = next }
        }
        .frame(height: headerHeight + cell * 0.8 + cell * CGFloat(month.weeks.count) + 2 * 7)
    }

    private let headerHeight: CGFloat = 32

    private var grid: some View {
        VStack(spacing: 2) {
            header
            HStack(spacing: 0) {
                Color.clear.frame(width: cell, height: cell * 0.8)
                ForEach(Array(month.weekdays.enumerated()), id: \.offset) { _, day in
                    Text(weekdayInitial(day))
                        .font(.caption2)
                        .foregroundStyle(.secondary)
                        .frame(width: cell, height: cell * 0.8)
                }
            }
            .accessibilityHidden(true)
            ForEach(Array(month.weeks.enumerated()), id: \.offset) { _, week in
                HStack(spacing: 0) {
                    Text("\(week.number)")
                        .font(.caption2)
                        .monospacedDigit()
                        .foregroundStyle(.tertiary)
                        .frame(width: cell, height: cell)
                        .accessibilityLabel(L10n.calendar_week_number(number: "\(week.number)"))
                    ForEach(week.days, id: \.date) { day in dayButton(day) }
                }
            }
        }
    }

    private var header: some View {
        HStack(spacing: 4) {
            Text(title)
                .font(.headline)
                .lineLimit(1)
                .minimumScaleFactor(0.8)
                .accessibilityIdentifier("CalendarPaneMonthTitle")
            Spacer(minLength: 0)
            Button { onPage(month.previous) } label: { Image(systemName: "chevron.left") }
                .accessibilityLabel(L10n.calendar_prev_month())
                .accessibilityIdentifier("CalendarPanePrevious")
            Button { onPage(month.next) } label: { Image(systemName: "chevron.right") }
                .accessibilityLabel(L10n.calendar_next_month())
                .accessibilityIdentifier("CalendarPaneNext")
        }
        .buttonStyle(.borderless)
        .frame(height: headerHeight)
    }

    private var title: String {
        parseISODate(month.first, calendar: calendar).map { monthTitle($0, calendar: calendar) }
            ?? month.first
    }

    private func dayButton(_ day: MiniDay) -> some View {
        Button { onDay(day.date) } label: {
            Text("\(day.day)")
                .font(.caption)
                .monospacedDigit()
                .foregroundStyle(day.isToday ? AnyShapeStyle(.white) : day.inMonth
                    ? AnyShapeStyle(.primary) : AnyShapeStyle(.tertiary))
                .frame(width: cell, height: cell)
                .background {
                    if day.isToday {
                        Circle().fill(Color.accentColor).padding(2)
                    } else if hovered == day.date {
                        Circle().fill(Color.primary.opacity(0.1)).padding(2)
                    }
                }
                .background { band(day.shown) }
                // The calendar's focus date: a ring, over today's fill as over any other day.
                .overlay {
                    if day.isFocus { Circle().strokeBorder(Color.accentColor, lineWidth: 1.5) }
                }
                .contentShape(Rectangle())
        }
        .buttonStyle(.plain)
        .onHover { inside in
            if inside { hovered = day.date } else if hovered == day.date { hovered = nil }
        }
        .focused($focused, equals: day.date)
        .onKeyPress(.leftArrow) { move(from: day.date, by: -1) }
        .onKeyPress(.rightArrow) { move(from: day.date, by: 1) }
        .onKeyPress(.upArrow) { move(from: day.date, by: -7) }
        .onKeyPress(.downArrow) { move(from: day.date, by: 7) }
        .accessibilityLabel(label(day))
        .accessibilityIdentifier("CalendarPaneDay_\(day.date)")
    }

    /// The soft band under the days the grid shows, rounded only where it ends within this row.
    @ViewBuilder private func band(_ span: ShownSpan) -> some View {
        let radius = cell / 2
        let fill = Color.accentColor.opacity(0.18)
        switch span {
        case .outside: Color.clear
        case .only: Capsule().fill(fill)
        case .start:
            UnevenRoundedRectangle(topLeadingRadius: radius, bottomLeadingRadius: radius).fill(fill)
        case .middle: Rectangle().fill(fill)
        case .end:
            UnevenRoundedRectangle(bottomTrailingRadius: radius, topTrailingRadius: radius).fill(fill)
        }
    }

    private func move(from date: String, by step: Int) -> KeyPress.Result {
        guard let index = days.firstIndex(where: { $0.date == date }),
              days.indices.contains(index + step) else { return .ignored }
        focused = days[index + step].date
        return .handled
    }

    /// The full date, then "today", "selected" and "shown in the calendar" where they apply.
    private func label(_ day: MiniDay) -> String {
        var parts: [String] = []
        if let date = parseISODate(day.date, calendar: calendar) {
            let style = Date.FormatStyle(date: .complete, time: .omitted, locale: L10n.appLocale,
                                         calendar: calendar, timeZone: calendar.timeZone)
            parts.append(date.formatted(style))
        } else {
            parts.append(day.date)
        }
        if day.isToday { parts.append(L10n.calendar_today()) }
        if day.isFocus { parts.append(L10n.a11y_calendar_focus_day()) }
        if day.shown != .outside { parts.append(L10n.a11y_calendar_pane_day_shown()) }
        return parts.joined(separator: ", ")
    }
}
