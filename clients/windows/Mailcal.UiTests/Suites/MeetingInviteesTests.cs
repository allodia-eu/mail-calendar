// Inviting people to a meeting (docs/calendar.md §4, "Organising a meeting"): the invitee field and
// its refusals, required and optional invitees, removing one, and deleting the meeting, each
// asserted on the mail the server sends rather than on the screen alone.
//
// WHY TWO ACCOUNTS. Alice organises; Bob and Dave are invited, and the invitations, cancellations
// and the optional mark are read from their mailboxes over JMAP. The cases build on one meeting and
// run in the order written; the last removes it and the mail it caused.

using Allodia.Mailcal.UiTests.Driver;
using Xunit;

namespace Allodia.Mailcal.UiTests.Suites;

[Collection(TwoAccountHarnessCollection.Name)]
[TestCaseOrderer(DeclarationOrderer.Name, DeclarationOrderer.Assembly)]
public sealed class MeetingInviteesTests(TwoAccountHarnessApp harness)
{
    private static readonly string Title = $"Invitees {DateTime.Now:yyyyMMdd-HHmmss}";

    private void Case(string name, Action<App, CalendarFlow> body) =>
        harness.Case(name, app => body(app, new CalendarFlow(app)), app => new CalendarFlow(app).CloseDialogs());

    private static Element? Under(App app, Element dialog, string id) => app.Agent.Find(new Filter { Id = id, Under = dialog });

    [HarnessFact]
    public void TheOrganiserCannotInviteThemselves() => Case(nameof(TheOrganiserCannotInviteThemselves), (app, flow) =>
    {
        var dialog = flow.OpenNewEvent();
        Assert.True(Under(app, dialog, "EventAttendeesHeading") is null,
            "a new event with nobody on it draws no attendee heading; one over nothing says we looked and found none");
        flow.InviteeKeys(Harness.Alice, enter: true);
        var refusal = Under(app, dialog, "EventInviteeRefusal")?.Name ?? "";
        Assert.True(Catalog.Everywhere("event_invitee_organiser").Contains(refusal),
            $"typing the organiser's own address says why it cannot be added (got '{refusal}')");
        Assert.True(flow.InviteeFieldValue() == Harness.Alice, "a refused address stays in the field to be corrected");
        Assert.True(Under(app, dialog, $"EventInviteeRemove:{Harness.Alice}") is null, "and is not on the list");
    });

    [HarnessFact]
    public void ANewMeetingInvitesThePeopleOnItsListAsRequiredOrOptional() => Case(nameof(ANewMeetingInvitesThePeopleOnItsListAsRequiredOrOptional), (app, flow) =>
    {
        var dialog = flow.OpenNewEvent();
        // Setting the title scrolls the form back to it a moment later, so the reveal waits for that.
        app.Agent.SetValue(Under(app, dialog, "EventTitleBox")!, Title);
        app.Agent.Settle(quiet: TimeSpan.FromMilliseconds(300));
        flow.RevealAtEnd(dialog, "EventInviteeField", "the editor lost its invitee field");
        flow.InviteeKeys("bo");
        var list = app.Agent.Wait(Filter.ById("RecipientSuggestions"), TimeSpan.FromSeconds(10))
            ?? throw new InvalidOperationException("typing a known name offers no suggestions");
        var suggestion = app.Agent.Find(new Filter { Type = "ListItem", Under = list, NameLike = $"*{Harness.Bob}*" })
            ?? throw new InvalidOperationException($"the suggestions do not offer {Harness.Bob}");
        Input.Click(app.Steady(suggestion));
        // Keys typed while the pick is still being committed land in the closing list, not the field.
        Assert.True(app.Agent.Wait(new Filter { Id = $"EventInviteeRemove:{Harness.Bob}", Under = dialog }, TimeSpan.FromSeconds(5)) is not null,
            "picking a suggestion puts it on the list");
        Assert.True(app.Agent.WaitGone(Filter.ById("RecipientSuggestions"), TimeSpan.FromSeconds(5)), "and closes the suggestions");
        app.Agent.Settle(quiet: TimeSpan.FromMilliseconds(250));
        flow.InviteeKeys(Harness.Dave, enter: true);
        Assert.True(Under(app, dialog, "PrimaryButton") is not null,
            "Enter in the invitee field adds the address; it does not press Save and close the editor");
        foreach (var who in new[] { Harness.Bob, Harness.Dave })
        {
            Assert.True(Under(app, dialog, $"EventInviteeRemove:{who}") is not null, $"{who} is on the list");
        }
        Assert.True(flow.InviteeFieldValue() == "", "an added address leaves the field");
        Assert.True(Under(app, dialog, "EventAttendeesHeading") is not null, "with people on the list, the list has its heading");

        app.Agent.Toggle(Under(app, dialog, $"EventInviteeOptional:{Harness.Dave}") ?? throw new InvalidOperationException("Dave's row offers no optional switch"));
        // Enter in an empty field reaches the default button and saves.
        app.TypeInto(Under(app, dialog, "EventInviteeField")!, "");
        Input.Press(Input.Enter);
        Assert.True(app.Agent.WaitGone(Filter.ById("EventInviteeField"), TimeSpan.FromSeconds(10)),
            "Enter in an empty invitee field must reach the default button and save");
        Harness.WaitSchedulingMail(Harness.Bob, "Invitation:", Title);
        Harness.WaitSchedulingMail(Harness.Dave, "Invitation:", Title);
    });

