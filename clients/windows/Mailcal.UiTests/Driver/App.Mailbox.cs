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
        Agent.ScrollIntoView(row);
        return Agent.Find(filter) ?? row;
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
