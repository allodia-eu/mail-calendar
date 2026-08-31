// Swipe actions + undo for the message list. The Windows half of a feature the core has
// carried since #67 and the other three clients already ship; the decisions live in
// Services/SwipeUndo.cs, shared in shape with Apple's SwipeUndo.swift and Android's SwipeUndo.kt.
//
// TWO THINGS SHAPE THIS, and they are what make Windows different from the touch clients:
//
// 1. THE GESTURE DOES NOT ANSWER TO A MOUSE. `SwipeControl` takes touch, pen, and precision-touchpad
//    input, a two-finger trackpad swipe reveals the action, as on macOS (confirmed on device), but
//    a mouse gets nothing, and plenty of Windows desktops have neither a touch screen nor a precision
//    touchpad. So the row's context menu is an EQUAL path, not a fallback: Move to Trash and Archive
//    there run through this same deferred machine and raise the same undo bar. A mouse-only user gets
//    the identical behaviour, undo included, otherwise the feature is invisible to them entirely.
//
// 2. THE UNDO WINDOW COMES WITH THE GESTURE. A swipe that dispatched immediately would delete
//    straight away where every other client hands the user ~4s to take it back. Delete and Archive
//    hide the row and dispatch NOTHING until the window elapses; Undo cancels the action outright.
//
// Known gap, inherited from Android and Apple: killing the app inside the undo window loses a
// deferred action. See docs, it is not silently swallowed, just not persisted.

using Allodia.Mailcal.Services;
using Allodia.Mailcal.ViewModels;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Controls;
using Microsoft.UI.Xaml.Media;
using uniffi.mailcal_bindings;

namespace Allodia.Mailcal.Views;

public sealed partial class MailListView
{
    /// <summary>How long the undo bar stays up before a deferred Delete/Archive is dispatched.</summary>
    private static readonly TimeSpan UndoWindow = TimeSpan.FromSeconds(4);

    /// <summary>How long a committed Delete/Archive stays hidden after its intent is dispatched. The
    /// core hides the row itself (an optimistic removal it publishes before any network round-trip),
    /// so this only has to outlast one snapshot hop. It matters when the core REJECTS the edit: the
    /// core then restores the row, and un-hiding lets it reappear instead of staying invisible until
    /// the app restarts.</summary>
    private static readonly TimeSpan CommitHideGrace = TimeSpan.FromSeconds(4);

    private readonly SwipeUndoController _swipe = new();

    /// <summary>Cancels the in-flight undo window when a newer swipe supersedes it.</summary>
    private CancellationTokenSource? _undoWindow;

    // The revealed buttons, rebuilt whenever the setting changes. WinUI shares one SwipeItems across
    // every SwipeControl that points at it (the documented pattern), so this is built once per
    // setting rather than once per row.
    private SwipeItems? _leftItems;
    private SwipeItems? _rightItems;

    /// <summary>Builds the swipe buttons for the configured actions and keeps them in step with the
    /// setting. Called from <c>Init</c> once the model is bound.</summary>
    private void InitSwipe()
    {
        RebuildSwipeItems();
        if (Model is null)
        {
            return;
        }
        Model.SwipeSettingsChanged += RebuildSwipeItems;
#if DEBUG
        Model.Rows.CollectionChanged += (_, _) => ApplySwipeLaunchHook();
#endif
    }

#if DEBUG
    // DEBUG/verification only, and the reason it exists is worth stating: a SwipeControl gesture is
    // touch/pen-only and a context flyout cannot be opened by synthetic input either, so this
    // feature is otherwise impossible to drive from a test, the same wall the rest of the Windows
    // debug tooling hit, which is why it is built on MAILCAL_* launch hooks rather than pixel taps
    // (clients/windows/control.ps1). MAILCAL_SWIPE=delete|archive|star performs that action on the
    // first message row once the list loads, through the very same PerformSwipe the gesture and the
    // context menu use, so what it exercises is the real path, not a test-only one.
    //
    // One-shot: rows arrive over several snapshots, and PerformSwipe itself triggers another.
    private bool _hookSwipeApplied;

