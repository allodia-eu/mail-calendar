// The meeting-invitation card, drawn above the message body.
//
// Everything on it was decided by the core (docs/invitations.md): whether there is a card at all, the
// organiser line, the attendee tally, the conflict count, and the meeting-day preview's geometry. This
// view localises and arranges; it computes no counts of its own, so this client and the next cannot
// disagree about whether a meeting clashes.
//
// SECURITY (Gate 8, docs/rendering-security.md), the summary, location, description and organizer
// name are attacker-controlled sender content, and they reach the screen without passing the HTML
// sanitiser, the CSP or a web view. So every one of them goes through `Text(verbatim:)`.
//
// A plain `Text(someString)` would already be safe, SwiftUI only parses markdown through the
// `LocalizedStringKey` overload, which a `String` variable cannot select. `verbatim:` is chosen anyway
// because it says so at the call site: it takes no other overload, so a later refactor that turns one
// of these into a string literal or an interpolation cannot silently start parsing `**bold**`. A title
// of `**bold** <b>x</b> & co` must appear exactly as typed. (The equivalent trap on GTK is
// `use_markup(false)`.)
//
// The conflict count is stated in WORDS beside the preview grid, always, docs/calendar.md §4: a
// picture the user has to read carefully is not a disclosure.

import MailcalBindings
import SwiftUI

/// The card's slot in the reading pane: the card when the open message is an invitation, and nothing
/// at all otherwise.
///
/// The decision is entirely the core's, its two-condition RSVP gate (a scheduling `METHOD` **and** an
/// `ATTENDEE` matching one of this account's own addresses, docs/invitations.md), so a published `.ics`
/// produces no card here and keeps its attachment chip instead. A stale snapshot for a
/// previously-opened message is already filtered out upstream.
struct InvitationBanner: View {
    let snapshot: ReadingSnapshot?
    let zone: String
    let use24Hour: Bool
    /// The message this card belongs to, so an answer can name it. The core resolves everything
    /// else, which address answers, which event it lands on, from the message alone.
    let account: String
    let messageKey: String
    let writeStatus: CalendarWriteStatus
    let respond: (InvitationResponse, String?, Bool, String) -> Void

    var body: some View {
        if let invitation = snapshot?.invitation {
            InvitationCardView(
                card: invitation,
                zone: zone,
                use24Hour: use24Hour,
                account: account,
                messageKey: messageKey,
                writeStatus: writeStatus,
                respond: respond
            )
        }
    }
}

struct InvitationCardView: View {
    let card: InvitationCard
    /// The user's display zone: the card's instants are UTC, and the host localises them.
    let zone: String
    let use24Hour: Bool
    let account: String
    let messageKey: String
    let writeStatus: CalendarWriteStatus
    let respond: (InvitationResponse, String?, Bool, String) -> Void

    @Environment(\.colorScheme) private var colorScheme
    /// Open whenever the calendar was actually read.
    ///
    /// It used to open only when the count was non-zero, "there is nothing to see, so save the
    /// room". That reasoning is wrong about what the grid is *for*: the question a person answering
    /// an invitation is asking is "what does my day look like", and the answer to that is the
    /// picture, not the number. "Nothing else in your calendar then" over a drawn, visibly empty day
    /// is a *stronger* answer than the same words over a collapsed row, and it is exactly the day
    /// on which the reader is deciding fastest.
    ///
    /// Still gated on `conflictsKnown`: an empty grid drawn over a calendar we have not read looks
    /// identical to a free day, which is the one thing this must never say (`docs/calendar.md` §4).
    @State private var showPreview: Bool

    init(
        card: InvitationCard,
        zone: String,
        use24Hour: Bool,
        account: String,
        messageKey: String,
        writeStatus: CalendarWriteStatus,
        respond: @escaping (InvitationResponse, String?, Bool, String) -> Void
    ) {
        self.card = card
        self.zone = zone
        self.use24Hour = use24Hour
        self.account = account
        self.messageKey = messageKey
        self.writeStatus = writeStatus
        self.respond = respond
        _showPreview = State(initialValue: card.conflictsKnown)
    }

