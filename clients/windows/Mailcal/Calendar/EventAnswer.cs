// An invitee's answer in an event's detail: the decisions behind Accept, Maybe and Decline, with no
// WinUI in them, so Mailcal.Tests can drive them. The twin of Apple's EventAnswer and Android's
// EventAnswer.kt; the buttons are Dialogs/EventDetailDialog.Answer.cs.
//
// Offered only where the core sets `EventDetail.MyAnswer`: the account is invited, does not
// organise the meeting, and has a way to tell the organiser (docs/calendar.md, "Answering from the
// calendar"). Accept and Maybe go at once and the detail stays open; Decline asks first, because a
// declined meeting leaves the calendar and the detail with it.
using uniffi.mailcal_bindings;

namespace Allodia.Mailcal.Calendar;

/// <summary>What pressing an answer does.</summary>
internal enum AnswerStep
{
    /// <summary>Nothing: the button cannot be pressed.</summary>
    None,

    /// <summary>Send the answer now.</summary>
    Send,

    /// <summary>Ask before declining; nothing is sent until the user confirms.</summary>
    ConfirmDecline,
}

/// <summary>The answer row's decisions.</summary>
/// <param name="Current">What this account has answered so far.</param>
/// <param name="Saving">Whether a calendar write is on its way: an answer is, and a second would
/// race it.</param>
internal sealed record EventAnswer(ResponseStatus Current, bool Saving)
{
    /// <summary>The three answers, in the order the row draws them.</summary>
    internal static readonly InvitationResponse[] Choices =
        [InvitationResponse.Accept, InvitationResponse.Tentative, InvitationResponse.Decline];

    /// <summary>
    /// The answer an event's detail offers, or <c>null</c> where the core offers none: this account
    /// organises the meeting, it is an appointment, or nothing would reach the organiser.
    /// </summary>
    internal static EventAnswer? Of(EventDetail detail, CalendarWriteStatus status) =>
        detail.MyAnswer is { } current ? new EventAnswer(current, status == CalendarWriteStatus.Saving) : null;

    /// <summary>Whether <paramref name="response"/> is the answer already given; that button is
    /// shown chosen and inert, since sending it again changes nothing.</summary>
    internal bool IsCurrent(InvitationResponse response) => response switch
    {
        InvitationResponse.Accept => Current == ResponseStatus.Accepted,
        InvitationResponse.Tentative => Current == ResponseStatus.Tentative,
        _ => Current == ResponseStatus.Declined,
    };

    internal bool CanPress(InvitationResponse response) => !Saving && !IsCurrent(response);

    internal AnswerStep Press(InvitationResponse response)
    {
        if (!CanPress(response))
        {
            return AnswerStep.None;
        }
        return response == InvitationResponse.Decline ? AnswerStep.ConfirmDecline : AnswerStep.Send;
    }
}
