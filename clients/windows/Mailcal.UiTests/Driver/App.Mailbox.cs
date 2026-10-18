// The mailbox moves every suite shares: the rows, a row by its title, the windows beside the mailbox,
// putting the composer away, and getting back to the state a case starts from.

using System.Drawing;
using System.Runtime.InteropServices;

namespace Allodia.Mailcal.UiTests.Driver;

internal sealed partial class App
{
    internal static readonly TimeSpan Patience = TimeSpan.FromSeconds(30);

    /// <summary>The mailbox window: the one the app opened first.</summary>
    internal AppWindow Mailbox => Agent.Windows().First(w => w.Kind == "MainWindow");

    /// <summary>Every window but the mailbox.</summary>
    internal IReadOnlyList<AppWindow> ExtraWindows() => Agent.Windows().Where(w => w.Kind != "MainWindow").ToList();

    /// <summary>The message rows, and only those: a bare ListItem sweep also finds the sidebar's.</summary>
    internal IReadOnlyList<Element> Rows() =>
        Agent.FindAll(new Filter { Type = "ListItem", Under = RowsList() });

    internal Element RowsList() =>
        Agent.Find(Filter.ById("RowsList") with { Window = Mailbox.Hwnd })
        ?? throw new InvalidOperationException("the message list (#RowsList) is not on screen");

    /// <summary>The row holding a text matching <paramref name="title"/>, scrolled into view because
    /// a row below the fold can be found but not pressed; throws naming what the list holds.</summary>
    internal Element RowTitled(string title)
    {
        var filter = new Filter { Type = "ListItem", Under = RowsList(), Has = new Filter { NameLike = title } };
        var row = Agent.Find(filter)
            ?? throw new InvalidOperationException($"no message row matching '{title}'. The list holds: {string.Join(" | ", Rows().Select(r => r.Name))}");
        // The list scrolls with an animation, so the row is drawn a moment after the request.
        Agent.ScrollIntoView(row);
        Element? drawn = null;
        Eventually(() => (drawn = Agent.Find(filter)) is { Rendered: true }, TimeSpan.FromSeconds(3));
        return drawn is null ? row : Steady(drawn);
    }

    /// <summary>
    /// Opens one of alice's folders, by the row the pane draws for it under the account, and waits
    /// until the list is that folder's, with rows or its empty panel drawn: Drafts is empty whenever
    /// the suite has tidied up after itself. Until the header names the folder, the rows on screen
    /// are still the last folder's, and a press meant for the new one lands on one of those.
    /// </summary>
    internal void OpenFolder(string folder)
    {
        var account = Agent.Find(Filter.Named(Harness.Alice, "ListItem") with { Window = Mailbox.Hwnd })
            ?? throw new InvalidOperationException($"the pane has no row for {Harness.Alice}");
        // A launch lists the account before its folders arrive from the server.
        var row = Agent.Wait(Filter.Named(folder, "ListItem") with { Under = account }, Patience)
            ?? throw new InvalidOperationException($"{Harness.Alice} shows no {folder} row; is its tree open?");
        Agent.Invoke(row);
        if (Agent.Wait(Filter.ById("MailboxHeader") with { Name = folder }, TimeSpan.FromSeconds(10)) is null)
        {
            throw new InvalidOperationException($"the list never became {folder}'s");
        }
        bool Drawn() => Agent.Find(Filter.ById("EmptyMailboxText") with { OnScreen = true }) is not null
            || (Agent.Find(Filter.ById("RowsList")) is { } list && Agent.Find(new Filter { Type = "ListItem", Under = list }) is not null);
        if (!Eventually(Drawn, TimeSpan.FromSeconds(40)))
        {
            throw new InvalidOperationException($"{folder} drew neither rows nor its empty panel within 40 s");
        }
        Agent.Settle();
    }

    /// <summary>The message titled <paramref name="subject"/> open in the reading pane: its row
    /// pressed, and the pane's subject read back, which is the evidence the open finished.</summary>
    internal void OpenMessage(string subject)
    {
        Agent.Invoke(RowTitled($"*{subject}*"));
        if (!Eventually(() => Agent.Find(Filter.ById("ReadingSubject") with { Window = Mailbox.Hwnd })?.Name == subject, Patience))
        {
            throw new InvalidOperationException($"the reading pane never showed '{subject}'");
        }
    }

    /// <summary>
    /// Clicks the row titled <paramref name="title"/> with the pointer, once or as a double-click,
    /// with the mailbox in front: a press goes to whatever is under the pointer. Then waits for the
    /// windows to settle, because a double-click opens its window on the second press and the pane
    /// is put back after the trailing click.
    /// </summary>
    internal void RowClicks(string title, int times = 1)
    {
        ShowMailbox();
        Input.Click(RowTitled(title), times);
        Agent.Settle(quiet: TimeSpan.FromMilliseconds(300), floor: TimeSpan.FromMilliseconds(500), cap: TimeSpan.FromSeconds(3));
    }

    /// <summary>
    /// Clicks <paramref name="field"/> and types <paramref name="text"/> into it with real
    /// keystrokes, and returns once the field holds what was typed. A WinUI text box reports the
    /// keyboard before Windows' text input has attached to it, and characters arriving in between
    /// are dropped without a trace, so typing waits a moment past the focus and, if the field still
    /// came up short, empties it and types once more.
    /// </summary>
    internal void TypeInto(Element field, string text)
    {
        Input.Click(Steady(field));
        if (!Eventually(() => Agent.Read(field).Focused, TimeSpan.FromSeconds(5)))
        {
            throw new InvalidOperationException($"#{field.Id} did not take the keyboard when clicked");
        }
        Agent.Settle(quiet: TimeSpan.FromMilliseconds(200), floor: TimeSpan.FromMilliseconds(300), cap: TimeSpan.FromSeconds(1));
        if (text.Length == 0)
        {
            return;
        }
        bool Holds() => Eventually(() => Agent.Read(field).Value == text, TimeSpan.FromSeconds(3));
        Input.Type(text);
        if (Holds())
        {
            return;
        }
        Agent.SetValue(field, "");
        Input.Type(text);
        if (!Holds())
        {
            throw new InvalidOperationException($"#{field.Id} never held what was typed into it ('{Agent.Read(field).Value}')");
        }
    }