    private void ApplySwipeLaunchHook()
    {
        if (_hookSwipeApplied || Model is null)
        {
            return;
        }
        var action = Environment.GetEnvironmentVariable("MAILCAL_SWIPE")?.Trim().ToLowerInvariant() switch
        {
            "delete" => (SwipeActionKind?)SwipeActionKind.Delete,
            "archive" => SwipeActionKind.Archive,
            "star" => SwipeActionKind.Star,
            _ => null,
        };
        // Only flat message rows swipe, so a threaded view's conversation headers are skipped.
        if (action is null || Model.Rows.FirstOrDefault(row => !row.IsThread) is not { } target)
        {
            return;
        }
        _hookSwipeApplied = true;
        Log.Info($"launch hook: MAILCAL_SWIPE={action} -> swiping the first message row");
        // Off the current event, not inline: we are inside Rows.CollectionChanged, and the swipe
        // hides the row, which re-projects and mutates Rows. Mutating an ObservableCollection from
        // inside its own CollectionChanged handler throws. (The real gesture and the context menu
        // never run from there, so only this hook has to step off the event.)
        DispatcherQueue.TryEnqueue(() => PerformSwipe(target, action.Value));
    }
#endif

    // One button per edge: the CONFIGURED action, not a fixed pair.
    //
    // The direction mapping is the one place this can silently invert. WinUI names its collections
    // for the edge the buttons are revealed FROM, while the setting names the direction the finger
    // travels, so dragging LEFT (SwipeDirection.Left) uncovers the RIGHT edge, and vice versa.
    private void RebuildSwipeItems()
    {
        if (Model is null)
        {
            return;
        }
        var settings = Model.SwipeActions;
        _rightItems = SwipeItemsFor(settings.Left);   // finger travels left  -> right edge
        _leftItems = SwipeItemsFor(settings.Right);   // finger travels right -> left edge

        // Re-point the SwipeControls already realised in the list; new ones pick the items up in
        // OnSwipeControlLoaded as they are created.
        foreach (var control in RealizedSwipeControls())
        {
            ApplySwipeItems(control);
        }
    }

    private SwipeItems SwipeItemsFor(SwipeActionKind action)
    {
        var item = new SwipeItem
        {
            Text = SwipeActionLabel(action),
            IconSource = new SymbolIconSource { Symbol = SwipeActionSymbol(action) },
            BehaviorOnInvoked = SwipeBehaviorOnInvoked.Close,
        };
        // Capture the action in the handler rather than deducing it from which edge fired: the two
        // edges can be bound to the SAME action, so identity of the button tells you nothing.
        item.Invoked += (_, args) => OnSwipeInvoked(args, action);
        // Reveal (the default), not Execute: the user swipes to uncover the button, then presses it.
        // A full-swipe Execute fires on the gesture alone, which is a lot of authority for one
        // stray drag, the undo bar is the safety net, not the confirmation.
        return new SwipeItems { item };
    }

    /// <summary>The settings (and swipe-button) label for an action.</summary>
    internal static string SwipeActionLabel(SwipeActionKind action) => action switch
    {
        SwipeActionKind.Archive => L10n.SwipeActionArchive(),
        SwipeActionKind.Star => L10n.SwipeActionStar(),
        _ => L10n.SwipeActionDelete(),
    };

    /// <summary>The past-tense confirmation the undo bar shows, by the time it is up, the row has
    /// already left the list (or been starred).</summary>
    private static string SwipeDoneLabel(SwipeActionKind action) => action switch
    {
        SwipeActionKind.Archive => L10n.SwipeDoneArchive(),
        SwipeActionKind.Star => L10n.SwipeDoneStar(),
        _ => L10n.SwipeDoneDelete(),
    };

    /// <summary>The symbol on an action's swipe button and in the settings picker.</summary>
    internal static Symbol SwipeActionSymbol(SwipeActionKind action) => action switch
    {
        SwipeActionKind.Archive => Symbol.MoveToFolder,
        SwipeActionKind.Star => Symbol.Favorite,
        _ => Symbol.Delete,
    };

    // A SwipeControl in the row template has just been realised: point it at the shared buttons.
    private void OnSwipeControlLoaded(object sender, RoutedEventArgs e)
    {
        if (sender is SwipeControl control)
        {
            ApplySwipeItems(control);
        }
    }

    private void ApplySwipeItems(SwipeControl control)
    {
        control.LeftItems = _leftItems;
        control.RightItems = _rightItems;
    }

    // The realised SwipeControls under the list, so a settings change re-points the rows already on
    // screen rather than waiting for them to be recycled.
    private IEnumerable<SwipeControl> RealizedSwipeControls() => Descendants(RowsList).OfType<SwipeControl>();

