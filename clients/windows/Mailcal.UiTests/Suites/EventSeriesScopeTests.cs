// Does a write on ONE occurrence actually ask which occurrences it meant? (docs/calendar.md §10.)
//
// WHY HERE. `Mailcal.Tests` pins which of the two paths a tap takes and can go no further, because
// the question is a ContentDialog and that assembly links no WinUI. So a scope question decided
// correctly and never *raised* passes `dotnet test`, and the first person to find out is whoever
// deletes a standup meaning to cancel one Tuesday.
//
// WHY IT LOOKS LIKE THIS. The question is reachable only from a surface that names an occurrence,
// and the only one is the drawn grid: the agenda lists the series, so it names none (the second
// case, the control for the first). The grid is a canvas whose events are peers with no Invoke
// pattern, so a suite can read one and cannot open it; each publishes its rectangle, computed by
// the renderer's own geometry, and a real click at its middle lands on the surface's pointer owner.
// The grid seats itself near the current time, so a block is wheeled into the timed area first:
// one above the viewport has a rectangle too, with a negative y.
//
// NOTHING BELOW MATCHES A LOCALISED STRING: the handles are the seeded event's title and the number
// of answers a dialog offers, three for the scope question and two for the plain confirmation. And
// nothing writes: every case backs out, so the suite runs against the seeded harness repeatedly.

using Allodia.Mailcal.UiTests.Driver;
using Xunit;

namespace Allodia.Mailcal.UiTests.Suites;

[Collection(HarnessCollection.Name)]
public sealed class EventSeriesScopeTests(HarnessApp harness)
{
    /// <summary>The daily series of the four the harness seeds (docker/stalwart/seed-calendar-week.sh):
    /// a page is a week, but whether it is the Monday-aligned week or seven days from today depends
    /// on the pager, and a daily series running ten days from this week's Thursday is on both.</summary>
    private const string Repeating = "Onboarding";

    private void Case(string name, Action<App, CalendarFlow> body) =>
        harness.Case(name, app => body(app, new CalendarFlow(app)), app => app.CloseDialogs());

    /// <summary>The drawn block whose name begins with <paramref name="title"/>, once the grid has
    /// painted it: blocks arrive on a later frame than the surface they sit on. Null when none is.</summary>
    private static Element? GridBlock(App app, string title, int timeoutSec = 20)
    {
        Element? block = null;
        App.Eventually(() => (block = Blocks(app, $"{title}*").FirstOrDefault()) is not null, TimeSpan.FromSeconds(timeoutSec));
        return block;
    }

    private static IEnumerable<Element> Blocks(App app, string name = "*") =>
        app.Agent.FindAll(new Filter { Window = app.Mailbox.Hwnd, NameLike = name }).Where(e => e.Class == "CalendarItem");

    /// <summary>The block for <paramref name="title"/>, wheeled until its middle sits in the lower part
    /// of the grid, which clears the day headers and the all-day band in one rule.</summary>
    private static Element RevealBlock(App app, string title)
    {
        var grid = app.Agent.Find(Filter.ById("CalendarGrid")) ?? throw new InvalidOperationException("the calendar grid is not on screen");
        var floor = grid.Y + grid.Height * 0.35;
        var ceiling = grid.Y + grid.Height * 0.85;
        for (var i = 0; i < 30; i++)
        {
            var block = GridBlock(app, title) ?? throw new InvalidOperationException(
                $"no drawn block for '{title}'. The grid is showing '{app.Agent.Find(Filter.ById("CalendarPeriod"))?.Name}' and holds: {string.Join(" | ", Blocks(app).Select(b => b.Name.Split(',')[0]))}");
            var middle = block.Y + block.Height / 2;
            if (middle >= floor && middle <= ceiling)
            {
                return app.Steady(block);
            }
            Input.BringToFront(app.Mailbox.Hwnd);
            Input.Wheel((int)(grid.X + grid.Width / 2), (int)(grid.Y + grid.Height / 2), middle < floor ? 1 : -1);
            app.Agent.Settle(quiet: TimeSpan.FromMilliseconds(150), cap: TimeSpan.FromMilliseconds(600));
        }
        throw new InvalidOperationException($"the block for '{title}' would not scroll into the timed area");
    }

    /// <summary>Opens the detail of the grid block for <paramref name="title"/> with a real click, and
    /// checks it is that event's: the click is aimed by geometry.</summary>
    private static Element OpenGridEvent(App app, CalendarFlow flow, string title)
    {
        flow.SetShape(Filter.ById("CalendarViewWeek", "MenuItem"));
        Input.Click(RevealBlock(app, title));
        var dialog = app.Dialog(Filter.ById("CloseButton", "Button"));
        if (!flow.TextsUnder(dialog).Any(text => text.StartsWith(title, StringComparison.Ordinal)))
        {
            throw new InvalidOperationException($"the click at the block for '{title}' opened somebody else's detail");
        }
        return dialog;
    }