    /// <summary>
    /// <paramref name="element"/> once it has stopped moving: a scroll is animated, and a press aimed
    /// at a rectangle read mid-scroll lands where the element was. Two reads 100 ms apart that agree.
    /// </summary>
    internal Element Steady(Element element)
    {
        var last = Agent.Read(element).Element;
        Eventually(() =>
        {
            Thread.Sleep(100);
            var now = Agent.Read(element).Element;
            var still = now.X == last.X && now.Y == last.Y && now.Width == last.Width && now.Height == last.Height;
            last = now;
            return still;
        }, TimeSpan.FromSeconds(3));
        return last;
    }

    /// <summary>
    /// The item in <paramref name="subject"/>'s row menu labelled with the catalog's
    /// <paramref name="key"/>, in whatever language the app came up in, or null when the menu does
    /// not offer it. A real right-click: the flyout is declared on the row template, which a keyboard
    /// shortcut sent to the focused container never reaches. Leaves the menu open.
    /// </summary>
    internal Element? RowMenuItem(string subject, string key)
    {
        ShowMailbox();
        Input.Click(RowTitled(subject), right: true);
        var labels = Catalog.Everywhere(key);
        Element? item = null;
        Eventually(
            () => (item = Agent.FindAll(new Filter { Type = "MenuItem", Window = Mailbox.Hwnd })
                .FirstOrDefault(m => labels.Contains(m.Name))) is not null,
            TimeSpan.FromSeconds(10));
        return item;
    }

    /// <summary>The window titled <paramref name="title"/>, waited for.</summary>
    internal AppWindow WaitWindow(string title, int timeoutSec = 20)
    {
        AppWindow? found = null;
        if (!Eventually(() => (found = Agent.Windows().FirstOrDefault(w => w.Title == title)) is not null, TimeSpan.FromSeconds(timeoutSec)))
        {
            throw new InvalidOperationException($"no window titled '{title}' within {timeoutSec} s; open windows: {string.Join(" | ", Agent.Windows().Select(w => w.Title))}");
        }
        return found!;
    }

    /// <summary>Raises the mailbox the way a person does, by pressing its title: a window the app
    /// has just opened stays in front until the reader asks for the mailbox.</summary>
    internal void ShowMailbox()
    {
        var title = Agent.Find(Filter.ById("PART_TitleText") with { Window = Mailbox.Hwnd })
            ?? throw new InvalidOperationException("the mailbox's title bar shows no title");
        Input.Click(title);
        Agent.Settle();
    }

    /// <summary>Closes every window but the mailbox, and waits until they are gone.</summary>
    internal void CloseExtraWindows()
    {
        foreach (var window in ExtraWindows())
        {
            SendMessage((nint)window.Hwnd, WmClose, 0, 0);
        }
        if (!Eventually(() => ExtraWindows().Count == 0, TimeSpan.FromSeconds(10)))
        {
            throw new InvalidOperationException($"windows would not close: {string.Join(" | ", ExtraWindows().Select(w => w.Title))}");
        }
    }

    /// <summary>
    /// Throws the open composer away by its Discard, answering "Discard draft?" when it asks, and
    /// waits until it is gone. Returns whether it asked; false, doing nothing, with no composer open.
    /// </summary>
    internal bool CloseComposer()
    {
        if (Agent.Find(Filter.ById("DiscardButton", "Button")) is not { } discard)
        {
            return false;
        }
        Agent.Invoke(discard);
        // The question's primary button is Discard (ComposerView.Drafts.cs). It is raised after the
        // press returns, so the floor is what gives it time to arrive.
        Agent.Settle(floor: TimeSpan.FromMilliseconds(300), cap: TimeSpan.FromMilliseconds(800));
        var confirm = Agent.Find(Filter.ById("PrimaryButton", "Button"));
        if (confirm is not null)
        {
            Agent.Invoke(confirm);
        }
        if (!Agent.WaitGone(Filter.ById("SendButton"), TimeSpan.FromSeconds(15)))
        {
            throw new InvalidOperationException("the composer is still open 15 s after its Discard was pressed");
        }
        return confirm is not null;
    }

    /// <summary>Puts away what the last case left: its windows, its composer, a dialog over the list.</summary>
    internal void Reset()
    {
        CloseExtraWindows();
        CloseComposer();
        foreach (var id in new[] { "CloseButton", "CancelButton" })
        {
            if (Agent.Find(Filter.ById(id, "Button") with { OnScreen = true }) is { } close)
            {
                Agent.Invoke(close);
                Agent.Settle();
            }
        }
    }

    /// <summary>The screen pixels under <paramref name="element"/>, read with its window in front:
    /// they come off the composited desktop, so anything lying over it would be read instead.</summary>
    internal static Bitmap Pixels(Element element)
    {
        Input.BringToFront(element.Window);
        var bitmap = new Bitmap((int)element.Width, (int)element.Height);
        using var graphics = Graphics.FromImage(bitmap);
        graphics.CopyFromScreen((int)element.X, (int)element.Y, 0, 0, bitmap.Size);
        return bitmap;
    }

    private const uint WmClose = 0x0010;

    [DllImport("user32.dll")] private static extern nint SendMessage(nint hwnd, uint message, nint wParam, nint lParam);
}
