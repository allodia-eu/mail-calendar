// Where the event editor's caret opens (docs/calendar.md §11): a new event starts in its title, an
// edit does not. And where the event detail's focus opens: on its default button, even when the
// notes hold a link.
//
// WHY HERE. EventEditorState, linked into Mailcal.Tests, knows whether it is editing and nothing
// else; who takes focus is the ContentDialog's, which that assembly cannot link.
//
// WHY THE EDIT HALF IS NOT REDUNDANT. A ContentDialog whose content holds a focusable control
// focuses the first one by itself, and the first one here is the title, so the edit half cannot be
// met by withholding the request: a client that did not ask would look right in code and be wrong
// on screen.
//
// WHY THE HARNESS. The edit half needs an event that exists. Nothing here writes: the editor is
// opened and cancelled.

using Allodia.Mailcal.UiTests.Driver;
using Xunit;

namespace Allodia.Mailcal.UiTests.Suites;

[Collection(HarnessCollection.Name)]
public sealed class EventEditorFocusTests(HarnessApp harness)
{
    // Seeded with notes holding an address (seed-calendar-week.sh), on alice's writable calendar.
    private const string WithLink = "Customer call";

    private void Case(string name, Action<App, CalendarFlow> body) =>
        harness.Case(name, app => body(app, new CalendarFlow(app)), app => app.CloseDialogs());

    private static Element TitleBox(App app, Element dialog) =>
        app.Agent.Find(new Filter { Id = "EventTitleBox", Under = dialog })
        ?? throw new InvalidOperationException("the dialog on screen is not the editor: it has no #EventTitleBox");

    [HarnessFact]
    public void ANewEventOpensWithTheCaretInItsTitle() => Case(nameof(ANewEventOpensWithTheCaretInItsTitle), (app, flow) =>
    {
        app.CloseDialogs();
        flow.ShowCalendar();
        var add = app.Agent.Find(Filter.ById("CalendarNewEvent", "Button")) ?? throw new InvalidOperationException("no New event button on the calendar");
        app.Agent.Invoke(add);
        var box = TitleBox(app, app.Dialog(Filter.ById("EventTitleBox")));
        // The request is made on Opened, raised once the dialog has finished presenting.
        Assert.True(App.Eventually(() => app.Agent.Read(box).Focused, TimeSpan.FromSeconds(10)),
            "a new event opened without the caret in its title, the one field it cannot be saved without (docs/calendar.md §11)");
    });

    [HarnessFact]
    public void EditingAnExistingEventDoesNotPutTheCaretInItsTitle() => Case(nameof(EditingAnExistingEventDoesNotPutTheCaretInItsTitle), (app, flow) =>
    {
        var box = TitleBox(app, flow.OpenEditor(WithLink));
        // The focus this forbids is placed by the dialog as it presents, so a read straight away can
        // catch the frame before it and pass for the wrong reason.
        app.Agent.Settle(quiet: TimeSpan.FromMilliseconds(300), floor: TimeSpan.FromMilliseconds(1200), cap: TimeSpan.FromSeconds(3));
        Assert.False(app.Agent.Read(box).Focused,
            "editing an event opened with the caret in its title: it has one already, and on a touch host the keyboard covers the dates the editor was opened to change (docs/calendar.md §11)");
    });

    [HarnessFact]
    public void TheEventDetailOpensOnItsDefaultButtonNotOnALinkInTheNotes() => Case(nameof(TheEventDetailOpensOnItsDefaultButtonNotOnALinkInTheNotes), (app, flow) =>
    {
        // The notes are sender text, and a ContentDialog focuses the first focusable control in its
        // content, which is that link: Enter would stop pressing Edit.
        var detail = flow.OpenDetail(WithLink);
        Assert.True(app.Agent.Find(new Filter { Type = "Hyperlink", Under = detail }) is not null,
            $"the notes drew no link, so this case cannot tell the default button from one; reseed the harness if '{WithLink}' has no notes");
        var edit = app.Agent.Find(new Filter { Id = "PrimaryButton", Type = "Button", Under = detail })
            ?? throw new InvalidOperationException($"'{WithLink}' offered no Edit; the harness calendar should be writable");
        var focused = App.Eventually(() => app.Agent.Read(edit).Focused, TimeSpan.FromSeconds(5));
        var holder = app.Agent.Focused();
        Assert.True(focused,
            $"the event detail opened with focus on {holder?.Type} '{holder?.Name}' rather than Edit, so Enter no longer edits the event, and on a link it opens an address the sender chose");
    });
}