    var body: some View {
        VStack(alignment: .leading, spacing: 6) {
            header
            if let notice = invitationNotice(card.kind) {
                Text(notice)
                    .font(.caption)
                    .foregroundStyle(.secondary)
                    .fixedSize(horizontal: false, vertical: true)
            }
            Text(verbatim: card.summary.isEmpty ? L10n.invitation_no_title() : card.summary)
                .font(.headline)
                .lineLimit(2)
            detail(L10n.invitation_organizer(), card.organizer)
            detail(L10n.invitation_when(), whenLine)
            if !card.location.isEmpty {
                detail(L10n.invitation_where(), card.location)
            }
            if card.recurring {
                Label(L10n.invitation_repeats(), systemImage: "repeat")
                    .font(.caption)
                    .foregroundStyle(.secondary)
            }
            description
            Divider()
            answer
            conflicts
        }
        .padding(10)
        .background(tint.opacity(0.08), in: RoundedRectangle(cornerRadius: 8))
        .overlay {
            RoundedRectangle(cornerRadius: 8).strokeBorder(tint.opacity(0.3), lineWidth: 1)
        }
        .padding(.horizontal, 12)
        .padding(.bottom, 8)
        // No container `accessibilityLabel`. `children: .contain` is supposed to keep the card's
        // own elements reachable, and on macOS it does, but on iOS, labelling the container
        // collapses the whole card into a single node. VoiceOver then reads "Meeting invitation
        // from Bob Tester" and stops: no time, no clash count, and **no Accept, Maybe or
        // Decline**. Answering an invitation became impossible with the screen reader on, in
        // the release that added the buttons.
        //
        // Nothing is lost by dropping it: the label only ever restated what the card already
        // shows in text, the "Meeting invitation" title and the "Organiser" line, which a
        // reader reaches directly now. The title carries the header trait instead, so rotor
        // navigation still lands on the card in one hop.
    }

    private var header: some View {
        HStack(spacing: 6) {
            Image(systemName: icon).foregroundStyle(tint)
            Text(invitationTitle(card.kind))
                .font(.subheadline.weight(.semibold))
                .foregroundStyle(tint)
                .accessibilityAddTraits(.isHeader)
            Spacer()
        }
    }

    /// A cancellation is the one kind that has to be unmissable, a stale hold otherwise sits in the
    /// calendar looking like a commitment.
    private var tint: Color {
        switch card.kind {
        case .rsvp: return .accentColor
        case .cancelled: return .red
        case .informational: return .secondary
        // Not red: nothing is wrong and nothing was lost, there is simply a newer copy to open.
        case .superseded: return .orange
        }
    }

    private var icon: String {
        switch card.kind {
        case .rsvp: return "calendar.badge.clock"
        case .cancelled: return "calendar.badge.minus"
        case .informational: return "calendar"
        case .superseded: return "calendar.badge.exclamationmark"
        }
    }

    private var whenLine: String {
        invitationWhen(
            startsAt: card.startsAt,
            endsAt: card.endsAt,
            allDay: card.allDay,
            zone: zone,
            use24Hour: use24Hour
        )
    }

    private func detail(_ label: String, _ value: String) -> some View {
        HStack(alignment: .firstTextBaseline, spacing: 6) {
            Text(label)
                .font(.caption)
                .foregroundStyle(.secondary)
                .frame(width: 72, alignment: .leading)
            Text(verbatim: value)
                .font(.caption)
                .textSelection(.enabled)
                .fixedSize(horizontal: false, vertical: true)
        }
    }

    /// The organiser's notes. Already truncated by the core (Gmail sends a wall of filler), and the
    /// card says so rather than implying the text ends there.
    @ViewBuilder
    private var description: some View {
        if !card.description.isEmpty {
            Text(verbatim: card.description)
                .font(.caption)
                .foregroundStyle(.secondary)
                .lineLimit(4)
                .textSelection(.enabled)
            if card.descriptionTruncated {
                Text(L10n.invitation_description_shortened())
                    .font(.caption2)
                    .foregroundStyle(.tertiary)
            }
        }
    }

