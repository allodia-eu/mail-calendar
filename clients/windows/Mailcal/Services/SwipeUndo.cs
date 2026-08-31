// Swipe-with-undo for the message list: the state machine a completed swipe runs through. The
// Apple client's SwipeUndo.swift and Android's SwipeUndo.kt are the same machine, keep all three
// in step.
//
// Delete and Archive are DEFERRED: the row hides locally the moment you swipe, but no intent is
// dispatched until the undo window closes. Undo therefore cancels the action outright, nothing
// ever reached the server, so there is no "un-move" to get wrong (an IMAP move mints a new UID, so
// the key we hold would be dead anyway). Star is different: it isn't destructive and the row stays
// in place, so it applies immediately and Undo un-stars.
//
// This class owns the decisions and nothing else: it returns the dispatch each step resolves to
// rather than reaching for the model, so the commit/revert/supersede rules read on their own.
//
// The one deviation from the Apple/Android twins: they track the hidden rows here, while on Windows
// the hiding lives in MailboxModel (MailboxModel.SwipeSettings.cs), the projection that builds the
// bound row collection is there, so that is the only place a row can actually be withheld from the
// list. The rules below are otherwise identical.

using uniffi.mailcal_bindings;

namespace Allodia.Mailcal.Services;

/// <summary>What a step of the swipe machine resolves to. The controller decides; the caller
/// applies it through <see cref="MailboxModel"/>.</summary>
internal enum SwipeEffectKind
{
    /// <summary>Dispatch nothing.</summary>
    None,

    /// <summary>Dispatch the deferred move-to-trash.</summary>
    Delete,

    /// <summary>Dispatch the deferred archive.</summary>
    Archive,

    /// <summary>Star (or un-star) the message now.</summary>
    SetFlagged,
}

/// <summary>One dispatch the controller has resolved to.</summary>
internal readonly record struct SwipeEffect(SwipeEffectKind Kind, string Account, string Key, bool Flagged)
{
    /// <summary>Nothing to dispatch.</summary>
    public static SwipeEffect Nothing => new(SwipeEffectKind.None, string.Empty, string.Empty, false);

    /// <summary>Move the message to Trash.</summary>
    public static SwipeEffect Delete(string account, string key) =>
        new(SwipeEffectKind.Delete, account, key, false);

    /// <summary>Archive the message.</summary>
    public static SwipeEffect Archive(string account, string key) =>
        new(SwipeEffectKind.Archive, account, key, false);

    /// <summary>Set (or clear) the message's flag.</summary>
    public static SwipeEffect SetFlagged(string account, string key, bool flagged) =>
        new(SwipeEffectKind.SetFlagged, account, key, flagged);
}

/// <summary>A swipe waiting out its undo window. <paramref name="Id"/> makes every swipe distinct
/// even when the same message is swiped the same way twice, so a second swipe supersedes the first
/// rather than being mistaken for it.</summary>
/// <param name="Id">A per-swipe serial number.</param>
/// <param name="Account">The owning account.</param>
/// <param name="Key">The message key.</param>
/// <param name="Action">The configured action this edge is bound to.</param>
internal sealed record PendingSwipe(long Id, string Account, string Key, SwipeActionKind Action)
{
    /// <summary>Star toggles a flag in place, the row does not leave the list, so hiding it would
    /// be a lie.</summary>
    public bool HidesRow => Action != SwipeActionKind.Star;
}

/// <summary>
/// Owns what a swipe does and when.
///
/// The rules:
/// <list type="bullet">
/// <item>A Delete/Archive swipe hides the row and dispatches <b>nothing</b>; <see cref="Commit"/>
/// dispatches, <see cref="Revert"/> throws the action away.</item>
/// <item>A Star swipe dispatches immediately (the row stays, so a delayed star would look broken);
/// <see cref="Commit"/> is then a no-op and <see cref="Revert"/> un-stars.</item>
/// <item><see cref="Commit"/>/<see cref="Revert"/> act only on the swipe that still owns the undo
/// window (see <see cref="IsCurrent"/>); a stale one is dropped rather than dispatched.</item>
/// </list>
/// </summary>
internal sealed class SwipeUndoController
{
    private long _counter;

    /// <summary>The swipe currently inside its undo window, or <c>null</c>.</summary>
    public PendingSwipe? Pending { get; private set; }

    /// <summary>Records a completed swipe, applying Star at once and deferring Delete/Archive. The
    /// caller must first settle any still-pending swipe, a user swiping two messages in a row
    /// expects the first one to have happened.</summary>
    public SwipeEffect OnSwipe(string account, string key, SwipeActionKind action)
    {
        _counter += 1;
        var swipe = new PendingSwipe(_counter, account, key, action);
        Pending = swipe;
        return swipe.HidesRow
            ? SwipeEffect.Nothing
            : SwipeEffect.SetFlagged(account, key, true);
    }

    /// <summary>The undo window closed without an Undo: dispatch the deferred action.</summary>
    public SwipeEffect Commit(PendingSwipe swipe)
    {
        if (!IsCurrent(swipe))
        {
            return SwipeEffect.Nothing;
        }
        Pending = null;
        return swipe.Action switch
        {
            SwipeActionKind.Delete => SwipeEffect.Delete(swipe.Account, swipe.Key),
            SwipeActionKind.Archive => SwipeEffect.Archive(swipe.Account, swipe.Key),
            // Star was applied the moment the row was swiped; committing is a no-op.
            _ => SwipeEffect.Nothing,
        };
    }

    /// <summary>The user pressed Undo.</summary>
    public SwipeEffect Revert(PendingSwipe swipe)
    {
        if (!IsCurrent(swipe))
        {
            return SwipeEffect.Nothing;
        }
        Pending = null;
        return swipe.Action switch
        {
            // Nothing was ever dispatched, the caller just puts the row back.
            SwipeActionKind.Delete or SwipeActionKind.Archive => SwipeEffect.Nothing,
            _ => SwipeEffect.SetFlagged(swipe.Account, swipe.Key, false),
        };
    }

    /// <summary>
    /// Whether <paramref name="swipe"/> still owns the undo window. A commit/revert can arrive for
    /// a swipe that no longer does, in two ways, and BOTH must be dropped rather than dispatched:
    /// a newer swipe superseded it (the newer one is <see cref="Pending"/> now), or the user pressed
    /// Undo while the window timer was already awake, the timer is cancelled a moment later, so
    /// without this guard the undone swipe would still dispatch its Delete.
    /// </summary>
    public bool IsCurrent(PendingSwipe swipe) => Pending?.Id == swipe.Id;
}
