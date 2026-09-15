// The model's half of the detached reading window (docs/reading-window.md): which windows are
// open, and the core slot each one reads.
//
// The window itself is a WinUI Window (Views/ReadingWindow.cs). Nothing here is platform-specific:
// it is the same dispatch -> snapshot loop the pane uses, aimed at a different slot, so a client
// that never opens a window simply never fills the map.

using System.Collections.Generic;
using Allodia.Mailcal.ViewModels;
using uniffi.mailcal_bindings;

namespace Allodia.Mailcal.Services;

public sealed partial class MailboxModel
{
    // Every open detached reading window, by its core reader id. One entry per window, so this is
    // also the list of them: what the shell sweeps when the mailbox closes, and what tells the
    // reload pass whose body to re-pull. The PANE is OpenedMessage/Reading and is deliberately not
    // a key here, exactly as the core keeps the two apart.
    private readonly Dictionary<string, WindowReader> _readingWindows = new();

    /// <summary>
    /// Opens <paramref name="opened"/> in its own window's slot and answers the reader the window
    /// draws, or the reader already up for that message.
    /// </summary>
    /// <remarks>
    /// One window per message (docs/reading-window.md): a second open of the same message answers
    /// the existing reader and dispatches nothing, so the caller brings that window forward instead
    /// of stacking a second one holding a second copy of the same body.
    ///
    /// The core does the rest. The fetch, the bounded retry for an account still dialing, the
    /// threshold before a loading state is announced and the mark-read are the pane's, and are
    /// reached through the same intent; only the slot the body lands in differs.
    /// </remarks>
    public WindowReader OpenReadingWindow(OpenedMessage opened)
    {
        var id = ReadingWindows.IdFor(opened.Account, opened.Key);
        if (_readingWindows.TryGetValue(id, out var existing))
        {
            return existing;
        }
        var reader = new WindowReader(opened);
        _readingWindows[id] = reader;
        _app?.Dispatch(new Intent.OpenMessageInWindow(id, opened.Account, opened.Key));
        return reader;
    }

    /// <summary>Whether a window is already open on this message.</summary>
    public bool HasReadingWindow(string account, string key) =>
        _readingWindows.ContainsKey(ReadingWindows.IdFor(account, key));

    /// <summary>
    /// The window has gone: forget it and tell the core to drop the body it was holding.
    /// </summary>
    /// <remarks>
    /// Both halves matter. A sanitised body carries every inline image resolved into it, so it is
    /// the largest thing either side holds per window, and a session that opened twenty messages
    /// would otherwise hold twenty of them for as long as it ran. This is not an
    /// <see cref="Intent"/>: it moves nothing, marks nothing and signals nothing.
    /// </remarks>
    public void CloseReadingWindow(string id)
    {
        if (_readingWindows.Remove(id))
        {
            _app?.CloseReadingWindow(id);
        }
    }

    /// <summary>
    /// Re-pulls every open window's body, on a <c>Surface.Reading</c> signal and only then.
    /// </summary>
    /// <remarks>
    /// The signal names no reader, so each open window reads its own slot back beside the pane.
    /// An answered invitation is why this is not merely the window's own open arriving: the core
    /// republishes that message's card to every reader showing it, so a window still offering
    /// Accept over a time the user has already agreed to is what this prevents.
    /// </remarks>
    private void ReloadReadingWindows()
    {
        if (_app is null)
        {
            return;
        }
        foreach (var (id, reader) in _readingWindows)
        {
            reader.TakeBody(Project(_app.ReadingWindowView(id)));
        }
    }

    /// <summary>
    /// Frees every open window's slot in the core.
    /// </summary>
    /// <remarks>
    /// The mailbox closing takes its reading windows with it (docs/reading-window.md), and this is
    /// the core-side half of that: the shell dismisses the windows, this drops what they held. Belt
    /// as well as braces, since each window's own close frees its slot; a window torn down without
    /// its Closed event would otherwise leave a body with nobody left to read it.
    /// </remarks>
    public void CloseAllReadingWindows()
    {
        foreach (var id in _readingWindows.Keys)
        {
            _app?.CloseReadingWindow(id);
        }
        _readingWindows.Clear();
    }
}
