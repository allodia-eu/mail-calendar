// The shell's half of the desktop's extra windows (docs/reading-window.md): opening them, keeping
// the list of them, and sweeping them when the mailbox closes.
//
// They are the shell's rather than the model's because a window is host state: the model holds one
// reader per open window, which is what the core needs, and nothing more. The shell holds the
// windows themselves, because it is what owns them and what has to take them with it when it goes.

using System;
using System.Collections.Generic;
using Allodia.Mailcal.Services;
using Allodia.Mailcal.ViewModels;
using Allodia.Mailcal.Views;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Input;

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
    /// Presented rather than merely activated. <c>Activate()</c> puts a new window at the top of
    /// the z-order and leaves the FOREGROUND on the mailbox, which is still mid-input because the
    /// double-click that asked for this window is still being delivered to it; Windows then
    /// restores its foreground window to the top (Services/WindowChrome.cs).
    /// </para>
    /// </remarks>
    internal void OpenReadingWindow(OpenedMessage opened)
    {
        var reader = Model.OpenReadingWindow(opened);
        // The correction is armed for a window already up as well as for a new one: the
        // double-click that asked for it puts the pane back either way, and it is that restore
        // the mailbox takes the front on (Views/MailListView.ReadingWindows.cs).
        var window = _readingWindows.Find(w => w.ReaderId == reader.Id);
        if (window is null)
        {
            window = new ReadingWindow(Model, reader);
            _readingWindows.Add(window);
            Log.Info("reading window: opened");
        }
        WindowChrome.Present(window);
        KeepInFrontWhileItSettles(window);
        WatchWindowOrder();
    }


    // The window the mailbox must not climb over yet, and until when.
    private Window? _settling;
    private DateTimeOffset _settlingUntil = DateTimeOffset.MinValue;
    private bool _watchingSettling;

    /// <summary>
    /// Puts <paramref name="window"/> back in front if the mailbox takes the front while the
    /// window is still opening.
    /// </summary>
    /// <remarks>
    /// Windows leaves a new top-level window in front on its own, which is why Outlook's and
    /// Thunderbird's message windows stay where they are put. This app loses that because the
    /// MAILBOX activates itself while a window opens, measured on the seeded harness tens of
    /// milliseconds after the window appears, with nothing in between that the app logs. The
    /// reading pane's WebView2 was the obvious suspect and is not it: its navigation completes more
    /// than a second before the activation arrives.
    /// <para>
    /// The honest name for this is a bounded correction, not a cure: it lasts <c>Settling</c> and
    /// then stops, so the mailbox can be raised over the window a moment later exactly as it can
    /// on the other two desktops. That boundedness is the whole point. Ownership would guarantee
    /// the z-order and take the ability to raise the mailbox away for good, which is not a trade
    /// this contract makes (docs/reading-window.md).
    /// </para>
    /// </remarks>
    private void KeepInFrontWhileItSettles(Window window)
    {
        _settling = window;
        _settlingUntil = DateTimeOffset.Now + Settling;
        if (_watchingSettling)
        {
            return;
        }
        _watchingSettling = true;
        // A press anywhere in the mailbox ends the correction at once. Without this the reader
        // cannot raise the mailbox for as long as Settling lasts, which trades one window that
        // will not come forward for another. PointerPressed on the content, handledEventsToo,
        // because a press that a control handles is still the reader asking for the mailbox, and
        // the window's activation state cannot answer this: WinUI reports a pointer activation as
        // a code one, so the only reliable evidence of a person is the pointer itself.
        if (Content is UIElement content)
        {
            content.AddHandler(
                UIElement.PointerPressedEvent,
                new PointerEventHandler((_, _) => _settling = null),
                handledEventsToo: true);
        }
        Activated += (_, e) =>
        {
            if (e.WindowActivationState == WindowActivationState.Deactivated
                || _settling is not { } settling
                || DateTimeOffset.Now > _settlingUntil
                || !IsStillOpen(settling))
            {
                return;
            }
            // Enqueued rather than called here: this runs inside the mailbox's own activation, and
            // taking the front back from inside it re-enters the handler.
            DispatcherQueue.TryEnqueue(() =>
            {
                if (_settling == settling && IsStillOpen(settling))
                {
                    WindowChrome.BringToForeground(settling);
                }
            });
        };
    }

    /// <summary>How long after opening a window the mailbox is not allowed to climb over it.</summary>
    /// <remarks>
    /// Long enough to cover both measured steals and short enough that a reader who turns to the
    /// mailbox is never fighting it: the second steal lands about 1.5s in.
    /// </remarks>
    private static readonly TimeSpan Settling = TimeSpan.FromSeconds(3);

    private bool IsStillOpen(Window window) =>
        (window is ReadingWindow reading && _readingWindows.Contains(reading))
        || (window is ComposerWindow composer && _composerWindows.Contains(composer));

    /// <summary>Opens a draft in a window of its own, for a reply or forward raised inside a
    /// reading window.</summary>
    internal void OpenComposerWindow(ComposeRequest request)
    {
        var window = new ComposerWindow(Model, request);
        _composerWindows.Add(window);
        WindowChrome.Present(window);
        KeepInFrontWhileItSettles(window);
        WatchWindowOrder();
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
