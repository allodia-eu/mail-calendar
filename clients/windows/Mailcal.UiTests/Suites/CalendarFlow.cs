// The calendar moves the event suites share: the calendar on screen, its agenda list, an event's
// detail and editor dialogs, and putting the dialogs away again.

using Allodia.Mailcal.UiTests.Driver;

namespace Allodia.Mailcal.UiTests.Suites;

internal sealed class CalendarFlow(App app)
{
    private Agent Agent => app.Agent;

    /// <summary>The calendar on screen; the dataset launches on mail.</summary>
    internal void ShowCalendar()
    {
        if (Agent.Find(Filter.ById("CalendarViewMenu", "Button")) is not null)
        {
            return;
        }
        Agent.Invoke(Agent.Find(Filter.ById("NavCalendar")) ?? throw new InvalidOperationException("no Calendar entry in the navigation pane"));
        if (Agent.Wait(Filter.ById("CalendarViewMenu", "Button"), App.Patience) is null)
        {
            throw new InvalidOperationException("the calendar did not come up");
        }
    }

    /// <summary>
    /// The agenda list. The agenda rather than the grid because the grid is a drawn canvas whose
    /// events have no Invoke pattern, while the agenda is a list whose item click opens the same
    /// detail. Its entry is the view menu's last item, taken by position so it holds in any language.
    /// </summary>
    internal Element ShowAgenda()
    {
        ShowCalendar();
        if (Agent.Find(Filter.ById("CalendarAgenda")) is { } existing)
        {
            return existing;
        }
        Agent.Invoke(Agent.Find(Filter.ById("CalendarViewMenu", "Button"))!);
        IReadOnlyList<Element> items = [];
        App.Eventually(() => (items = Agent.FindAll(new Filter { Type = "MenuItem", Window = app.Mailbox.Hwnd })).Count > 0, TimeSpan.FromSeconds(10));
        if (items.Count != 6)
        {
            throw new InvalidOperationException(
                $"the view menu offers {items.Count} items where day, 3 days, work week, week, month and agenda were six: the positional pick would open something else");
        }
        Agent.Invoke(items[^1]);
        return Agent.Wait(Filter.ById("CalendarAgenda"), App.Patience) ?? throw new InvalidOperationException("the agenda list did not come up");
    }

    /// <summary>
    /// Opens the agenda event titled <paramref name="title"/> and returns its detail dialog. Matched
    /// on the text inside the row: an agenda row announces its view-model's type name rather than
    /// its event, because its template carries no automation name.
    /// </summary>
    internal Element OpenDetail(string title)
    {
        // A dialog left by a failed case would swallow the press on this one's row.
        CloseDialogs();
        var agenda = ShowAgenda();
        var rows = Agent.FindAll(new Filter { Type = "ListItem", Under = agenda });
        if (rows.Count == 0)
        {
            throw new InvalidOperationException("the agenda is empty: the harness calendar never synced");
        }
        var row = Agent.Find(new Filter { Type = "ListItem", Under = agenda, Has = Filter.Named(title, "Text") })
            ?? throw new InvalidOperationException(
                $"no agenda row for '{title}': the living week is seeded relative to today, so a stale harness is the usual cause (scripts/dev/harness.sh reset)");
        Agent.Invoke(row);
        return Dialog(Filter.ById("CloseButton", "Button"));
    }

    /// <summary>Opens the editor over the event's detail and returns the editor's dialog: the one
    /// holding the title box, which the detail does not have.</summary>
    internal Element OpenEditor(string title)
    {
        OpenDetail(title);
        var edit = Agent.Find(Filter.ById("PrimaryButton", "Button"))
            ?? throw new InvalidOperationException($"'{title}' offered no Edit button; the harness calendar should be writable");
        Agent.Invoke(edit);
        return Dialog(Filter.ById("EventTitleBox"));
    }

    /// <summary>The open dialog holding <paramref name="holding"/>: a dialog is a popup over the
    /// shell, and the shell keeps an empty popup of its own beside it.</summary>
    internal Element Dialog(Filter holding) =>
        Agent.Wait(new Filter { Type = "Window", Window = app.Mailbox.Hwnd, Has = holding }, TimeSpan.FromSeconds(15))
            ?? throw new InvalidOperationException("no dialog opened");

    /// <summary>
    /// Closes every open dialog, the editor stacked over a detail included, and waits until the
    /// last of them has finished closing. A dialog's own popup goes at once, but the layer that dims
    /// the window behind it stays until the close is complete, and the app opens no other dialog
    /// before then: a press on the next event in that time opens nothing.
    /// </summary>
    internal void CloseDialogs()
    {
        var close = Filter.ById("CloseButton", "Button");
        for (var i = 0; i < 4 && Agent.FindAll(close) is { Count: > 0 } open; i++)
        {
            Agent.Invoke(open[0]);
            App.Eventually(() => Agent.FindAll(close).Count < open.Count, TimeSpan.FromSeconds(5));
        }
        if (!App.Eventually(() => !Agent.FindAll(new Filter { Type = "Window", Window = app.Mailbox.Hwnd }).Any(w => w.Class == "Popup"), TimeSpan.FromSeconds(5)))
        {
            throw new InvalidOperationException("a dialog was still closing 5 s after it was closed");
        }
    }

    /// <summary>The texts of every Text element under <paramref name="root"/>.</summary>
    internal IReadOnlyList<string> TextsUnder(Element root) =>
        Agent.FindAll(new Filter { Type = "Text", Under = root }).Select(t => t.Name).Where(n => n.Length > 0).ToList();
}
