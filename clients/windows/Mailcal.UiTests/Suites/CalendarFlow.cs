// The calendar moves the event suites share: the calendar on screen, its agenda list, an event's
// detail and editor dialogs.

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
    /// Puts the calendar into the shape the view menu's item <paramref name="item"/> names, and brings
    /// the page back to today. The layout is the core's and the calendar reopens the way it was
    /// left, so a case pins the shape rather than inheriting it; and choosing a shape re-seats the
    /// grid on the period it was already showing, which may be one a case before this paged to.
    /// </summary>
    internal void SetShape(Filter item)
    {
        ShowCalendar();
        Agent.Invoke(Agent.Find(Filter.ById("CalendarViewMenu", "Button"))!);
        Agent.Invoke(Agent.Wait(item, TimeSpan.FromSeconds(10)) ?? throw new InvalidOperationException($"the view menu offers no #{item.Id}"));
        Agent.WaitGone(item, TimeSpan.FromSeconds(5));
        if (Agent.Find(Filter.ById("CalendarToday", "Button")) is { } today)
        {
            Agent.Invoke(today);
        }
        Agent.Settle(quiet: TimeSpan.FromMilliseconds(300));
    }

    /// <summary>
    /// The agenda list. The agenda rather than the grid because the grid is a drawn canvas whose
    /// events have no Invoke pattern, while the agenda is a list whose item click opens the same
    /// detail. Chosen from the view menu by the item's own id, which holds in any language.
    /// </summary>
    internal Element ShowAgenda()
    {
        ShowCalendar();
        if (Agent.Find(Filter.ById("CalendarAgenda")) is { } existing)
        {
            return existing;
        }
        Agent.Invoke(Agent.Find(Filter.ById("CalendarViewMenu", "Button"))!);
        var item = Agent.Wait(Filter.ById("CalendarViewAgenda", "MenuItem"), TimeSpan.FromSeconds(10))
            ?? throw new InvalidOperationException("the view menu offers no agenda");
        Agent.Invoke(item);
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
        app.CloseDialogs();
        var agenda = ShowAgenda();
        // The agenda fills in after it is drawn.
        if (Agent.Wait(new Filter { Type = "ListItem", Under = agenda }, TimeSpan.FromSeconds(10)) is null)
        {
            throw new InvalidOperationException("the agenda is empty: the harness calendar never synced");
        }
        var row = Agent.Wait(new Filter { Type = "ListItem", Under = agenda, Has = Filter.Named(title, "Text") }, TimeSpan.FromSeconds(10))
            ?? throw new InvalidOperationException(
                $"no agenda row for '{title}': the living week is seeded relative to today, so a stale harness is the usual cause (scripts/dev/harness.sh reset)");
        Agent.Invoke(row);
        return app.Dialog(Filter.ById("CloseButton", "Button"));
    }

    /// <summary>Opens the editor over the event's detail and returns the editor's dialog: the one
    /// holding the title box, which the detail does not have.</summary>
    internal Element OpenEditor(string title)
    {
        OpenDetail(title);
        var edit = Agent.Find(Filter.ById("PrimaryButton", "Button"))
            ?? throw new InvalidOperationException($"'{title}' offered no Edit button; the harness calendar should be writable");
        Agent.Invoke(edit);
        return app.Dialog(Filter.ById("EventTitleBox"));
    }

    /// <summary>
    /// Opens the detail of the meeting <paramref name="title"/> as it is on
    /// <paramref name="account"/>'s calendar. With two accounts in one app, a meeting between them
    /// is two agenda rows with one title; the detail names its calendar by the account's address.
    /// The invitee's copy arrives on their next calendar sync, so a missing one asks for it.
    /// </summary>
    internal Element OpenMeetingDetail(string title, string account)
    {
        for (var attempt = 0; attempt < 3; attempt++)
        {
            app.CloseDialogs();
            var agenda = ShowAgenda();
            var rows = Agent.FindAll(new Filter { Type = "ListItem", Under = agenda, Has = Filter.Named(title, "Text") });
            foreach (var row in rows)
            {
                Agent.Invoke(row);
                var dialog = app.Dialog(Filter.ById("CloseButton", "Button"));
                if (TextsUnder(dialog).Any(text => text.Contains($"({account})", StringComparison.Ordinal)))
                {
                    return dialog;
                }
                app.CloseDialogs();
            }
            Agent.Invoke(Agent.Find(Filter.ById("CalendarRefresh", "Button")) ?? throw new InvalidOperationException("the calendar offers no Sync button"));
            App.Eventually(
                () => Agent.FindAll(new Filter { Type = "ListItem", Under = agenda, Has = Filter.Named(title, "Text") }).Count > rows.Count,
                TimeSpan.FromSeconds(20));
        }
        throw new InvalidOperationException($"no copy of '{title}' in {account}'s calendar");
    }

    /// <summary>Opens the new-event editor, scrolled to its invitee field, and returns it.</summary>
    internal Element OpenNewEvent()
    {
        app.CloseDialogs();
        ShowAgenda();
        Agent.Invoke(Agent.Find(Filter.ById("CalendarNewEvent", "Button")) ?? throw new InvalidOperationException("the calendar offers no New event"));
        var editor = app.Dialog(Filter.ById("EventTitleBox"));
        RevealAtEnd(editor, "EventInviteeField",
            $"the editor offers no invitee field on {Harness.Alice}'s calendar, whose server sends invitations");
        return editor;
    }

    /// <summary>
    /// Scrolls <paramref name="dialog"/>'s form to its end until <paramref name="id"/> is drawn there,
    /// and returns it. The invitee section is added once the editor knows the calendar's server sends
    /// invitations, after the dialog appears, so a single scroll can stop short of it; and an element
    /// out of view reports no size at all, so nothing but the scroll can tell whether it exists.
    /// </summary>
    internal Element RevealAtEnd(Element dialog, string id, string missing)
    {
        Element? element = null;
        var revealed = App.Eventually(() =>
        {
            Agent.ScrollToEnd(dialog);
            Agent.Settle(quiet: TimeSpan.FromMilliseconds(200), cap: TimeSpan.FromSeconds(2));
            return (element = Agent.Find(new Filter { Id = id, Under = dialog })) is { Rendered: true };
        }, TimeSpan.FromSeconds(10));
        return revealed ? element! : throw new InvalidOperationException(missing);
    }

    /// <summary>
    /// Types into the invitee field with real keystrokes, so its own change handling runs. With
    /// <paramref name="enter"/>, presses Enter and returns once it has been answered: the address
    /// put on the list (the field empties) or refused (a refusal is drawn).
    /// </summary>
    internal void InviteeKeys(string text, bool enter = false)
    {
        Element? field = null;
        if (!App.Eventually(() => (field = Agent.Find(Filter.ById("EventInviteeField"))) is { Rendered: true }, TimeSpan.FromSeconds(5)))
        {
            throw new InvalidOperationException("the invitee field is not on screen to type into");
        }
        app.TypeInto(field!, text);
        if (enter)
        {
            // Typing a known address opens the suggestions a moment later, and an Enter that lands
            // while they open goes to them.
            Agent.Settle(quiet: TimeSpan.FromMilliseconds(300), floor: TimeSpan.FromMilliseconds(300));
            Input.Press(Input.Enter);
            var answered = App.Eventually(
                () => Agent.Read(field!).Value is null or "" || Agent.Find(Filter.ById("EventInviteeRefusal")) is { Name.Length: > 0 },
                TimeSpan.FromSeconds(5));
            if (!answered)
            {
                throw new InvalidOperationException($"Enter after '{text}' neither added it nor said why not");
            }
        }
        Agent.Settle(quiet: TimeSpan.FromMilliseconds(250));
    }

    /// <summary>What the invitee field holds now.</summary>
    internal string InviteeFieldValue() =>
        Agent.Read(Agent.Find(Filter.ById("EventInviteeField")) ?? throw new InvalidOperationException("no invitee field")).Value ?? "";

    /// <summary>The texts of every Text element under <paramref name="root"/>.</summary>
    internal IReadOnlyList<string> TextsUnder(Element root) =>
        Agent.FindAll(new Filter { Type = "Text", Under = root }).Select(t => t.Name).Where(n => n.Length > 0).ToList();
}