    private static IEnumerable<DependencyObject> Descendants(DependencyObject root)
    {
        var count = VisualTreeHelper.GetChildrenCount(root);
        for (var i = 0; i < count; i++)
        {
            var child = VisualTreeHelper.GetChild(root, i);
            yield return child;
            foreach (var nested in Descendants(child))
            {
                yield return nested;
            }
        }
    }

    // A revealed swipe button was pressed. The SwipeControl's DataContext is the row it wraps.
    private void OnSwipeInvoked(SwipeItemInvokedEventArgs args, SwipeActionKind action)
    {
        if (args.SwipeControl.DataContext is MailRow row)
        {
            PerformSwipe(row, action);
        }
    }

    /// <summary>
    /// A completed swipe (or the context-menu equivalent) on a row. An earlier swipe still inside
    /// its undo window is committed first, a user acting on two messages in a row expects the first
    /// one to have happened.
    /// </summary>
    private void PerformSwipe(MailRow row, SwipeActionKind action)
    {
        if (Model is null || row.IsThread)
        {
            return; // only flat message rows swipe (as on Apple and Android).
        }
        if (_swipe.Pending is { } previous)
        {
            FinishSwipe(previous, undone: false);
        }
        var effect = _swipe.OnSwipe(row.Account, row.Key, action);
        if (_swipe.Pending is { HidesRow: true })
        {
            Model.HideRow(row.Account, row.Key);
        }
        Apply(effect);
        ShowUndoBar(action);
        StartUndoWindow();
    }

    // Runs the pending swipe's undo window, cancelling any window still running for an older one.
    private void StartUndoWindow()
    {
        _undoWindow?.Cancel();
        _undoWindow?.Dispose();
        var cts = new CancellationTokenSource();
        _undoWindow = cts;
        var swipe = _swipe.Pending;
        if (swipe is null)
        {
            return;
        }
        _ = RunUndoWindowAsync(swipe, cts.Token);
    }

    private async Task RunUndoWindowAsync(PendingSwipe swipe, CancellationToken token)
    {
        try
        {
            await Task.Delay(UndoWindow, token);
        }
        catch (TaskCanceledException)
        {
            return; // superseded by a newer swipe, or undone.
        }
        FinishSwipe(swipe, undone: false);
    }

    private void OnUndoSwipe(object sender, RoutedEventArgs e)
    {
        if (_swipe.Pending is { } swipe)
        {
            FinishSwipe(swipe, undone: true);
        }
    }

    /// <summary>
    /// Closes a swipe's undo window: dispatch the deferred action, or throw it away. A committed row
    /// is un-hidden after a grace period, so a core-REJECTED edit doesn't leave it invisible until
    /// the app restarts.
    ///
    /// A swipe that no longer owns the window (undone, or superseded and already settled) is a no-op
    /// in the controller, and must not schedule an un-hide either, that would reveal a row the
    /// NEWER swipe is legitimately hiding.
    /// </summary>
    private void FinishSwipe(PendingSwipe swipe, bool undone)
    {
        if (Model is null || !_swipe.IsCurrent(swipe))
        {
            return;
        }
        _undoWindow?.Cancel();
        Apply(undone ? _swipe.Revert(swipe) : _swipe.Commit(swipe));
        HideUndoBar();

        if (undone)
        {
            // Nothing was ever dispatched for a Delete/Archive: put the row straight back.
            if (swipe.HidesRow)
            {
                Model.UnhideRow(swipe.Account, swipe.Key);
            }
            return;
        }
        if (swipe.HidesRow)
        {
            _ = ReleaseHideAfterGraceAsync(swipe);
        }
    }

    private async Task ReleaseHideAfterGraceAsync(PendingSwipe swipe)
    {
        await Task.Delay(CommitHideGrace);
        Model?.UnhideRow(swipe.Account, swipe.Key);
    }

    // Applies the controller's decision to the core.
    private void Apply(SwipeEffect effect)
    {
        switch (effect.Kind)
        {
            case SwipeEffectKind.Delete:
                Model?.Delete(effect.Account, effect.Key);
                break;
            case SwipeEffectKind.Archive:
                Model?.Archive(effect.Account, effect.Key);
                break;
            case SwipeEffectKind.SetFlagged:
                Model?.SetFlagged(effect.Account, effect.Key, effect.Flagged);
                break;
            default:
                break;
        }
    }

    private void ShowUndoBar(SwipeActionKind action)
    {
        UndoBar.Message = SwipeDoneLabel(action);
        UndoBar.IsOpen = true;
    }

    private void HideUndoBar() => UndoBar.IsOpen = false;
}
