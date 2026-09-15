// The shell's half of the desktop's extra windows (docs/reading-window.md): opening them, keeping
// the list of them, and sweeping them when the mailbox closes.
//
// They are the shell's rather than the model's because a window is host state: the model holds one
// reader per open window, which is what the core needs, and nothing more. The shell holds the
// windows themselves, because it is what owns them and what has to take them with it when it goes.

using System.Collections.Generic;
using Allodia.Mailcal.Services;
using Allodia.Mailcal.ViewModels;
using Allodia.Mailcal.Views;
using Microsoft.UI.Xaml;

namespace Allodia.Mailcal;

public sealed partial class MainWindow
{
    private readonly List<ReadingWindow> _readingWindows = new();
    private readonly List<ComposerWindow> _composerWindows = new();

    /// <summary>
    /// Opens <paramref name="opened"/> in a window of its own, or brings the window already
    /// showing that message forward.
    /// </summary>
    /// <remarks>
    /// One window per message (docs/reading-window.md). The model answers the same reader for a
    /// second open of one message and dispatches nothing, so the branch below is which window to
    /// raise, never whether to fetch again.
    /// <para>
    /// Presented rather than merely activated, either way. <c>Activate()</c> puts a new window at
    /// the top of the z-order and leaves the FOREGROUND on the mailbox, which is still mid-input
    /// because the double-click that asked for this window is still being delivered to it; Windows
    /// then restores its foreground window to the top and the new one drops behind it about fifty
    /// milliseconds after appearing (Services/WindowChrome.cs).
    /// </para>
    /// </remarks>
    internal void OpenReadingWindow(OpenedMessage opened)
    {
        var reader = Model.OpenReadingWindow(opened);
        if (_readingWindows.Find(w => w.ReaderId == reader.Id) is { } existing)
        {
            WindowChrome.Present(existing);
            return;
        }
        var window = new ReadingWindow(Model, reader);
        _readingWindows.Add(window);
        WindowChrome.Present(window);
        Log.Info("reading window: opened");
    }

    /// <summary>Opens a draft in a window of its own, for a reply or forward raised inside a
    /// reading window.</summary>
    internal void OpenComposerWindow(ComposeRequest request)
    {
        var window = new ComposerWindow(Model, request);
        _composerWindows.Add(window);
        WindowChrome.Present(window);
        Log.Info("composer window: opened");
    }

    /// <summary>A reading window has closed and is no longer the shell's to sweep.</summary>
    internal void ForgetReadingWindow(ReadingWindow window) => _readingWindows.Remove(window);

    /// <summary>A composer window has closed and is no longer the shell's to sweep.</summary>
    internal void ForgetComposerWindow(ComposerWindow window) => _composerWindows.Remove(window);

    /// <summary>
    /// Closes every reading and composer window this mailbox opened.
    /// </summary>
    /// <remarks>
    /// They were opened out of the mailbox, and leaving them behind leaves the app running as a
    /// scatter of message windows with no way back to the list (docs/reading-window.md). Iterated
    /// over a copy, because each Close raises Closed, which removes that window from the list it
    /// is being iterated over.
    /// </remarks>
    private void CloseChildWindows()
    {
        foreach (var window in _readingWindows.ToArray())
        {
            window.Close();
        }
        foreach (var window in _composerWindows.ToArray())
        {
            window.Close();
        }
        // Belt as well as braces: each window's own close frees its slot, but a window torn down
        // without its Closed event would leave a body in the core with nobody left to read it.
        Model.CloseAllReadingWindows();
    }

    /// <summary>Repaints the open child windows when the app's appearance changes.</summary>
    private void ApplyAppearanceToChildWindows(uniffi.mailcal_bindings.Appearance appearance)
    {
        foreach (var window in _readingWindows)
        {
            window.AppearanceApplied(appearance);
        }
        foreach (var window in _composerWindows)
        {
            window.AppearanceApplied(appearance);
        }
    }
}
