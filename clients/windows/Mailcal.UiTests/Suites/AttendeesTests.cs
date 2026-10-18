// An event's detail, and its editor, list who is coming (docs/calendar.md §4, "The attendee list").
//
// WHY THE HARNESS. Stalwart decodes ORGANIZER into a participant carrying CHAIR, with no owner role
// anywhere on the event, so a projection asking only for owner marks nobody and the feature does
// nothing on a whole class of account. No hand-built record catches that; reverting the chair
// fallback turns "only the organiser is marked as the organiser" red.
//
// What it does not prove: the core's merge of a split ORGANIZER and matching ATTENDEE pair into one
// row. JSCalendar merges the pair server-side, so this account hands the projection one participant.
// The core's unit tests cover the merge; the case here guards the rendering, one row per person.
//
// Almost nothing below matches a localised string. Names and addresses come from
// seed-calendar-week.sh, the heading is found by its AutomationId, and the answer labels are
// compared with each other: two attendees seeded ACCEPTED read alike, the NEEDS-ACTION one does not.
//
// The seeded meeting, Monday 10:30: Bob Tester organises (with a matching ATTENDEE, CHAIR, ACCEPTED),
// Alice Tester is REQ-PARTICIPANT and NEEDS-ACTION, Carol External is REQ-PARTICIPANT and ACCEPTED.

using Allodia.Mailcal.UiTests.Driver;
using Xunit;

namespace Allodia.Mailcal.UiTests.Suites;

[Collection(HarnessCollection.Name)]
public sealed class AttendeesTests(HarnessApp harness)
{
    private const string Meeting = "Quarterly planning";
    private const string Plain = "Lunch"; // the same day, with no ATTENDEE line at all
    private const string Organiser = "Bob Tester";
    private const string OrganiserMail = "bob@test.local";
    private const string Unanswered = "Alice Tester";
    private const string UnansweredMail = "alice@test.local";
    private const string Accepted = "Carol External";
    private const string AcceptedMail = "carol@example.com";

    private void Case(string name, Action<App, CalendarFlow> body) =>
        harness.Case(name, app => body(app, new CalendarFlow(app)), app => app.CloseDialogs());

    /// <summary>Every text in the dialog, with the y it is drawn at, or null for one scrolled out of
    /// view, which reports no size.</summary>
    private static List<(string Text, double? Top)> Texts(App app, Element dialog) =>
        app.Agent.FindAll(new Filter { Type = "Text", Under = dialog })
            .Where(t => t.Name.Length > 0)
            .Select(t => (t.Name, t.Rendered ? t.Y : (double?)null))
            .ToList();

    /// <summary>The label beside an attendee for how they answered: a row is two columns, so it is
    /// the text drawn at the same y as the name. Read by place because the label is localised.</summary>
    private static string AnswerFor(List<(string Text, double? Top)> texts, string name)
    {
        var row = texts.FirstOrDefault(t => t.Text == name && t.Top is not null);
        if (row.Text is null)
        {
            throw new InvalidOperationException($"'{name}' is not drawn, so there is no answer to read beside them");
        }
        var answer = texts.FirstOrDefault(t => t.Top is { } top && Math.Abs(top - row.Top!.Value) < 2 && t.Text != name);
        return answer.Text ?? throw new InvalidOperationException($"'{name}' has no label beside them; an attendee row says how they answered");
    }

    private static Element? Heading(App app, Element dialog) =>
        app.Agent.Find(new Filter { Id = "EventAttendeesHeading", Type = "Text", Under = dialog });

    [HarnessFact]
    public void AMeetingListsEveryoneOnItOrganiserFirst() => Case(nameof(AMeetingListsEveryoneOnItOrganiserFirst), (app, flow) =>
    {
        var texts = Texts(app, flow.OpenDetail(Meeting));
        var ys = new Dictionary<string, double>();
        foreach (var who in new[] { Organiser, Unanswered, Accepted })
        {
            Assert.True(texts.Any(t => t.Text == who), $"the roster names everyone the server put on the meeting; '{who}' is missing");
            ys[who] = texts.First(t => t.Text == who).Top
                ?? throw new InvalidOperationException($"'{who}' is in the tree but not drawn: the detail clips its own roster");
        }
        // The organiser is who a reader looks for first; everyone else keeps the event's order.
        Assert.True(ys[Organiser] < ys[Unanswered], "the organiser sorts above every other attendee");
        Assert.True(ys[Unanswered] < ys[Accepted], "after the organiser, attendees keep the order the event gave them");
    });

