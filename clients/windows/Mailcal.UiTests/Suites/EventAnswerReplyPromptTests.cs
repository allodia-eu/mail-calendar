// "The organiser wasn't told" when the answer came from the calendar: the question is put inside the
// open event detail, because the window's own bar sits under the dialog's overlay where nobody can
// reach it (docs/calendar.md, "Answering from the calendar"; docs/invitations.md).
//
// The verdict is faked in the core, as in InvitationReplyPromptTests, on the same CalDAV half, the
// only route that asks. Its own invitation: the seeded ones belong to other suites, so the suite
// writes a meeting Bob organises straight into Alice's calendar, as seed-calendar-week.sh seeds one,
// and removes it, and the mail it caused, at the end. The cases run in the order written.

using Allodia.Mailcal.UiTests.Driver;
using Xunit;

namespace Allodia.Mailcal.UiTests.Suites;

[Collection(FakeReplyHarnessCollection.Name)]
[TestCaseOrderer(DeclarationOrderer.Name, DeclarationOrderer.Assembly)]
public sealed class EventAnswerReplyPromptTests(FakeReplyHarnessApp harness)
{
    private static readonly string Stamp = DateTime.Now.ToString("yyyyMMdd-HHmmss", System.Globalization.CultureInfo.InvariantCulture);
    private static readonly string Title = $"Calendar answer {Stamp}";
    private static readonly string Uid = $"windows-answer-{Stamp}";
    private const string Organiser = "bob@test.local";

    private static readonly Filter Prompt = Filter.ById("EventReplyPrompt") with { OnScreen = true };

    private static void PutMeeting()
    {
        var day = DateTime.Now.AddDays(1).ToString("yyyyMMdd", System.Globalization.CultureInfo.InvariantCulture);
        Harness.PutEvent(Harness.Alice, Uid, string.Join("\r\n",
            "BEGIN:VCALENDAR", "VERSION:2.0", "PRODID:-//Allodia//Windows UI suite//EN", "BEGIN:VEVENT",
            $"UID:{Uid}@test.local", "DTSTAMP:20260101T000000Z",
            $"DTSTART;TZID=Europe/Amsterdam:{day}T140000", $"DTEND;TZID=Europe/Amsterdam:{day}T150000",
            $"SUMMARY:{Title}",
            $"ORGANIZER;CN=Bob Tester:mailto:{Organiser}",
            $"ATTENDEE;CN=Bob Tester;ROLE=CHAIR;PARTSTAT=ACCEPTED;RSVP=FALSE:mailto:{Organiser}",
            "ATTENDEE;CN=Alice Tester;ROLE=REQ-PARTICIPANT;PARTSTAT=NEEDS-ACTION;RSVP=TRUE:mailto:alice@test.local",
            "END:VEVENT", "END:VCALENDAR", ""));
    }

    [HarnessFact]
    public void AReportedFailurePutsTheQuestionInsideTheOpenDetailNamingTheOrganiser() =>
        harness.Case(nameof(AReportedFailurePutsTheQuestionInsideTheOpenDetailNamingTheOrganiser), app =>
        {
            ReplyPromptFlow.ForgetRemembered();
            PutMeeting();
            var detail = new CalendarFlow(app).OpenMeetingDetail(Title, Harness.Alice);
            app.Agent.Invoke(app.Agent.Find(new Filter { Id = "EventAnswerAccept", Type = "Button", Under = detail })
                ?? throw new InvalidOperationException("the detail offers no Accept"));
            var bar = app.Agent.Wait(Prompt with { Under = detail }, TimeSpan.FromSeconds(60))
                ?? throw new InvalidOperationException("the server reported the answer undelivered and the open detail asked nothing");
            Assert.True(ReplyPromptFlow.ShowsBody(ReplyPromptFlow.Texts(app, bar), Title, Organiser),
                $"the question names the meeting and who would be emailed; it shows: {string.Join(" | ", ReplyPromptFlow.Texts(app, bar))}");
        });

    [HarnessFact]
    public void AnsweringItFromTheDetailClosesIt() => harness.Case(nameof(AnsweringItFromTheDetailClosesIt), app =>
    {
        var bar = app.Agent.Find(Prompt) ?? throw new InvalidOperationException("the question is not on screen to answer; the case before this one did not raise it");
        app.Agent.Invoke(ReplyPromptFlow.Button(app, bar, "invitation_reply_undelivered_dismiss")
            ?? throw new InvalidOperationException("the question offers no way to decline sending"));
        Assert.True(app.Agent.WaitGone(Prompt, TimeSpan.FromSeconds(30)), "the core clears the question as it is answered, and the detail follows it");
        // The answer itself was stored; only telling the organiser failed.
        Assert.Contains(app.Agent.Find(Filter.ById("EventAnswerLine"))?.Name ?? "", Catalog.Everywhere("invitation_response_accepted"));
    }, app =>
    {
        app.CloseDialogs();
        ReplyPromptFlow.ForgetRemembered();
        Harness.DeleteEvent(Harness.Alice, Uid);
        foreach (var who in new[] { Harness.Alice, Organiser })
        {
            Harness.RemoveMail(who, Title);
        }
    }, reset: false);
}
