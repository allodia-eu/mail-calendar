// The chips the grid draws: the timed blocks inside it, and the bars above it.
//
// The core solved the overlaps. `column`/`columns` are its answer: an event in a cluster of three
// sits in lane `column` of `columns`, so the width is that lane's share of the day. The client never
// re-packs, if it did, two clients could column identical data differently.

import MailcalBindings
import SwiftUI

private let blockGap: CGFloat = 1
private let blockPadding: CGFloat = 1
private let blockCorner: CGFloat = 4

private func blockInset(minutes: Int) -> CGFloat {
    minutes < 30 ? blockGap : blockGap + blockPadding
}

/// The vertical space a `minutes`-long block leaves for its label, at this zoom.
func blockLabelSpace(minutes: Int, hourHeight: CGFloat) -> CGFloat {
    hourHeight * (CGFloat(minutes) / 60) - blockInset(minutes: minutes) * 2
}

/// The line height a `minutes`-long block's label gets.
func blockLabelLineHeight(minutes: Int) -> CGFloat { minutes < 30 ? 11 : 14 }

/// The type size that goes with it.
func blockLabelFontSize(minutes: Int) -> CGFloat { minutes < 30 ? 9 : 11 }

/// Whether a block is tall enough to hold its own title **at this zoom**.
///
/// Zoomed out to the whole day, a 15-minute event is a few points tall and *cannot* hold text, so it
/// doesn't get any, rather than getting a title cut through the middle. It stays a coloured block,
/// keeps its full spoken label for VoiceOver, and reveals its title when the user zooms in. This is
/// what every good calendar does, and it is why the rule has to be a function of the zoom rather than
/// a constant.
func blockShowsLabel(minutes: Int, hourHeight: CGFloat) -> Bool {
    blockLabelSpace(minutes: minutes, hourHeight: hourHeight) >= blockLabelLineHeight(minutes: minutes)
}

/// A block only earns a second line (its start time) once there is room for two.
func blockShowsTime(minutes: Int, hourHeight: CGFloat) -> Bool {
    blockLabelSpace(minutes: minutes, hourHeight: hourHeight)
        >= blockLabelLineHeight(minutes: minutes) * 2
}

/// One event, inside one day column.
struct CalendarTimedBlock: View {
    let segment: TimedSegment
    let calendars: [CalendarRow]
    let dayWidth: CGFloat
    let hourHeight: CGFloat
    let use24Hour: Bool
    let dark: Bool
    /// Whether a tall enough block earns its start time on a second line.
    ///
    /// `false` on the invitation card's meeting preview, whose blocks carry a **title only**, the
    /// twin of Android's `drawPreviewChip` and Windows' `Chip`, which never had a second line at
    /// all. It is not a taste difference: that preview shows a *band* of the day, so a block
    /// beginning above the band is drawn with its top off-screen, and the second line was the one
    /// piece of it low enough to survive, a bare "11:00" floating under the all-day bar with no
    /// title above it. The block itself is meant to read as a slab there, exactly as a scrolled
    /// calendar's does.
    var showsTime: Bool = true
    /// A tap opens the event's detail.
    let onOpen: () -> Void

    private var start: Int { Int(segment.startMinutes) }
    private var end: Int { Int(segment.endMinutes) }
    private var minutes: Int { end - start }

    /// Whether this block draws its start time: only where the caller allows one *and* there is
    /// room for a second line.
    private var withTime: Bool {
        showsTime && blockShowsTime(minutes: minutes, hourHeight: hourHeight)
    }

    var body: some View {
        let calendar = calendars.row(account: segment.account, calendar: segment.calendar)
        let swatch = calendar.swatchOrFallback(dark: dark)
        let title = segment.title.isEmpty ? L10n.event_no_title() : segment.title
        let columnWidth = dayWidth / CGFloat(segment.columns)

        VStack(alignment: .leading, spacing: 0) {
            if blockShowsLabel(minutes: minutes, hourHeight: hourHeight) {
                Text(title)
                    .font(.system(size: blockLabelFontSize(minutes: minutes)))
                    .lineLimit(withTime || minutes < 30 ? 1 : 2)
                    .foregroundStyle(parseHexColor(swatch.text))
                if withTime {
                    Text(clockTime(start, use24Hour: use24Hour))
                        .font(.system(size: blockLabelFontSize(minutes: minutes)))
                        .lineLimit(1)
                        .foregroundStyle(parseHexColor(swatch.text))
                }
            }
            Spacer(minLength: 0)
        }
        .padding(.horizontal, 4)
        .padding(.vertical, blockInset(minutes: minutes) - blockGap)
        .frame(
            width: max(columnWidth - blockGap * 2, 0),
            height: max(hourHeight * (CGFloat(minutes) / 60) - blockGap * 2, 0),
            alignment: .topLeading
        )
        .participationFill(segment.participation, color: parseHexColor(swatch.background))
        .clipShape(RoundedRectangle(cornerRadius: blockCorner))
        .holdHatch(
            segment.participation,
            color: parseHexColor(swatch.border),
            cornerRadius: blockCorner
        )
        .overlay(
            RoundedRectangle(cornerRadius: blockCorner)
                .strokeBorder(
                    parseHexColor(swatch.border),
                    style: participationStroke(segment.participation)
                )
        )
        .offset(
            x: dayWidth * CGFloat(segment.day) + columnWidth * CGFloat(segment.column) + blockGap,
            y: hourHeight * (CGFloat(start) / 60) + blockGap
        )
        .onTapGesture(perform: onOpen)
        // Unconditional: a block too short to show its title still announces it, and an unanswered
        // hold says so, because its dashed border is invisible to a screen reader.
        .accessibilityLabel(
            calendarEventLabel(
                title: title,
                time: timeRange(start, end, use24Hour: use24Hour),
                calendar: calendar?.name ?? "",
                participation: segment.participation
            )
        )
    }
}

/// One all-day/multi-day bar, spanning `band.days` columns from `band.day` in lane `band.lane`.
struct CalendarAllDayChip: View {
    let band: AllDayBand
    let calendars: [CalendarRow]
    let dayWidth: CGFloat
    let dark: Bool
    /// A tap opens the event's detail.
    let onOpen: () -> Void

    var body: some View {
        let calendar = calendars.row(account: band.account, calendar: band.calendar)
        let swatch = calendar.swatchOrFallback(dark: dark)
        let title = band.title.isEmpty ? L10n.event_no_title() : band.title

        Text(title)
            .font(.system(size: 10))
            .lineLimit(1)
            .foregroundStyle(parseHexColor(swatch.text))
            .padding(.horizontal, 5)
            .frame(
                width: max(dayWidth * CGFloat(band.days) - blockGap * 2, 0),
                height: calendarLaneHeight - blockGap * 2,
                alignment: .leading
            )
            .participationFill(band.participation, color: parseHexColor(swatch.background))
            .clipShape(RoundedRectangle(cornerRadius: blockCorner))
            .holdHatch(
                band.participation,
                color: parseHexColor(swatch.border),
                cornerRadius: blockCorner
            )
            .holdBorder(
                band.participation,
                color: parseHexColor(swatch.border),
                cornerRadius: blockCorner
            )
            .offset(
                x: dayWidth * CGFloat(band.day) + blockGap,
                y: calendarLaneHeight * CGFloat(band.lane) + blockGap
            )
            .onTapGesture(perform: onOpen)
            .accessibilityLabel(
                calendarEventLabel(
                    title: title,
                    time: L10n.calendar_all_day(),
                    calendar: calendar?.name ?? "",
                    participation: band.participation
                )
            )
    }
}
