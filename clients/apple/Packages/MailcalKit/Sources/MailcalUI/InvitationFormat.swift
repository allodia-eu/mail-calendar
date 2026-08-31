// Every localised string and every layout decision the invitation card makes, as plain functions
// over plain values.
//
// The core decides *whether there is a card and what is on it* (docs/invitations.md); this file
// decides how those values read in the user's language and how tall the preview's hours are. It is
// deliberately view-free so each rule is a check that can fail, the conflict wording, the
// attendee arithmetic, and the preview's hour span are all things a screenshot would not catch.
//
// Times arrive as UTC instants because the core ships no display tzdata (docs/timestamps.md), so
// every function here takes the display zone as an argument rather than reading a global.

import CoreGraphics
import Foundation
import MailcalBindings

/// A span of wall-clock minutes from midnight, the unit the core's grid solver emits.
struct MinuteSpan: Equatable {
    let start: Int
    let end: Int
}

/// The card's heading: what this message is, before any detail.
func invitationTitle(_ kind: InvitationKind) -> String {
    switch kind {
    case .rsvp: return L10n.invitation_title()
    case .cancelled: return L10n.invitation_cancelled_title()
    case .informational: return L10n.invitation_informational_title()
    case .superseded: return L10n.invitation_superseded_title()
    }
}

/// The sentence under the heading that says why this card offers no answer, or `nil` where none
/// is owed.
///
/// A superseded invitation is the one kind whose *absence* of buttons needs explaining in its own
/// right: the details still look answerable, so without this the card reads as broken rather than
/// out of date. The `canRespond` gate has its own sentence (`invitation_cannot_respond`) and is a
/// different fact, that account can never answer; this mail simply is not the current one.
func invitationNotice(_ kind: InvitationKind) -> String? {
    switch kind {
    case .superseded: return L10n.invitation_superseded()
    case .rsvp, .cancelled, .informational: return nil
    }
}

/// The subject line for the reply the core emails to the organiser, on an account whose calendar
/// server does no scheduling of its own.
///
/// Composed in the client because the core carries no locale (AGENTS.md → "Localisation is
/// client-side"), and this is copy a stranger reads in their inbox. Passing `nil` instead is safe
/// but silent: the core falls back to `Re:` plus the invitation's own subject, and the
/// organiser's message list then no longer says which way we answered.
func invitationReplySubject(_ response: InvitationResponse, _ summary: String) -> String {
    switch response {
    case .accept: return L10n.invitation_reply_subject_accepted(summary: summary)
    case .tentative: return L10n.invitation_reply_subject_tentative(summary: summary)
    case .decline: return L10n.invitation_reply_subject_declined(summary: summary)
    }
}

/// How this account has answered so far, in words.
func invitationResponseLine(_ status: ResponseStatus) -> String {
    switch status {
    case .needsAction: return L10n.invitation_response_needs_action()
    case .accepted: return L10n.invitation_response_accepted()
    case .declined: return L10n.invitation_response_declined()
    case .tentative: return L10n.invitation_response_tentative()
    case .delegated: return L10n.invitation_response_delegated()
    }
}

/// What else is in the user's calendar over the meeting's window, **in words**.
///
/// The count is stated rather than left to the preview grid: docs/calendar.md §4, a picture the
/// user has to read carefully is not a disclosure. Zero and one get their own wording because
/// "0 other things" and "1 other things" are not sentences.
///
/// `known: false` is **not** zero. It means the core could not read the calendar over this window:
/// on a cold start mail syncs before calendars, so an invitation opened straight away lands here, and
/// "Nothing else in your calendar then" would then be a confident lie over a calendar nobody read.
/// Same rule as the grid's `isMaterialized`.
func invitationConflictLine(count: UInt32, known: Bool) -> String {
    guard known else { return L10n.invitation_conflicts_unknown() }
    switch count {
    case 0: return L10n.invitation_conflicts_none()
    case 1: return L10n.invitation_conflicts_one()
    default: return L10n.invitation_conflicts(count: Int(count))
    }
}

