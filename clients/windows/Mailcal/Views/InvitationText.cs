// The invitation card's words, one place, so the card and its respond row cannot phrase the same
// thing two ways.
//
// The *choices* are pure and unit-tested (InvitationFormat, in the Calendar layer); this maps each
// choice to an L10n string, which is a WinUI resource call and so cannot live in the test-linked
// half. Exactly the seam CalendarEventText already uses for reminders and recurrence.
using System.Globalization;
using Allodia.Mailcal.Calendar;
using uniffi.mailcal_bindings;

namespace Allodia.Mailcal.Views;

/// <summary>Localised copy for the meeting-invitation card.</summary>
internal static class InvitationText
{
    /// <summary>The card's heading: what this message is, before any detail.</summary>
    internal static string Title(InvitationKind kind) => kind switch
    {
        InvitationKind.Cancelled => L10n.InvitationCancelledTitle(),
        InvitationKind.Informational => L10n.InvitationInformationalTitle(),
        InvitationKind.Superseded => L10n.InvitationSupersededTitle(),
        _ => L10n.InvitationTitle(),
    };

    /// <summary>
    /// The sentence under the heading saying why no answer is offered, or <c>null</c> where none is
    /// owed. A superseded card still looks answerable, so without this it reads as broken rather
    /// than out of date.
    /// </summary>
    internal static string? Notice(InvitationKind kind) => kind switch
    {
        InvitationKind.Superseded => L10n.InvitationSuperseded(),
        _ => null,
    };

    /// <summary>
    /// The subject line for the reply the core emails to the organiser, on an account whose
    /// calendar server does no scheduling of its own.
    /// </summary>
    /// <remarks>
    /// Composed here, not in the core, because the core carries no locale (AGENTS.md →
    /// "Localisation is client-side") and this is copy a stranger reads in their inbox. Passing
    /// <c>null</c> is safe but silent: the core then falls back to <c>Re:</c> plus the
    /// invitation's own subject, and the organiser's list no longer says which way we answered.
    /// </remarks>
    internal static string ReplySubject(InvitationResponse response, string summary) =>
        response switch
        {
            InvitationResponse.Tentative => L10n.InvitationReplySubjectTentative(summary),
            InvitationResponse.Decline => L10n.InvitationReplySubjectDeclined(summary),
            _ => L10n.InvitationReplySubjectAccepted(summary),
        };

    /// <summary>How this account has answered so far, in words.</summary>
    internal static string Response(ResponseStatus status) => status switch
    {
        ResponseStatus.Accepted => L10n.InvitationResponseAccepted(),
        ResponseStatus.Declined => L10n.InvitationResponseDeclined(),
        ResponseStatus.Tentative => L10n.InvitationResponseTentative(),
        ResponseStatus.Delegated => L10n.InvitationResponseDelegated(),
        _ => L10n.InvitationResponseNeedsAction(),
    };

    /// <summary>What else is in the user's calendar over the meeting's window, <b>in words</b>.</summary>
    internal static string Conflicts(uint count, bool known) =>
        InvitationFormat.Conflicts(count, known) switch
        {
            ConflictLine.None => L10n.InvitationConflictsNone(),
            ConflictLine.One => L10n.InvitationConflictsOne(),
            ConflictLine.Many => L10n.InvitationConflicts((int)count),
            _ => L10n.InvitationConflictsUnknown(),
        };

    /// <summary>The attendee tally as the single line the card shows, or empty when there is none.</summary>
    internal static string Attendees(AttendeeTally tally)
    {
        var phrases = new List<string>();
        foreach (var line in InvitationFormat.Attendees(tally))
        {
            phrases.Add(line switch
            {
                AttendeeLine.OnlyYou => L10n.InvitationAttendeesOne(),
                AttendeeLine.AcceptedOfTotal => L10n.InvitationAttendees(
                    tally.Accepted.ToString(CultureInfo.InvariantCulture),
                    tally.Total.ToString(CultureInfo.InvariantCulture)),
                AttendeeLine.TentativeOne => L10n.InvitationAttendeesTentativeOne(),
                AttendeeLine.Tentative => L10n.InvitationAttendeesTentative((int)tally.Tentative),
                AttendeeLine.DeclinedOne => L10n.InvitationAttendeesDeclinedOne(),
                AttendeeLine.Declined => L10n.InvitationAttendeesDeclined((int)tally.Declined),
                AttendeeLine.PendingOne => L10n.InvitationAttendeesPendingOne(),
                _ => L10n.InvitationAttendeesPending((int)tally.NeedsAction),
            });
        }
        return string.Join(" · ", phrases);
    }

    /// <summary>What to say about an answer on its way out, or null when there is nothing to say.</summary>
    internal static string? Write(CalendarWriteStatus status) =>
        InvitationFormat.Write(status) switch
        {
            WriteLine.Sending => L10n.InvitationSending(),
            WriteLine.Failed => L10n.InvitationFailed(),
            _ => null,
        };
}