    /// This account's own answer, how everyone else answered, and the buttons to change it.
    ///
    /// The answer line reads the **calendar's** copy, not the email's, the mail is frozen at the
    /// moment it was sent, so a card built from it would still say "you haven't answered" after
    /// you had. Only a card carrying an RSVP shows buttons: a cancellation has nothing to answer.
    private var answer: some View {
        VStack(alignment: .leading, spacing: 3) {
            HStack(spacing: 6) {
                Image(systemName: answerIcon).foregroundStyle(.secondary)
                Text(invitationResponseLine(card.myResponse)).font(.caption)
            }
            let attendees = invitationAttendeeLines(card.attendees)
            if !attendees.isEmpty {
                Text(attendees.joined(separator: " · "))
                    .font(.caption2)
                    .foregroundStyle(.secondary)
            }
            if card.kind == .rsvp {
                InvitationRespondView(
                    card: card,
                    account: account,
                    messageKey: messageKey,
                    status: writeStatus,
                    respond: respond
                )
                .padding(.top, 4)
            }
        }
    }

    private var answerIcon: String {
        switch card.myResponse {
        case .accepted: return "checkmark.circle"
        case .declined: return "xmark.circle"
        case .tentative: return "questionmark.circle"
        case .delegated: return "arrowshape.turn.up.right.circle"
        case .needsAction: return "clock"
        }
    }

    /// What else is in the calendar then, stated in words, then shown.
    ///
    /// The preview is offered only when the calendar was actually read. An empty grid drawn over an
    /// unread calendar looks exactly like a free day, which is the whole failure this guards.
    private var conflicts: some View {
        VStack(alignment: .leading, spacing: 4) {
            Text(invitationConflictLine(count: card.conflictCount, known: card.conflictsKnown))
                .font(.caption)
                .foregroundStyle(card.conflictsKnown && card.conflictCount > 0 ? .primary : .secondary)
            if card.conflictsKnown {
                DisclosureGroup(isExpanded: $showPreview) {
                    InvitationPreviewGrid(
                        preview: card.preview,
                        meeting: meetingSpan,
                        use24Hour: use24Hour,
                        dark: colorScheme == .dark
                    )
                    .padding(.top, 4)
                } label: {
                    Text(L10n.invitation_conflicts_preview())
                        .font(.caption2)
                        .foregroundStyle(.secondary)
                }
            }
        }
    }

    /// The meeting's own wall-clock window in the preview's layout zone.
    ///
    /// Included in the preview's hour span explicitly rather than relying on the hold the provider
    /// scheduled: a bare IMAP+SMTP account has no auto-scheduling server, so nothing lands on the
    /// grid and the meeting would otherwise be off the top of its own preview.
    private var meetingSpan: MinuteSpan {
        let layoutZone = card.preview.timezone.isEmpty ? zone : card.preview.timezone
        return meetingMinuteSpan(startsAt: card.startsAt, endsAt: card.endsAt, zone: layoutZone)
    }
}

/// The meeting-day preview: one day of the user's own calendar.
///
/// Laid out by the same `calendar::grid::build` every calendar surface uses and drawn with the same
/// block views, so the preview and the real grid cannot disagree, and an unanswered hold is dashed
/// here for the same reason it is dashed there.
///
/// The hour height is derived from the span rather than fixed, so every block on that day fits: the
/// preview never clips, which is what lets it stay a picture with no "and 2 more" caveat.
struct InvitationPreviewGrid: View {
    let preview: InvitationPreview
    let meeting: MinuteSpan
    let use24Hour: Bool
    let dark: Bool

    private let gutter: CGFloat = 34

    var body: some View {
        let span = invitationPreviewSpan(
            meeting: meeting,
            others: preview.timed.map {
                MinuteSpan(start: Int($0.startMinutes), end: Int($0.endMinutes))
            }
        )
        // Tall enough that the meeting's own block can carry its title, see
        // `invitationPreviewHeight`, which is why this is derived from the span rather than fixed.
        let gridHeight = invitationPreviewHeight(hours: span.count)
        let hourHeight = gridHeight / CGFloat(span.count)
        VStack(spacing: 0) {
            if preview.allDayLanes > 0 {
                allDayBanner
            }
            HStack(alignment: .top, spacing: 0) {
                ruler(span: span, hourHeight: hourHeight, gridHeight: gridHeight)
                GeometryReader { geometry in
                    grid(
                        span: span,
                        hourHeight: hourHeight,
                        gridHeight: gridHeight,
                        dayWidth: geometry.size.width
                    )
                }
                .frame(height: gridHeight)
            }
        }
    }

