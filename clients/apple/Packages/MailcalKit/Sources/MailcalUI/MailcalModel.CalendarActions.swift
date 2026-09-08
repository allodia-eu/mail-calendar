// The calendar surface's model calls: creating, editing, moving and deleting an event, answering
// an invitation, and the two synchronous queries an editor asks before it commits.
//
// Split from `MailcalModel.Actions.swift` to keep each file under the 500-line limit; the mail
// actions, the account settings and the MCP switches stay there.

import Foundation
import MailcalBindings

extension MailboxModel {
    /// Create a calendar event from the editor's payload, then refresh the agenda. The editor built
    /// the calendar target, the all-day flag, the device-zone wall clock, the notes, and the location.
    func createEvent(_ args: CreateArgs) {
        app?.dispatch(
            intent: .createEvent(
                title: args.title,
                start: args.start,
                end: args.end,
                account: args.account,
                calendar: args.calendar,
                allDay: args.allDay,
                timezone: args.timezone,
                notes: args.notes,
                location: args.location,
                recurrence: args.recurrence
            )
        )
    }

    /// Edit a stored calendar event from the editor's payload (a provider-neutral patch).
    func updateEvent(_ args: UpdateArgs) {
        app?.dispatch(
            intent: .updateEvent(
                account: args.account,
                key: args.key,
                title: args.title,
                start: args.start,
                end: args.end,
                notes: args.notes,
                location: args.location,
                occurrence: args.occurrence,
                recurrence: args.recurrence,
                timesFromOccurrence: args.timesFromOccurrence
            )
        )
    }

    /// Move or resize a stored event from a **drag** on the grid.
    ///
    /// Deliberately not an `updateEvent` with new times: what goes across is how far the pointer
    /// moved, in whole days and minutes, and the core applies it to the event's own wall clock. A
    /// meeting in another zone therefore cannot be re-timed by the zone the grid was drawn in:
    /// see `mailcal_account::calendar_drag`.
    func moveEvent(_ args: CalendarMoveArgs) {
        app?.dispatch(
            intent: .moveEvent(
                account: args.account,
                key: args.key,
                edge: args.edge,
                days: args.days,
                minutes: args.minutes,
                occurrence: args.occurrence
            )
        )
    }

    /// Delete a calendar event by its provider key (on its owning `account`), then
    /// refresh the agenda.
    /// Delete an event, or the single `occurrence` of it the user named.
    ///
    /// `nil` removes the whole series, which is a different request, so the caller asks first
    /// whenever the event has an occurrence to name (docs/calendar.md §10).
    func deleteEvent(_ account: String, _ key: String, occurrence: String? = nil) {
        app?.dispatch(intent: .deleteEvent(account: account, key: key, occurrence: occurrence))
    }

    /// Answer the invitation a message carries.
    ///
    /// Named by the **message**, never by the event: the answer goes out as the address the
    /// invitation matched, which on an aliased account is not the account's own identity, and only
    /// the core knows the address set (docs/invitations.md §4).
    ///
    /// `comment` and `notifyOrganizer` may only be sent when the card says the transport carries
    /// them (`canComment` / `canChooseNotify`). A transport that cannot honour one refuses the
    /// whole answer rather than dropping it, so passing a note to an account that carries none
    /// loses the answer, not just the note.
    func respondToInvitation(
        _ account: String,
        _ key: String,
        _ response: InvitationResponse,
        comment: String? = nil,
        notifyOrganizer: Bool = true,
        replySubject: String? = nil
    ) {
        app?.dispatch(
            intent: .respondToInvitation(
                account: account,
                key: key,
                response: response,
                comment: comment,
                notifyOrganizer: notifyOrganizer,
                replySubject: replySubject
            )
        )
    }

    /// Answer the "the organiser wasn't told" question: whether to email the reply ourselves after
    /// the calendar server reported it could not.
    ///
    /// Carries no handle on the meeting, the core holds the question and clears it the moment
    /// this arrives, which is what stops a double-tap emailing the organiser twice. The subject is
    /// composed here for the same reason the RSVP's is: it is copy a stranger reads in their
    /// inbox, and the core has no locale.
    func answerReplyPrompt(send: Bool, remember: Bool) {
        let subject = replyPrompt.map { invitationReplySubject($0.response, $0.summary) }
        app?.dispatch(
            intent: .answerReplyPrompt(send: send, remember: remember, replySubject: subject)
        )
    }

    /// The full detail of one stored event, or `nil` if it is not in the store, the detail sheet
    /// a tap opens, and what the editor prefills from. A local read, no network.
    ///
    /// `occurrence` is the token the tapped surface carried, passed back verbatim so the times
    /// are that occurrence's rather than the series'.
    func eventDetail(_ account: String, _ key: String, _ occurrence: String) -> EventDetail? {
        app?.eventDetail(account: account, key: key, occurrence: occurrence)
    }

    /// What saving `args` over the whole series would cost the occurrences the user changed on
    /// their own, or `nil` when there is nothing to say.
    ///
    /// Asked with the payload about to be dispatched, so the answer is about *this* edit: on a
    /// server that folds a moved occurrence back only when the series moves, a retitle costs
    /// nothing and is not warned about.
    func seriesEditWarning(_ args: UpdateArgs) -> SeriesEditWarning? {
        app?.seriesEditWarning(
            account: args.account,
            key: args.key,
            edit: ProposedEdit(
                title: args.title,
                start: args.start,
                end: args.end,
                notes: args.notes,
                location: args.location,
                // The real one: a rule change is the edit two of the four providers answer by
                // discarding every override, so the warning has to be asked knowing about it.
                recurrence: args.recurrence
            )
        )
    }
}