/// How the invitation's attendees have answered, as the phrases to join.
///
/// Counts only, never a roster: the addresses belong to other people and are attacker-controlled
/// (docs/invitations.md). Every non-zero bucket earns a phrase, because the four sum to the total
/// and a line that leaves one out reads as arithmetic that does not add up. An invitation whose
/// only attendee is this account says so instead of "1 of 1 accepted".
func invitationAttendeeLines(_ tally: AttendeeTally) -> [String] {
    guard tally.total > 0 else { return [] }
    guard tally.total > 1 else { return [L10n.invitation_attendees_one()] }
    var lines = [
        L10n.invitation_attendees(
            accepted: String(tally.accepted),
            total: String(tally.total)
        )
    ]
    // One is its own string, per count, because the catalog has no plural machinery and Dutch
    // needs a different verb: "1 moeten nog antwoorden" is wrong, and one outstanding reply is the
    // commonest case on a small meeting. English reads fine either way, which is exactly why this
    // was invisible until the card was looked at in Dutch. Same shape as the conflict count.
    if tally.tentative > 0 {
        lines.append(
            tally.tentative == 1
                ? L10n.invitation_attendees_tentative_one()
                : L10n.invitation_attendees_tentative(count: Int(tally.tentative)))
    }
    if tally.declined > 0 {
        lines.append(
            tally.declined == 1
                ? L10n.invitation_attendees_declined_one()
                : L10n.invitation_attendees_declined(count: Int(tally.declined)))
    }
    if tally.needsAction > 0 {
        lines.append(
            tally.needsAction == 1
                ? L10n.invitation_attendees_pending_one()
                : L10n.invitation_attendees_pending(count: Int(tally.needsAction)))
    }
    return lines
}

/// The meeting's "when", localised in `zone`.
///
/// All-day shows the inclusive day(s), the stored end is exclusive, so a one-day event whose end
/// is the next midnight must read as one date, not two. A timed meeting collapses the date when
/// start and end share one. The clock honours the user's 12/24-hour **setting** rather than the
/// locale's default, so mail and calendar cannot disagree with each other.
///
/// Pure: no clock is read, so the same inputs always give the same string.
func invitationWhen(
    startsAt: String,
    endsAt: String,
    allDay: Bool,
    zone: String,
    use24Hour: Bool,
    locale: Locale = L10n.appLocale
) -> String {
    guard let start = parseUtcInstant(startsAt) else { return "" }
    let end = parseUtcInstant(endsAt) ?? start
    var calendar = Calendar(identifier: .gregorian)
    calendar.timeZone = TimeZone(identifier: zone) ?? .current
    calendar.locale = locale
    let day = dayFormatter(calendar: calendar, locale: locale)

    if allDay {
        let lastDay = calendar.date(byAdding: .day, value: -1, to: end) ?? start
        if lastDay <= start || calendar.isDate(lastDay, inSameDayAs: start) {
            return day.string(from: start)
        }
        return "\(day.string(from: start)) – \(day.string(from: lastDay))"
    }

    let from = clockTime(minutesOfDay(start, in: calendar), use24Hour: use24Hour)
    let to = clockTime(minutesOfDay(end, in: calendar), use24Hour: use24Hour)
    if calendar.isDate(start, inSameDayAs: end) {
        return "\(day.string(from: start)), \(from) – \(to)"
    }
    return "\(day.string(from: start)) \(from) – \(day.string(from: end)) \(to)"
}

/// The full weekday-and-date formatter the "when" line uses, in the locale's own field order.
private func dayFormatter(calendar: Calendar, locale: Locale) -> DateFormatter {
    let formatter = DateFormatter()
    formatter.calendar = calendar
    formatter.timeZone = calendar.timeZone
    formatter.locale = locale
    formatter.setLocalizedDateFormatFromTemplate("EEEEdMMMMy")
    return formatter
}

/// Wall-clock minutes from midnight, in `calendar`'s zone, the unit `clockTime` speaks.
private func minutesOfDay(_ date: Date, in calendar: Calendar) -> Int {
    let parts = calendar.dateComponents([.hour, .minute], from: date)
    return (parts.hour ?? 0) * 60 + (parts.minute ?? 0)
}