    /// One day, so a bar spans the full width and the banner is as tall as the core's lane count. No
    /// "+N" overflow: a single day's all-day events fit, and capping them here would hide something.
    private var allDayBanner: some View {
        GeometryReader { geometry in
            ZStack(alignment: .topLeading) {
                ForEach(preview.allDay, id: \.rowID) { band in
                    CalendarAllDayChip(
                        band: band,
                        calendars: [],
                        dayWidth: geometry.size.width,
                        dark: dark,
                        onOpen: {}
                    )
                }
            }
        }
        .frame(height: calendarLaneHeight * CGFloat(preview.allDayLanes))
        .padding(.leading, gutter)
    }

    /// The hour labels down the left edge.
    ///
    /// Every label is `.offset` into place, and an offset contributes nothing to layout, so the
    /// ZStack is only as tall as one label. It therefore has to be given the grid's height **before**
    /// being clipped: clipping first cropped all but the topmost label out of an 11-point box, which
    /// is exactly how this shipped the first time (one half-cut "08" and nothing else).
    private func ruler(span: Range<Int>, hourHeight: CGFloat, gridHeight: CGFloat) -> some View {
        let stride = invitationPreviewStride(hourHeight: hourHeight)
        return ZStack(alignment: .topTrailing) {
            // A transparent spacer gives the stack the grid's height on its own, so the frame below
            // is a constraint rather than the only thing holding it open.
            Color.clear
            ForEach(Array(span), id: \.self) { hour in
                if (hour - span.lowerBound) % stride == 0 {
                    Text(hourLabel(hour, use24Hour: use24Hour))
                        .font(.system(size: 9))
                        .foregroundStyle(.secondary)
                        .padding(.trailing, 4)
                        // A label straddles its own gridline, as the full grid's ruler does, except
                        // the first, which has no line above it and would hang off the top.
                        .offset(y: max(hourHeight * CGFloat(hour - span.lowerBound) - 4, 0))
                }
            }
        }
        .frame(width: gutter, height: gridHeight, alignment: .topTrailing)
        .clipped()
    }

    private func grid(
        span: Range<Int>,
        hourHeight: CGFloat,
        gridHeight: CGFloat,
        dayWidth: CGFloat
    ) -> some View {
        ZStack(alignment: .topLeading) {
            CalendarGridLines(dayCount: 1, dayWidth: dayWidth, hourHeight: hourHeight)
            ForEach(preview.timed, id: \.rowID) { segment in
                CalendarTimedBlock(
                    segment: segment,
                    calendars: [],
                    dayWidth: dayWidth,
                    hourHeight: hourHeight,
                    use24Hour: use24Hour,
                    dark: dark,
                    // Title only, as the Android and Windows previews already draw, see
                    // `CalendarTimedBlock.showsTime`.
                    showsTime: false,
                    onOpen: {}
                )
            }
        }
        // The blocks position themselves from midnight, as they do on the real grid; the whole day is
        // laid out and slid up so the span starts at the top. Same multiplication, no second solver.
        .frame(width: dayWidth, height: hourHeight * CGFloat(calendarHours), alignment: .topLeading)
        .offset(y: -hourHeight * CGFloat(span.lowerBound))
        .frame(height: gridHeight, alignment: .topLeading)
        .clipped()
    }
}

/// The meeting's UTC instants as wall-clock minutes from midnight in `zone`.
///
/// Returns a one-hour span at midnight for an instant that will not parse, the preview then draws the
/// day it was given rather than nothing at all, which is the same best-effort posture the core takes
/// when it cannot resolve a conflict window.
func meetingMinuteSpan(startsAt: String, endsAt: String, zone: String) -> MinuteSpan {
    var calendar = Calendar(identifier: .gregorian)
    calendar.timeZone = TimeZone(identifier: zone) ?? .current
    guard let start = parseUtcInstant(startsAt) else { return MinuteSpan(start: 0, end: 60) }
    let end = parseUtcInstant(endsAt) ?? start
    let from = calendar.dateComponents([.hour, .minute], from: start)
    let to = calendar.dateComponents([.hour, .minute], from: end)
    let startMinutes = (from.hour ?? 0) * 60 + (from.minute ?? 0)
    // An end past midnight, or on a later day, belongs to the end of this day's grid.
    let sameDay = calendar.isDate(start, inSameDayAs: end)
    let endMinutes = sameDay ? (to.hour ?? 0) * 60 + (to.minute ?? 0) : 24 * 60
    return MinuteSpan(start: startMinutes, end: max(endMinutes, startMinutes + 1))
}
