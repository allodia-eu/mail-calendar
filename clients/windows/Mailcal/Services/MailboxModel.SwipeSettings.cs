// The swipe surface of the model: the two per-direction actions, and the local row-hiding that
// the deferred dispatch needs. The twin of MailboxModel.SendSettings.cs. Split into its own
// partial to keep MailboxModel.cs under the 500-line limit.
//
// The actions live in the core, which signals Surface.Settings after a change; the list rebuilds
// its buttons on that signal.

using uniffi.mailcal_bindings;

namespace Allodia.Mailcal.Services;

public sealed partial class MailboxModel
{
    // Rows withheld from the projection while a swipe waits out its undo window, and briefly
    // after it commits (see SwipeUndoController). A deferred Delete/Archive dispatches NOTHING
    // until the window closes, so the core still has the row and would happily project it; the only
    // place it can actually be kept off the list is here, where the bound collection is built.
    //
    // Keyed by MailRow.Id ("m:<account>:<key>"), which is what the projection reconciles on. Only
    // flat rows swipe, so only they are ever hidden.
    private readonly HashSet<string> _hiddenRowKeys = new();

    // These two carry the generated UniFFI types (SwipeSettings / SwipeDirection / SwipeActionKind),
    // which are `internal`, the generated types stay confined to this service layer. Internal is
    // enough: the list and the settings dialog that consume them are in this same assembly.

    /// <summary>The per-direction swipe actions, read fresh from the core, which owns them and
    /// their defaults; null while no core is open.</summary>
    internal SwipeSettings? SwipeActions => _app?.SwipeSettings();

    /// <summary>Binds one swipe direction to an action. The list rebuilds its buttons on the
    /// Settings signal that follows.</summary>
    internal void SetSwipeAction(SwipeDirection direction, SwipeActionKind action) =>
        DispatchPreference(new PreferenceIntent.SetSwipeAction(direction, action));

    /// <summary>Raised when the core's swipe actions are not the ones last raised, so the list can
    /// rebuild its swipe buttons. They are built imperatively (a <c>SwipeItems</c> is not a
    /// bindable per-row value), so the list cannot simply re-read the setting on the next render.</summary>
    public event Action? SwipeSettingsChanged;

    // The swipe actions SwipeSettingsChanged last announced.
    private SwipeSettings? _swipeActionsRaised;

    /// <summary>Raises <see cref="SwipeSettingsChanged"/> when the core's swipe actions moved: on
    /// the first reload a core answers, and on a Settings signal that changed them.</summary>
    private void FollowSwipeActions()
    {
        if (SwipeActions is not { } current || current == _swipeActionsRaised)
        {
            return;
        }
        _swipeActionsRaised = current;
        SwipeSettingsChanged?.Invoke();
    }

    /// <summary>Hides a message row locally, its swipe is inside the undo window, so nothing has
    /// been dispatched and the core still has it.</summary>
    internal void HideRow(string account, string key)
    {
        if (_hiddenRowKeys.Add(RowIdOf(account, key)))
        {
            Reload();
        }
    }

    /// <summary>
    /// Stops hiding a message row. Called on Undo (the action never happened, so the row simply
    /// returns) and, after a grace period, on a committed swipe too, by then the core has
    /// published a snapshot without the row, so it is a no-op *unless* the core REJECTED the edit
    /// and restored it, in which case this is what lets the row reappear instead of staying
    /// invisible until the app restarts.
    /// </summary>
    internal void UnhideRow(string account, string key)
    {
        if (_hiddenRowKeys.Remove(RowIdOf(account, key)))
        {
            Reload();
        }
    }

    /// <summary>Whether <paramref name="rowId"/> is currently withheld by a pending or
    /// just-committed swipe. Read by the projection.</summary>
    private bool IsRowHidden(string rowId) => _hiddenRowKeys.Contains(rowId);

    // The projection's id for a flat message row (MailboxModel.Projection.cs BuildRow). The account
    // is part of it because a provider key is unique only WITHIN an account, so the unified inbox
    // can show two rows carrying the same key.
    private static string RowIdOf(string account, string key) => "m:" + account + ":" + key;
}