    /// <summary>Presses <paramref name="button"/> on <paramref name="dialog"/> and returns the dialog
    /// that replaces it.</summary>
    private static Element Press(App app, Element dialog, string button)
    {
        app.Agent.Invoke(app.Agent.Find(new Filter { Id = button, Type = "Button", Under = dialog })
            ?? throw new InvalidOperationException($"the dialog offers no #{button}"));
        var open = new Filter { Type = "Window", Window = app.Mailbox.Hwnd, Has = Filter.ById("CloseButton", "Button") };
        Element? next = null;
        if (!App.Eventually(() => (next = app.Agent.FindAll(open).FirstOrDefault(d => d.Handle != dialog.Handle)) is not null, TimeSpan.FromSeconds(15)))
        {
            throw new InvalidOperationException($"#{button} opened no dialog in its place");
        }
        app.Agent.Settle(quiet: TimeSpan.FromMilliseconds(300));
        return next!;
    }

    /// <summary>How many of the three answer buttons <paramref name="dialog"/> offers.</summary>
    private static int Answers(App app, Element dialog) =>
        new[] { "PrimaryButton", "SecondaryButton", "CloseButton" }
            .Count(id => app.Agent.Find(new Filter { Id = id, Type = "Button", Under = dialog, OnScreen = true }) is not null);

    [HarnessFact]
    public void DeletingOneOccurrenceFromTheGridAsksWhichOccurrencesItMeant() =>
        Case(nameof(DeletingOneOccurrenceFromTheGridAsksWhichOccurrencesItMeant), (app, flow) =>
        {
            var question = Press(app, OpenGridEvent(app, flow, Repeating), "SecondaryButton");
            Assert.True(Answers(app, question) == 3,
                "a delete opened from the grid on a repeating event must offer three answers, this event, all events, cancel. Two means the generic confirmation was raised instead, and answering it removes the whole series.");
        });

    [HarnessFact]
    public void TheSameDeleteFromTheAgendaAsksNothingAboutScope() => Case(nameof(TheSameDeleteFromTheAgendaAsksNothingAboutScope), (app, flow) =>
    {
        // The control: an agenda row lists the series, so there is no single day to name and the
        // ordinary confirmation is correct.
        flow.SetShape(Filter.ById("CalendarViewAgenda", "MenuItem"));
        var question = Press(app, flow.OpenDetail(Repeating), "SecondaryButton");
        Assert.True(Answers(app, question) == 2, "an agenda row IS the series, there is no occurrence to name, so the scope question must not be put.");
    });

    [HarnessFact]
    public void SavingAnEditToOneOccurrenceAsksWhichOccurrencesItMeant() =>
        Case(nameof(SavingAnEditToOneOccurrenceAsksWhichOccurrencesItMeant), (app, flow) =>
        {
            // The same question from the same detail, so a client that wired only the delete passes
            // every case above and fails this one.
            var editor = Press(app, OpenGridEvent(app, flow, Repeating), "PrimaryButton");
            Assert.True(app.Agent.Find(new Filter { Id = "EventTitleBox", Under = editor }) is not null, "Edit did not open the event editor");
            var question = Press(app, editor, "PrimaryButton");
            Assert.True(Answers(app, question) == 3,
                "a save from the grid on a repeating event must offer three answers, this event, all events, cancel. Two means the whole series was about to be rewritten to change one Tuesday.");
        });

    [HarnessFact]
    public void BackingOutOfTheScopeQuestionKeepsWhatWasTyped() => Case(nameof(BackingOutOfTheScopeQuestionKeepsWhatWasTyped), (app, flow) =>
    {
        // WinUI permits one ContentDialog at a time, so the editor is closed by the time the question
        // is put, and the only way back is to reopen it over the same state. The editor is
        // cancelled at the end, so nothing is written.
        var editor = Press(app, OpenGridEvent(app, flow, Repeating), "PrimaryButton");
        var typed = $"{Repeating} (edited)";
        app.Agent.SetValue(app.Agent.Find(new Filter { Id = "EventTitleBox", Under = editor }) ?? throw new InvalidOperationException("the editor has no title field"), typed);
        var question = Press(app, editor, "PrimaryButton");
        var reopened = Press(app, question, "CloseButton");
        var again = app.Agent.Find(new Filter { Id = "EventTitleBox", Under = reopened });
        Assert.True(again is not null,
            "backing out of the scope question left no editor on screen. Cancelling a question about a save must not also discard the save.");
        Assert.True(app.Agent.Read(again).Value == typed,
            "the reopened editor no longer holds what was typed. On this platform the question cannot be raised OVER the editor, so the editor is reopened, and reopening it on anything but the same state makes cancel and discard the same button.");
    });

    [HarnessFact]
    public void BackingOutOfTheScopeQuestionDeletesNothing() => Case(nameof(BackingOutOfTheScopeQuestionDeletesNothing), (app, flow) =>
    {
        var question = Press(app, OpenGridEvent(app, flow, Repeating), "SecondaryButton");
        app.Agent.Invoke(app.Agent.Find(new Filter { Id = "CloseButton", Type = "Button", Under = question })!);
        app.CloseDialogs();
        flow.SetShape(Filter.ById("CalendarViewWeek", "MenuItem"));
        Assert.True(GridBlock(app, Repeating) is not null,
            $"'{Repeating}' is no longer drawn after cancelling the scope question. Backing out of a destructive question must write nothing at all.");
    });
}