    [HarnessFact]
    public void TheOptionalInviteeAndOnlyTheyIsMarkedInTheAttendeeList() => Case(nameof(TheOptionalInviteeAndOnlyTheyIsMarkedInTheAttendeeList), (app, flow) =>
    {
        var texts = flow.TextsUnder(flow.OpenMeetingDetail(Title, Harness.Alice));
        Assert.True(texts.Contains(Harness.Dave), "Dave is on the roster");
        var optional = Catalog.Everywhere("event_attendee_optional");
        Assert.True(texts.Count(optional.Contains) == 1, "one optional invitee, one mark; a required one carries none");
    });

    [HarnessFact]
    public void RemovingAnInviteeCancelsTheirsAloneAndTheLastOneStays() => Case(nameof(RemovingAnInviteeCancelsTheirsAloneAndTheLastOneStays), (app, flow) =>
    {
        flow.OpenMeetingDetail(Title, Harness.Alice);
        app.Agent.Invoke(app.Agent.Find(Filter.ById("PrimaryButton", "Button"))!);
        var editor = flow.Dialog(Filter.ById("EventTitleBox"));
        app.Agent.Invoke(flow.RevealAtEnd(editor, $"EventInviteeRemove:{Harness.Dave}", "Dave is not on the list to remove"));
        Element? last = null;
        Assert.True(App.Eventually(() => (last = Under(app, editor, $"EventInviteeRemove:{Harness.Bob}")) is { Enabled: false }, TimeSpan.FromSeconds(5)),
            "a meeting keeps at least one invitee, so the last remove is inert");
        app.Agent.Invoke(Under(app, editor, "PrimaryButton")!);
        Harness.WaitSchedulingMail(Harness.Dave, "Cancelled:", Title);
        Assert.False(Harness.HasSchedulingMail(Harness.Bob, "Cancelled:", Title), "Bob is still invited, so he is sent no cancellation");
    });

    [HarnessFact]
    public void DeletingTheMeetingTellsWhoeverIsStillOnIt() => Case(nameof(DeletingTheMeetingTellsWhoeverIsStillOnIt), (app, flow) =>
    {
        flow.OpenMeetingDetail(Title, Harness.Alice);
        app.Agent.Invoke(app.Agent.Find(Filter.ById("SecondaryButton", "Button"))!);
        app.Agent.Invoke(app.Agent.Wait(new Filter { Id = "PrimaryButton", Type = "Button" }, TimeSpan.FromSeconds(10))!);
        Harness.WaitSchedulingMail(Harness.Bob, "Cancelled:", Title);
        foreach (var who in new[] { Harness.Alice, Harness.Bob, Harness.Dave })
        {
            Harness.RemoveMail(who, Title);
        }
    });
}