/// The hour band the meeting-day preview draws: `first..<last`, in whole hours.
///
/// **The meeting, everything it clashes with, and an hour of air**, padded a whole hour each side
/// so nothing sits flush against an edge, and never narrower than `minimumHours` so a 30-minute
/// meeting on an empty afternoon still has context around it.
///
/// It used to span the **whole day's** blocks, and that is the change: a normal working day runs
/// 08:00–22:00, so fourteen hours were squeezed into the preview's box and an hour came out about
/// ten points, under `blockShowsLabel`'s threshold, so the invitation's own block drew as an
/// *unnamed* rectangle beside a named one. A picture that shows *that* the afternoon is taken but
/// not *by what* answers the wrong question: the reader's next move is deciding whether the clash
/// matters, and they cannot without the title. Growing the box instead pushed the message itself
/// off the screen, which is worse, so the band narrows and the hours get taller.
///
/// **Nothing that the card counts can fall outside this.** A conflict is by definition an event
/// overlapping the meeting's own window, so every one of them is in `clashing` and its *whole*
/// extent is inside the band, a long booking that starts before the meeting drags `first` back
/// with it rather than being drawn cut off at the top edge with its title off-screen. What is left
/// out is the rest of the day, which the card states in words above the grid, and which the
/// disclosure label names (`invitation_conflicts_preview`: "Around this meeting", not "that day").
/// docs/calendar.md §4, nothing is hidden without saying so; this says so.
func invitationPreviewSpan(
    meeting: MinuteSpan,
    others: [MinuteSpan],
    minimumHours: Int = 6
) -> Range<Int> {
    // Half-open on both sides, exactly as `count_conflicts` overlaps in the core: back-to-back is
    // not a clash, so an event ending as the meeting starts does not widen the band.
    let clashing = others.filter { $0.start < meeting.end && meeting.start < $0.end }
    let spans = [meeting] + clashing
    let earliest = spans.map(\.start).min() ?? 0
    let latest = spans.map(\.end).max() ?? 60
    var first = max(earliest / 60 - 1, 0)
    // Ceil, so a block ending at 09:15 keeps the whole 09:00 hour, then pad.
    var last = min((latest + 59) / 60 + 1, 24)
    // Alternating, later hour first, so the meeting sits near the middle of the band rather than
    // pinned to its top, the hours after a meeting are the more interesting of the two.
    var growAfter = true
    while last - first < minimumHours, first > 0 || last < 24 {
        if growAfter, last < 24 {
            last += 1
        } else if first > 0 {
            first -= 1
        } else {
            last += 1
        }
        growAfter.toggle()
    }
    return first..<last
}

/// How tall the meeting-day preview draws, for a span of `hours`.
///
/// **Normally just `previewHeight`**, the band is narrow now (`invitationPreviewSpan` shows the
/// meeting and its clashes, not the whole day), so at six hours an hour already gets 25 points and
/// there is nothing to fix. This exists for the case the band *cannot* be narrow: an all-morning
/// booking the meeting sits inside drags the band out to ten or twelve hours, and at a fixed
/// height the blocks around it would go back to being unnamed rectangles. So an hour is allowed
/// `previewIdealHourHeight` and the box grows, up to `previewMaximumHeight`, past which this
/// stops being a preview sitting above a message and starts pushing the message off the screen.
///
/// Beyond that cap the hour height falls back below the ideal and short blocks quietly lose their
/// titles. That is the correct trade and not a hole in the rule above: nothing is ever *clipped*,
/// only unlabelled, and every block keeps its spoken label (`docs/calendar.md` §4).
///
/// The three numbers are *layout*, and a platform may hold its own; the formula is the rule, and
/// it is the same in `InvitationFormat.kt` and `InvitationFormat.cs`.
func invitationPreviewHeight(hours: Int) -> CGFloat {
    let ideal = CGFloat(max(hours, 1)) * previewIdealHourHeight
    return min(max(ideal, previewHeight), previewMaximumHeight)
}

/// The height one hour wants: enough that a 60-minute block clears `blockShowsLabel` (which needs
/// 14 points of label space after a 2-point inset each side) with a little to spare.
private let previewIdealHourHeight: CGFloat = 20

/// What the preview normally is, tall enough to read a morning against an afternoon, short enough
/// that the message body is still the thing on screen.
private let previewHeight: CGFloat = 150

/// The ceiling, for a band a long booking forced wide. A preview taller than this stops being one.
private let previewMaximumHeight: CGFloat = 240

/// How many hours apart the preview's labelled gridlines sit.
///
/// A squeezed span leaves no room to label every hour, two labels overlapping is worse than
/// three-hourly ones, so the stride is derived from the height a single hour actually gets.
func invitationPreviewStride(hourHeight: CGFloat) -> Int {
    guard hourHeight > 0 else { return 1 }
    return max(Int((18 / hourHeight).rounded(.up)), 1)
}

/// The spoken label for a calendar record, with the unanswered-hold disclosure appended.
///
/// The dashed border and hatched gutter that mark an unanswered invitation are **invisible to a
/// screen reader**, so the label has to say it, docs/calendar.md §4, the spoken-grid rule. Shared
/// by the grid block, the all-day bar, the month chip and the agenda row so one rule covers every
/// surface that can show a hold.
func calendarEventLabel(
    title: String,
    time: String,
    calendar: String,
    participation: ResponseStatus
) -> String {
    let base = L10n.calendar_event_a11y(title: title, time: time, calendar: calendar)
    guard participation == .needsAction else { return base }
    return "\(base), \(L10n.a11y_invitation_awaiting_response())"
}

/// Whether a calendar record is an invitation this account has not answered, the one condition
/// that turns on the provisional drawing (dashed border, hatched gutter, reduced fill).
///
/// `declined` never reaches a client: the core hides those from every calendar surface.
func isAwaitingResponse(_ participation: ResponseStatus) -> Bool {
    participation == .needsAction
}
