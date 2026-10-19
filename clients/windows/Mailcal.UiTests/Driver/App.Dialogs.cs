// Dialogs over the mailbox: the one holding a given element, once it has opened, and closing them
// all until the last has finished closing.

namespace Allodia.Mailcal.UiTests.Driver;

internal sealed partial class App
{
    /// <summary>
    /// The open dialog holding <paramref name="holding"/>, once it has finished opening: a dialog is
    /// a popup over the shell (the shell keeps an empty popup of its own beside it), and it moves its
    /// focus, and its form with it, as it finishes opening.
    /// </summary>
    internal Element Dialog(Filter holding)
    {
        var dialog = Agent.Wait(new Filter { Type = "Window", Window = Mailbox.Hwnd, Has = holding }, TimeSpan.FromSeconds(15))
            ?? throw new InvalidOperationException("no dialog opened");
        Agent.Settle(quiet: TimeSpan.FromMilliseconds(300));
        return dialog;
    }

    /// <summary>
    /// Closes every open dialog, the editor stacked over a detail included, and waits until the
    /// last of them has finished closing. A dialog's own popup goes at once, but the layer that dims
    /// the window behind it stays until the close is complete, and the app opens no other dialog
    /// before then: a press on the next event in that time opens nothing. And a dialog closed can
    /// raise another in its place, as backing out of a scope question reopens the editor it was
    /// asked about, so once it has closed one it looks again after a moment.
    /// </summary>
    internal void CloseDialogs()
    {
        var close = Filter.ById("CloseButton", "Button");
        var closed = false;
        for (var i = 0; i < 6; i++)
        {
            var open = Agent.FindAll(close);
            if (open.Count == 0)
            {
                if (!closed)
                {
                    break;
                }
                closed = false;
                Agent.Settle(quiet: TimeSpan.FromMilliseconds(300), floor: TimeSpan.FromMilliseconds(400));
                continue;
            }
            Agent.Invoke(open[0]);
            Eventually(() => Agent.FindAll(close).Count < open.Count, TimeSpan.FromSeconds(5));
            closed = true;
        }
        // A popup with no size draws nothing and catches no press, whatever it holds: a closed
        // suggestion list or tooltip can leave one behind.
        IReadOnlyList<Element> popups = [];
        if (!Eventually(() => (popups = Agent.FindAll(new Filter { Type = "Window", Window = Mailbox.Hwnd })
                .Where(w => w.Class == "Popup" && w.Width > 0 && w.Height > 0).ToList()).Count == 0, TimeSpan.FromSeconds(5)))
        {
            var held = popups.Select(p => $"'{p.Name}' #{p.Id} {p.Width}x{p.Height} holding {Agent.FindAll(new Filter { Under = p }).Count}");
            throw new InvalidOperationException($"a dialog was still closing 5 s after it was closed: {string.Join(" | ", held)}");
        }
    }
}
