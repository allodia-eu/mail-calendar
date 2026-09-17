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
    /// The reader is minted here and the window is shown a turn later (<see cref="ShowWindow"/>),
    /// so the body is already being fetched while the press that asked for it finishes.
    /// </para>
    /// </remarks>
    internal void OpenReadingWindow(OpenedMessage opened)
    {
        var reader = Model.OpenReadingWindow(opened);
        ShowWindow(() =>
        {
            var window = _readingWindows.Find(w => w.ReaderId == reader.Id);
            if (window is null)
            {
                window = new ReadingWindow(Model, reader);
                _readingWindows.Add(window);
                Log.Info("reading window: opened");
            }
            return window;
        });
    }

    /// <summary>
    /// Shows the window <paramref name="build"/> returns, once the input that asked for it has
    /// been delivered.
    /// </summary>
    /// <remarks>
    /// A turn later rather than now, and this is the whole of why a window opened from the list
    /// stays in front. A double-click raises <c>DoubleTapped</c> in the middle of its second
    /// press, so a window shown from that handler is shown while the list still has the press to
    /// finish; the row under the pointer then takes focus, and focusing an element activates the
    /// window it is in, which puts the mailbox back over the window it had just opened. It did so on
    /// every open measured, 31 to 41 ms after the window took the front, and on none at all once
    /// the press is allowed to finish first. The same gesture through the row's context menu never
    /// showed it, because a menu item's invoke carries no press for the list to act on
    /// (docs/client-traps.md).
    /// <para>
    /// A dispatcher turn rather than waiting for the trailing click: a press whose pointer leaves
    /// the row before it is released raises no click at all, and a window that has been asked for
    /// is owed whatever the gesture does next.
    /// </para>
    /// </remarks>
    private void ShowWindow(Func<Window> build) =>
        DispatcherQueue.TryEnqueue(() =>
        {
            // Recording before the window says it opened, so nothing the mailbox does between the
            // two goes unseen (MainWindow.WindowOrder.cs).
            WatchWindowOrder();
            var window = build();
            WindowChrome.Present(window);
            KeepInFrontWhileItSettles(window);
        });

    // The window the mailbox may not climb over yet, and until when.
    private Window? _presented;
    private DateTimeOffset _settlingUntil = DateTimeOffset.MinValue;
    private bool _watchingSettling;

    /// <summary>How long after opening a window the mailbox may not climb over it.</summary>
    /// <remarks>
    /// Twice the slowest activation ever measured with the app left alone, which is 330 ms, and no
    /// more than that: everything longer that looked like the fault turned out to be a person or a
    /// test driver raising the mailbox on purpose. The reach matters because a correction cannot
    /// tell those apart. A press in the mailbox ends it, but Alt-Tab, the taskbar and a script's
    /// <c>SetForegroundWindow</c> raise no press, so for as long as this lasts they are fought;
    /// at three seconds that was long enough for a click meant for the mailbox to land in the
    /// window instead.
    /// </remarks>
    private static readonly TimeSpan Settling = TimeSpan.FromMilliseconds(750);

    /// <summary>
    /// Puts <paramref name="window"/> back in front if the mailbox takes the front while the
    /// window is still settling.
    /// </summary>
    /// <remarks>
    /// The one thing here that is not a cure, and it is deliberately blunt because the cause is
    /// not known. On a mailbox syncing a real account, EVERY open is followed by the mailbox
    /// activating itself, measured between 90 ms and 1.3 s later over 22 opens, with its bound
    /// collections untouched throughout; a quiet seeded mailbox shows almost none of it, which is
    /// what makes this expensive to chase and what a gate on the harness alone will not see
    /// (docs/client-traps.md). Whatever it is, the reader asked for the window, so the window is
    /// put back.
    /// <para>
    /// A press anywhere in the mailbox ends the correction, because a reader asking for the
    /// mailbox outranks it, and a window's activation state cannot answer that on its own: WinUI
    /// reports a pointer activation as a code one, so the pointer itself is the only evidence of a
    /// person. <see cref="Settling"/> ends it otherwise.
    /// </para>
    /// </remarks>
    private void KeepInFrontWhileItSettles(Window window)
    {
        _presented = window;
        _settlingUntil = DateTimeOffset.Now + Settling;
        if (_watchingSettling)
        {
            return;
        }
        _watchingSettling = true;
        if (Content is UIElement content)
        {
            content.AddHandler(
                UIElement.PointerPressedEvent,
                new PointerEventHandler((_, _) => _presented = null),
                handledEventsToo: true);
        }
        Activated += (_, e) =>
        {
            if (e.WindowActivationState == WindowActivationState.Deactivated
                || DateTimeOffset.Now > _settlingUntil
                || _presented is not { } target
                || !IsStillOpen(target))
            {
                return;
            }
            // Enqueued rather than called here: this runs inside the mailbox's own activation, and
            // taking the front back from inside it re-enters the handler.
            DispatcherQueue.TryEnqueue(() =>
            {
                if (_presented == target && IsStillOpen(target))
                {
                    // Once per window, never a tug of war: the fault takes the front once, so a
                    // mailbox that comes forward a second time is somebody insisting, and this
                    // has no way to tell that from the fault repeating.
                    _presented = null;
                    Log.Debug("window order: putting the window back, it was still settling");
                    WindowChrome.BringToForeground(target);
                }
            });
        };
    }

    private bool IsStillOpen(Window window) =>
        (window is ReadingWindow reading && _readingWindows.Contains(reading))
        || (window is ComposerWindow composer && _composerWindows.Contains(composer));

    /// <summary>Opens a draft in a window of its own, for a reply or forward raised inside a
    /// reading window.</summary>
    internal void OpenComposerWindow(ComposeRequest request) =>
        ShowWindow(() =>
        {
            var window = new ComposerWindow(Model, request);
            _composerWindows.Add(window);
            Log.Info("composer window: opened");
            return window;
        });

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
