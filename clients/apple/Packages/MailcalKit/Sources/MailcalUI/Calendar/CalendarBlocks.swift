// The chips the grid draws: the timed blocks inside it, and the bars above it.
//
// The core solved the overlaps. `column`/`columns` are its answer: an event in a cluster of three
// sits in lane `column` of `columns`, so the width is that lane's share of the day. The client never
// re-packs, if it did, two clients could column identical data differently.

import MailcalBindings
import SwiftUI

let blockGap: CGFloat = 1
private let blockPadding: CGFloat = 1
let blockCorner: CGFloat = 4
/// The bar of the calendar's colour down an event's leading edge.
let eventBarWidth: CGFloat = 3

func blockInset(minutes: Int) -> CGFloat {
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

/// How many lines the grid's title may wrap over: every whole line the block has room for, less the
/// one its start time takes when it draws one. As Apple's Calendar does, a long title wraps rather
/// than stopping at the first line, and the last line drawn is always whole. Zero where there is no
/// room for a label.
func blockTitleLines(minutes: Int, hourHeight: CGFloat, withTime: Bool = true) -> Int {
    guard blockShowsLabel(minutes: minutes, hourHeight: hourHeight) else { return 0 }
    let whole = Int(
        (blockLabelSpace(minutes: minutes, hourHeight: hourHeight)
            / blockLabelLineHeight(minutes: minutes)).rounded(.down)
    )
    let time = withTime && blockShowsTime(minutes: minutes, hourHeight: hourHeight)
    return max(whole - (time ? 1 : 0), 1)
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
        let colors = calendar.eventColorsOrFallback(dark: dark)
        let title = segment.title.isEmpty ? L10n.event_no_title() : segment.title
        let columnWidth = dayWidth / CGFloat(segment.columns)

        VStack(alignment: .leading, spacing: 0) {
            if blockShowsLabel(minutes: minutes, hourHeight: hourHeight) {
                EventTitleLine(
                    title: title,
                    hasConference: segment.hasConference,
                    lineLimit: max(
                        blockTitleLines(minutes: minutes, hourHeight: hourHeight, withTime: showsTime),
                        1
                    )
                )
                .font(.system(size: blockLabelFontSize(minutes: minutes), weight: .semibold))
                .foregroundStyle(colors.text)
                if withTime {
                    Text(clockTime(start, use24Hour: use24Hour))
                        .font(.system(size: blockLabelFontSize(minutes: minutes)))
                        .lineLimit(1)
                        .foregroundStyle(colors.text)
                }
            }
            Spacer(minLength: 0)
        }
        .padding(.leading, eventBarWidth + 4)
        .padding(.trailing, 4)
        .padding(.vertical, blockInset(minutes: minutes) - blockGap)
        .frame(
            width: max(columnWidth - blockGap * 2, 0),
            height: max(hourHeight * (CGFloat(minutes) / 60) - blockGap * 2, 0),
            alignment: .topLeading
        )
        .eventBlockStyle(segment.participation, colors: colors, cornerRadius: blockCorner)
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
                participation: segment.participation,
                hasConference: segment.hasConference,
                repeats: !segment.occurrenceStart.isEmpty
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
        let colors = calendar.eventColorsOrFallback(dark: dark)
        let title = band.title.isEmpty ? L10n.event_no_title() : band.title

        EventTitleLine(title: title, hasConference: band.hasConference)
            .font(.system(size: 10, weight: .semibold))
            .foregroundStyle(colors.text)
            .padding(.leading, eventBarWidth + 4)
            .padding(.trailing, 5)
            .frame(
                width: max(dayWidth * CGFloat(band.days) - blockGap * 2, 0),
                height: calendarLaneHeight - blockGap * 2,
                alignment: .leading
            )
            .eventBlockStyle(band.participation, colors: colors, cornerRadius: blockCorner)
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
                    participation: band.participation,
                    hasConference: band.hasConference,
                    repeats: !band.occurrenceStart.isEmpty
                )
            )
    }
}

extension View {
    /// An event's block, as the drawn grid draws it: a tint of its calendar's colour, the colour
    /// itself down the leading edge, and for an unanswered hold a fainter tint, a hatched edge and a
    /// dashed border instead (`CalendarWeekContent.draw`).
    func eventBlockStyle(
        _ participation: ResponseStatus, colors: EventColors, cornerRadius: CGFloat
    ) -> some View {
        let hold = isAwaitingResponse(participation)
        return background(colors.fill.opacity(hold ? holdFillOpacity : 1))
            .overlay(alignment: .leading) {
                if hold {
                    ParticipationHatch(color: colors.accent).frame(width: eventBarWidth + 1)
                } else {
                    colors.accent.frame(width: eventBarWidth)
                }
            }
            .clipShape(RoundedRectangle(cornerRadius: cornerRadius))
            .holdBorder(participation, color: colors.accent, cornerRadius: cornerRadius)
    }
}