    [HarnessFact]
    public void TheOrganiserAppearsOnceNotOncePerLineTheServerSent() => Case(nameof(TheOrganiserAppearsOnceNotOncePerLineTheServerSent), (app, flow) =>
    {
        // One row longer than the "of N" tally beside it is a discrepancy nothing on screen explains.
        var texts = Texts(app, flow.OpenDetail(Meeting));
        Assert.True(texts.Count(t => t.Text == Organiser) == 1, "an ORGANIZER line and its matching ATTENDEE line are one person, so one row");
        Assert.True(texts.Count(t => t.Text.Contains(OrganiserMail, StringComparison.Ordinal)) == 1, "one row is also one address line");
    });

    [HarnessFact]
    public void TheTwoWhoAcceptedReadAlikeAndTheOneWhoHasNotReadsDifferently() => Case(nameof(TheTwoWhoAcceptedReadAlikeAndTheOneWhoHasNotReadsDifferently), (app, flow) =>
    {
        // A projection that flattened every status fails the second half; one that let the organiser
        // inference answer for somebody fails the first.
        var texts = Texts(app, flow.OpenDetail(Meeting));
        var accepted = AnswerFor(texts, Accepted);
        Assert.Equal(accepted, AnswerFor(texts, Organiser));
        Assert.True(accepted != AnswerFor(texts, Unanswered), $"an attendee who has not replied may not read as one who accepted (both say '{accepted}')");
    });

    [HarnessFact]
    public void OnlyTheOrganiserIsMarkedAsTheOrganiser() => Case(nameof(OnlyTheOrganiserIsMarkedAsTheOrganiser), (app, flow) =>
    {
        // Under a name comes the address, then the organiser's label: the organiser's line is longer
        // than the bare address, and an ordinary attendee's is exactly theirs. The shape, not the word.
        var texts = Texts(app, flow.OpenDetail(Meeting)).Select(t => t.Text).ToList();
        var organiserLine = texts.FirstOrDefault(t => t.StartsWith(OrganiserMail, StringComparison.Ordinal))
            ?? throw new InvalidOperationException("the organiser's row shows no address line");
        Assert.True(organiserLine.Length > OrganiserMail.Length, "the organiser's line says more than the bare address: that is where the label goes");
        Assert.Contains(UnansweredMail, texts);
    });

    [HarnessFact]
    public void AnEventNobodyWasInvitedToShowsNoAttendeeHeadingAtAll() => Case(nameof(AnEventNobodyWasInvitedToShowsNoAttendeeHeadingAtAll), (app, flow) =>
    {
        // Not an empty list: a heading with nothing under it reads as "we looked and found none".
        var dialog = flow.OpenDetail(Plain);
        var texts = Texts(app, dialog).Select(t => t.Text).ToList();
        foreach (var who in new[] { Organiser, Accepted, AcceptedMail })
        {
            Assert.DoesNotContain(who, texts);
        }
        Assert.True(Heading(app, dialog) is null, "no attendees means no heading");
    });

    [HarnessFact]
    public void TheHeadingIsThereWhenThereAreAttendees() => Case(nameof(TheHeadingIsThereWhenThereAreAttendees), (app, flow) =>
    {
        // The guard for the case above: a heading that stopped rendering would pass it.
        Assert.True(Heading(app, flow.OpenDetail(Meeting)) is not null, "the meeting draws its roster with no heading over it");
    });

    [HarnessFact]
    public void TheEditorCarriesTheSameListAndSaysItCannotBeChangedThere() => Case(nameof(TheEditorCarriesTheSameListAndSaysItCannotBeChangedThere), (app, flow) =>
    {
        // An edit made blind to who is coming is what this prevents. The form is taller than its
        // scroller, so the roster is below the fold, and the scroll tells reachable from clipped.
        var dialog = flow.OpenEditor(Meeting);
        var heading = flow.RevealAtEnd(dialog, Filter.ById("EventAttendeesHeading").Id!,
            "the editor shows no attendee list it can scroll to, so an edit would be made blind to who is on the meeting");
        var texts = Texts(app, dialog);
        foreach (var who in new[] { Organiser, Unanswered, Accepted })
        {
            Assert.True(texts.Any(t => t.Text == who), $"the editor shows the same roster as the detail; '{who}' is missing");
        }
        Assert.True(texts.Count(t => t.Text == Organiser) == 1, "one row per person in the editor too");
        // The sentence that keeps a read-only surface honest, found by elimination rather than by
        // its wording: the line below the heading that is no attendee's name or address.
        var roster = new[] { Organiser, Unanswered, Accepted, UnansweredMail, AcceptedMail };
        var note = texts.FirstOrDefault(t => t.Top > heading.Y && !roster.Contains(t.Text)
            && !t.Text.Contains(OrganiserMail, StringComparison.Ordinal) && t.Text.Contains(' ', StringComparison.Ordinal));
        Assert.True(note.Text is not null, "the editor lists attendees and says nothing of their being read-only, which invites an edit it will drop");
    });
}
