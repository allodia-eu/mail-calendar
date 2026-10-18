// Answering a meeting from its event in the calendar (docs/calendar.md §4, "Answering from the
// calendar"): Accept, Maybe and Decline on the invitee's own copy, the detail staying open after an
// answer, and Decline asking first.
//
// WHY TWO ACCOUNTS. Alice organises and invites Bob, both signed in to this app (stalwart-multi), so
// Bob's copy reaches his calendar here and his answer reaches Alice's mailbox, read over JMAP: an
// answer stored and told to nobody looks the same from Bob's side.
//
// Each run organises its own meeting under a title with the time in it, so the seeded invitations
// other suites read keep their answers, and removes it and the mail it caused at the end. The cases
// build on one another and run in the order written.

using Allodia.Mailcal.UiTests.Driver;
using Xunit;

namespace Allodia.Mailcal.UiTests.Suites;

[Collection(TwoAccountHarnessCollection.Name)]
[TestCaseOrderer(DeclarationOrderer.Name, DeclarationOrderer.Assembly)]
public sealed class EventAnswerTests(TwoAccountHarnessApp harness)
{
    private static readonly string Title = $"Answer {DateTime.Now:yyyyMMdd-HHmmss}";

    private void Case(string name, Action<App, CalendarFlow> body) =>
        harness.Case(name, app => body(app, new CalendarFlow(app)), app => new CalendarFlow(app).CloseDialogs());

    private static string AnswerLine(App app) => app.Agent.Find(Filter.ById("EventAnswerLine"))?.Name ?? "";

    private static Element AnswerButton(App app, string id) =>
        app.Agent.Find(Filter.ById(id, "Button")) ?? throw new InvalidOperationException($"the detail offers no #{id}");

    /// <summary>Waits until the answer line reads one of <paramref name="key"/>'s values, in whatever
    /// language the app came up in.</summary>
    private static void WaitAnswerLine(App app, string key, int timeoutSec = 45)
    {
        var values = Catalog.Everywhere(key);
        if (!App.Eventually(() => values.Contains(AnswerLine(app)), TimeSpan.FromSeconds(timeoutSec)))
        {
            throw new InvalidOperationException($"the answer line never read {key} (last: '{AnswerLine(app)}')");
        }
    }

    [HarnessFact]
    public void TheOrganiserIsOfferedNoAnswerToTheirOwnMeeting() => Case(nameof(TheOrganiserIsOfferedNoAnswerToTheirOwnMeeting), (app, flow) =>
    {
        var editor = flow.OpenNewEvent();
        flow.InviteeKeys(Harness.Bob, enter: true);
        app.Agent.SetValue(app.Agent.Find(new Filter { Id = "EventTitleBox", Under = editor })!, Title);
        app.Agent.Invoke(app.Agent.Find(new Filter { Id = "PrimaryButton", Type = "Button", Under = editor })!);
        Harness.WaitSchedulingMail(Harness.Bob, "Invitation:", Title);

        var detail = flow.OpenMeetingDetail(Title, Harness.Alice);
        Assert.True(app.Agent.Find(new Filter { Id = "EventAnswerLine", Under = detail }) is null,
            "Alice called the meeting; she attends by definition and has nothing to answer");
    });

    [HarnessFact]
    public void TheInviteesDetailOpensOnItsDefaultButtonNotOnAnAnswer() => Case(nameof(TheInviteesDetailOpensOnItsDefaultButtonNotOnAnAnswer), (app, flow) =>
    {
        flow.OpenMeetingDetail(Title, Harness.Bob);
        Assert.True(Catalog.Everywhere("invitation_response_needs_action").Contains(AnswerLine(app)), "Bob has not answered, and the detail says so");
        // A ContentDialog focuses its content's first focusable control, which is Accept: Enter would
        // then accept a meeting where it should press the default button.
        Assert.True(App.Eventually(() => app.Agent.Focused()?.Id == "PrimaryButton", TimeSpan.FromSeconds(2)),
            $"the initial focus is the default button, not #{app.Agent.Focused()?.Id}");
    });

