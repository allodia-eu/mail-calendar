// An invitee's answer from the event detail, tested without a dialog: which button is the answer
// already given, that Maybe answers at once and Decline asks first, and that nothing can be pressed
// while an answer is on its way. The Windows twin of Android's EventAnswerTest and Apple's
// EventAnswerTests; the decline question itself is the UI suite's (Mailcal.UiTests, EventAnswerTests).
using System.Linq;
using Allodia.Mailcal.Calendar;
using uniffi.mailcal_bindings;
using Xunit;

namespace Allodia.Mailcal.Tests;

public class EventAnswerTests
{
    private static EventDetail Meeting(ResponseStatus? myAnswer) =>
        new(
            "acct", "/cal/m.ics", "work", "Planning", false, "Europe/Amsterdam",
            "2026-08-03T10:00:00", "2026-08-03T11:00:00", null, null, null, null, null, null,
            false, true, false, myAnswer, string.Empty, []);

    [Fact]
    public void Only_the_answer_already_given_is_current()
    {
        static InvitationResponse[] Current(ResponseStatus status) =>
            [.. EventAnswer.Choices.Where(new EventAnswer(status, Saving: false).IsCurrent)];

        Assert.Equal([InvitationResponse.Accept], Current(ResponseStatus.Accepted));
        Assert.Equal([InvitationResponse.Tentative], Current(ResponseStatus.Tentative));
        Assert.Equal([InvitationResponse.Decline], Current(ResponseStatus.Declined));
        // An unanswered meeting offers all three.
        Assert.Empty(Current(ResponseStatus.NeedsAction));
    }

    [Fact]
    public void The_detail_offers_no_answer_where_the_core_offers_none() =>
        Assert.Null(EventAnswer.Of(Meeting(myAnswer: null), CalendarWriteStatus.Idle));

    [Fact]
    public void Maybe_answers_at_once()
    {
        var answer = EventAnswer.Of(Meeting(ResponseStatus.Accepted), CalendarWriteStatus.Idle)!;
        Assert.Equal(AnswerStep.Send, answer.Press(InvitationResponse.Tentative));
        // The answer already given sends nothing again.
        Assert.Equal(AnswerStep.None, answer.Press(InvitationResponse.Accept));
    }

    [Fact]
    public void Decline_asks_first()
    {
        var answer = EventAnswer.Of(Meeting(ResponseStatus.NeedsAction), CalendarWriteStatus.Idle)!;
        Assert.Equal(AnswerStep.ConfirmDecline, answer.Press(InvitationResponse.Decline));
    }

    [Fact]
    public void Nothing_can_be_pressed_while_an_answer_is_on_its_way()
    {
        var saving = EventAnswer.Of(Meeting(ResponseStatus.Tentative), CalendarWriteStatus.Saving)!;
        Assert.All(EventAnswer.Choices, response => Assert.False(saving.CanPress(response)));
        Assert.All(EventAnswer.Choices, response => Assert.Equal(AnswerStep.None, saving.Press(response)));

        var settled = EventAnswer.Of(Meeting(ResponseStatus.Tentative), CalendarWriteStatus.Saved)!;
        Assert.True(settled.CanPress(InvitationResponse.Accept));
        Assert.False(settled.CanPress(InvitationResponse.Tentative));
    }
}
