// The chrome above the time grid: the day headings, and the all-day banner.
//
// Both are laid out from the same strip position the grid is, so a column and its heading can never
// drift apart, and both scroll past the same pinned gutter.

import MailcalBindings
import SwiftUI

/// The column headings: the ISO week number in the gutter, a Dutch/German convention worth keeping:
/// then each day's weekday and date, with today's circled.
struct CalendarDayHeader: View {
    let weeks: [CalendarStripWeek]
    let strip: CalendarStrip
    let today: Date
    let dayWidth: CGFloat
    let calendar: Calendar

    var body: some View {
        HStack(alignment: .center, spacing: 0) {
            VStack(spacing: 0) {
                Text(L10n.calendar_week_short())
                    .font(.caption2)
                    .foregroundStyle(.secondary)
                Text("\(weekNumber)")
                    .font(.callout)
                    .foregroundStyle(.secondary)
            }
            .frame(width: calendarGutter)
            .accessibilityLabel(L10n.calendar_week_number(number: "\(weekNumber)"))

            ZStack(alignment: .topLeading) {
                Color.clear
                ForEach(weeks) { week in
                    CalendarWeekHeadings(
                        days: week.days, today: today, dayWidth: dayWidth, calendar: calendar
                    )
                    .equatable()
                    .offset(x: strip.origin(ofWeek: week.index, dayWidth: dayWidth))
                }
            }
            .frame(maxWidth: .infinity, alignment: .leading)
            .clipped()
        }
        .padding(.vertical, 5)
    }

    /// The week of the **leftmost** column on screen, which is the one the gutter sits beside.
    private var weekNumber: Int {
        guard let first = weeks.first?.days.first else { return 0 }
        return isoWeekNumber(first, zone: calendar.timeZone)
    }
}

/// One week's day headings. Equatable, so a frame that only slides the strip leaves them alone.
struct CalendarWeekHeadings: View, Equatable {
    let days: [Date]
    let today: Date
    let dayWidth: CGFloat
    let calendar: Calendar

    var body: some View {
        HStack(spacing: 0) {
            ForEach(Array(days.enumerated()), id: \.offset) { _, date in
                heading(date)
            }
        }
        .frame(width: dayWidth * CGFloat(daysInWeek), alignment: .leading)
    }

    private func heading(_ date: Date) -> some View {
        let isToday = calendar.isDate(date, inSameDayAs: today)
        return VStack(spacing: 2) {
            Text(weekdayShort(date, calendar: calendar))
                .font(.caption2)
                .foregroundStyle(isToday ? Color.accentColor : Color.secondary)
                .lineLimit(1)
            Text("\(calendar.component(.day, from: date))")
                .font(.callout.weight(isToday ? .semibold : .regular))
                .foregroundStyle(isToday ? Color.white : Color.primary)
                .frame(width: 26, height: 26)
                .background(isToday ? Color.accentColor : Color.clear, in: Circle())
        }
        .frame(width: dayWidth)
    }
}

/// The band above the grid: all-day and multi-day events, each spanning whole day columns.
///
/// The core stacks them into non-colliding lanes; this caps how many are on screen. Past the cap the
/// last row becomes a per-day "+N" chip and the band becomes tappable to expand, so a busy week never
/// grows a banner that swallows the grid, and nothing is hidden without saying so.
///
/// `lanes` is the largest count across the weeks on screen, not each week's own: with the hour ruler
/// pinned there is one content top for the whole surface (see `CalendarGridView.bannerLanes`).
struct CalendarAllDayBanner: View {
    let weeks: [CalendarStripWeek]
    let strip: CalendarStrip
    let lanes: Int
    let dayWidth: CGFloat
    let dark: Bool
    /// A tap on an all-day band opens that event's detail.
    let onOpen: (EventRefID) -> Void

    @State private var expanded = false

    var body: some View {
        let overflows = allDayOverflows(lanes: lanes)
        let drawnLanes = allDayDrawnLanes(lanes: lanes, expanded: expanded)
        let bannerLanes = allDayBannerLanes(lanes: lanes, expanded: expanded)

        HStack(alignment: .top, spacing: 0) {
            Text(L10n.calendar_all_day())
                .font(.caption2)
                .foregroundStyle(.secondary)
                .multilineTextAlignment(.trailing)
                .frame(width: calendarGutter, alignment: .trailing)
                .padding(.trailing, 6)

            ZStack(alignment: .topLeading) {
                Color.clear
                ForEach(weeks) { week in
                    CalendarWeekBands(
                        week: week,
                        drawnLanes: drawnLanes,
                        showsMore: overflows && !expanded,
                        dayWidth: dayWidth,
                        dark: dark,
                        onOpen: onOpen
                    )
                    .equatable()
                    .frame(
                        width: dayWidth * CGFloat(daysInWeek),
                        height: calendarLaneHeight * CGFloat(bannerLanes),
                        alignment: .topLeading
                    )
                    .offset(x: strip.origin(ofWeek: week.index, dayWidth: dayWidth))
                }
            }
            .frame(height: calendarLaneHeight * CGFloat(bannerLanes))
            .frame(maxWidth: .infinity, alignment: .leading)
            .clipped()
        }
        .contentShape(Rectangle())
        .onTapGesture {
            // Only tappable when there is actually something to reveal.
            if overflows { expanded.toggle() }
        }
    }
}

/// One week's all-day bars, in its own coordinates. Equatable on what it draws, so a frame that only
/// slides the strip leaves the week's bars alone; `onOpen` is not compared.
struct CalendarWeekBands: View, Equatable {
    let week: CalendarStripWeek
    let drawnLanes: Int
    /// Whether the banner is capped, so a day with more bars than lanes says "+N".
    let showsMore: Bool
    let dayWidth: CGFloat
    let dark: Bool
    let onOpen: (EventRefID) -> Void

    nonisolated static func == (lhs: Self, rhs: Self) -> Bool {
        lhs.week.index == rhs.week.index && lhs.week.days == rhs.week.days
            && lhs.week.page.allDay == rhs.week.page.allDay
            && lhs.week.page.calendars == rhs.week.page.calendars
            && lhs.drawnLanes == rhs.drawnLanes && lhs.showsMore == rhs.showsMore
            && lhs.dayWidth == rhs.dayWidth && lhs.dark == rhs.dark
    }

    var body: some View {
        let hidden = allDayOverflowPerDay(
            bands: week.page.allDay, dayCount: week.days.count, drawnLanes: drawnLanes
        )
        ZStack(alignment: .topLeading) {
            ForEach(week.page.allDay.filter { Int($0.lane) < drawnLanes }, id: \.rowID) { band in
                CalendarAllDayChip(
                    band: band, calendars: week.page.calendars, dayWidth: dayWidth, dark: dark,
                    onOpen: {
                        onOpen(
                            EventRefID(
                                account: band.account,
                                key: band.event,
                                occurrence: band.occurrenceStart
                            )
                        )
                    }
                )
            }
            if showsMore {
                ForEach(Array(hidden.enumerated()), id: \.offset) { day, count in
                    if count > 0 {
                        Text(L10n.calendar_all_day_more(count: count))
                            .font(.system(size: 10))
                            .foregroundStyle(.secondary)
                            .padding(.horizontal, 5)
                            .frame(width: dayWidth, height: calendarLaneHeight, alignment: .leading)
                            .offset(
                                x: dayWidth * CGFloat(day),
                                y: calendarLaneHeight * CGFloat(drawnLanes)
                            )
                            .accessibilityLabel(L10n.calendar_all_day_expand(count: count))
                    }
                }
            }
        }
    }
}
