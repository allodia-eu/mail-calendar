// Why a window that opened in front is no longer in front.
//
// This is a diagnostic, not a mechanism: it changes nothing about the windows and only writes at
// DEBUG level, so a normal run carries none of it. It exists because the fault does not reproduce
// on the seeded harness. A mailbox that is quiet reconciles its list a handful of times; one
// syncing a real account reconciles it continuously, and only the second loses the window.
//
// WHAT IT IS FOR. WinUI 3 reassigns focus when the element holding it is removed, and that
// reassignment activates the window the element was in. So the question is never "did the mailbox
// come forward", it is "what did the mailbox do immediately before it came forward". These lines
// pair a mailbox activation with the state Windows reports for it and with the list's last
// reconcile, which is what tells a removed row from a user's click:
//
//   window order: mailbox activated (code), list changed 14ms ago (+3/-0 rows)
//   window order: mailbox activated (pointer), list changed 9s ago (+0/-0 rows)
//
// A `pointer` line is the reader clicking the mailbox, which is allowed to bring it forward. A
// `code` line close behind a list change is the fault, and the counts name the operation to fix.

using System;
using Allodia.Mailcal.Services;
using Microsoft.UI.Xaml;

namespace Allodia.Mailcal;

public sealed partial class MainWindow
{
    private DateTimeOffset _listChangedAt = DateTimeOffset.MinValue;
    private int _rowsAdded;
    private int _rowsRemoved;
    private bool _watchingOrder;

    /// <summary>
    /// Starts recording why the mailbox comes forward, for as long as a window is open beside it.
    /// </summary>
    /// <remarks>
    /// Wired on the first window rather than at launch: a mailbox on its own can take the
    /// foreground as often as it likes, and there is nothing for it to take it from.
    /// </remarks>
    internal void WatchWindowOrder()
    {
        if (_watchingOrder)
        {
            return;
        }
        _watchingOrder = true;
        Model.Rows.CollectionChanged += (_, e) =>
        {
            _listChangedAt = DateTimeOffset.Now;
            _rowsAdded += e.NewItems?.Count ?? 0;
            _rowsRemoved += e.OldItems?.Count ?? 0;
        };
        Activated += (_, e) =>
        {
            if (e.WindowActivationState == WindowActivationState.Deactivated)
            {
                return;
            }
            var state = e.WindowActivationState == WindowActivationState.PointerActivated
                ? "pointer"
                : "code";
            var since = _listChangedAt == DateTimeOffset.MinValue
                ? "never"
                : $"{(DateTimeOffset.Now - _listChangedAt).TotalMilliseconds:F0}ms ago";
            Log.Debug(
                $"window order: mailbox activated ({state}), list changed {since} "
                + $"(+{_rowsAdded}/-{_rowsRemoved} rows)");
            _rowsAdded = 0;
            _rowsRemoved = 0;
        };
    }
}