    [HarnessFact]
    public void MaybeAnswersAtOnceAndTheDetailStaysOpenShowingIt() => Case(nameof(MaybeAnswersAtOnceAndTheDetailStaysOpenShowingIt), (app, flow) =>
    {
        flow.OpenMeetingDetail(Title, Harness.Bob);
        app.Agent.Invoke(AnswerButton(app, "EventAnswerTentative"));
        WaitAnswerLine(app, "invitation_response_tentative");
        Assert.True(app.Agent.Find(Filter.ById("CloseButton", "Button")) is not null, "the detail stays open after Maybe");
        Assert.True(App.Eventually(() => !AnswerButton(app, "EventAnswerTentative").Enabled, TimeSpan.FromSeconds(5)), "the answer given is inert");
        Assert.True(AnswerButton(app, "EventAnswerAccept").Enabled, "the others can still change it");
        Harness.WaitSchedulingMail(Harness.Alice, "Tentative:", Title);
    });

    [HarnessFact]
    public void DeclineAsksFirstAndBackingOutSendsNothing() => Case(nameof(DeclineAsksFirstAndBackingOutSendsNothing), (app, flow) =>
    {
        flow.OpenMeetingDetail(Title, Harness.Bob);
        app.Agent.Invoke(AnswerButton(app, "EventAnswerDecline"));
        var confirm = app.Agent.Wait(Filter.ById("EventDeclineConfirm"), TimeSpan.FromSeconds(10))
            ?? throw new InvalidOperationException("Decline sent at once, without asking");
        app.Agent.Invoke(app.Agent.Find(new Filter { Id = "CloseButton", Type = "Button", Under = confirm })!);
        // Backing out reopens the detail rather than leaving the user on the calendar.
        WaitAnswerLine(app, "invitation_response_tentative", timeoutSec: 10);
        // An absence needs time to be one: the server would have sent within this.
        Thread.Sleep(TimeSpan.FromSeconds(3));
        Assert.False(Harness.HasSchedulingMail(Harness.Alice, "Declined:", Title), "a cancelled decline tells nobody");
    });

    [HarnessFact]
    public void AConfirmedDeclineTellsTheOrganiserAndTakesTheMeetingOffTheCalendar() => Case(nameof(AConfirmedDeclineTellsTheOrganiserAndTakesTheMeetingOffTheCalendar), (app, flow) =>
    {
        flow.OpenMeetingDetail(Title, Harness.Bob);
        app.Agent.Invoke(AnswerButton(app, "EventAnswerDecline"));
        var confirm = app.Agent.Wait(Filter.ById("EventDeclineConfirm"), TimeSpan.FromSeconds(10))
            ?? throw new InvalidOperationException("Decline sent at once, without asking");
        app.Agent.Invoke(app.Agent.Find(new Filter { Id = "PrimaryButton", Type = "Button", Under = confirm })!);
        Assert.True(app.Agent.WaitGone(Filter.ById("CloseButton", "Button"), TimeSpan.FromSeconds(10)), "the detail closes with the meeting");
        Harness.WaitSchedulingMail(Harness.Alice, "Declined:", Title);
        var agenda = flow.ShowAgenda();
        var copies = app.Agent.FindAll(new Filter { Type = "ListItem", Under = agenda, Has = Filter.Named(Title, "Text") });
        Assert.True(copies.Count == 1, $"only Alice's copy is left; a declined meeting is hidden from Bob's calendar ({copies.Count} rows)");

        // Alice's copy goes too, so the next run starts from a clean calendar.
        flow.OpenMeetingDetail(Title, Harness.Alice);
        app.Agent.Invoke(app.Agent.Find(Filter.ById("SecondaryButton", "Button"))!);
        app.Agent.Invoke(app.Agent.Wait(new Filter { Id = "PrimaryButton", Type = "Button" }, TimeSpan.FromSeconds(10))!);
        foreach (var who in new[] { Harness.Alice, Harness.Bob })
        {
            Harness.RemoveMail(who, Title);
        }
    });
}
