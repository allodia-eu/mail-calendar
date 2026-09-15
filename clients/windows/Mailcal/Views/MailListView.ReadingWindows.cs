// Opening a message in a window of its own, from the list (docs/reading-window.md): the
// double-click, and the named context-menu item that is the same thing without a pointer.
//
// THE ORDER THESE ARRIVE IN IS THE WHOLE PROBLEM, and it was measured rather than assumed. A
// double-click on a row raises, in this order:
//
//   1. ItemClick, from the first press, which opens the row in the reading pane;
//   2. DoubleTapped, which is the first moment anything knows a window was wanted;
//   3. ItemClick AGAIN, from the second press, ~80ms after the double-tap.
//
// So the pane has already moved by (2), and (3) lands after the correction. The contract is that a
// double-click leaves the pane alone (docs/reading-window.md), and three things together hold it:
//
//   * every ItemClick records what the pane was showing BEFORE it opened anything;
//   * RowDoubleClick refuses (3), so the trailing click neither re-opens the row nor overwrites
//     that record with the message (1) just opened;
//   * (2) opens the window and puts the pane back to what was recorded.
//
// The record must therefore survive until (3) has been refused. Clearing it at (2), which an
// earlier version did, defeats the whole arrangement: the trailing click then reads as a fresh
// first click and re-opens the message in the pane, which is exactly the symptom this prevents.
//
// The restore re-opens rather than un-does, because the core's pane slot has been overwritten by
// then. The pane keeps drawing what it had while that runs (ReadingHandover).
//
// The handler is added with handledEventsToo, on the LIST rather than on a row template: the row is
// a ListViewItem and a conversation's sub-rows are Buttons, both of which handle their own pointer
// input, so a DoubleTapped declared in the item markup is not reliably delivered.

using System;
using System.Runtime.InteropServices;
using Allodia.Mailcal.Services;
using Allodia.Mailcal.ViewModels;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Input;

namespace Allodia.Mailcal.Views;

public sealed partial class MailListView
{
    /// <summary>Counts the two clicks of a double-click, at the rate this desktop is set to.</summary>
    private readonly RowDoubleClick _doubleClick =
        new(TimeSpan.FromMilliseconds(Math.Max(GetDoubleClickTime(), 1)));

    /// <summary>
    /// What the reading pane was showing before the click currently being handled. Restored when
    /// that click turns out to be the first half of a double-click.
    /// </summary>
    private OpenedMessage? _paneBeforeClick;

    private void InitReadingWindows() =>
        RowsList.AddHandler(
            UIElement.DoubleTappedEvent,
            new DoubleTappedEventHandler(OnRowDoubleTapped),
            handledEventsToo: true);

    /// <summary>
    /// Records what the pane holds before a click opens something over it, and says whether this
    /// click is the second of a double-click and so must open nothing.
    /// </summary>
    private bool ClickOpensInPane(string rowId)
    {
        if (_doubleClick.CompletesDoubleClick(rowId, DateTimeOffset.UtcNow))
        {
            return false;
        }
        _paneBeforeClick = Model?.OpenedMessage;
        return true;
    }

    // A double-click on a MESSAGE row opens it in a window of its own. A conversation HEADER is not
    // a message and keeps what a second click already did there, which on Windows is collapsing the
    // thread it just expanded.
    private void OnRowDoubleTapped(object sender, DoubleTappedRoutedEventArgs e)
    {
        var opened = (e.OriginalSource as FrameworkElement)?.DataContext switch
        {
            ThreadMessageItem message => MailboxModel.ThreadMessageHeader(message),
            MailRow { IsThread: false } row => MailboxModel.RowHeader(row),
            _ => null,
        };
        if (opened is null)
        {
            return;
        }
        OpenInWindow(opened);
        // The pane was opened by the first press of this same double-click; put it back.
        Model?.RestoreReadingPane(_paneBeforeClick);
    }

    // The same thing as a named item on the row's context menu. A double-click cannot be reached
    // from the keyboard or invoked by a screen reader, so the gesture is not the affordance: this
    // item is, and it is what the capability matrix claims (docs/reading-window.md).
    private void OnOpenInWindow(object sender, RoutedEventArgs e)
    {
        if (RowOf(sender) is { } row)
        {
            OpenInWindow(MailboxModel.RowHeader(row));
        }
    }

    private void OnThreadMessageOpenInWindow(object sender, RoutedEventArgs e)
    {
        if ((sender as FrameworkElement)?.Tag is ThreadMessageItem message)
        {
            OpenInWindow(MailboxModel.ThreadMessageHeader(message));
        }
    }

    // Nothing is asked about an unsent draft here, and nothing is closed: a window takes no column
    // away from the composer and does not touch the pane, so there is no draft for it to drop.
    //
    // The click tracker is deliberately left armed. The trailing ItemClick of this very
    // double-click has not arrived yet, and it is the tracker that refuses it.
    private void OpenInWindow(OpenedMessage opened) => App.Shell?.OpenReadingWindow(opened);

    // The desktop's own double-click speed. Someone who has slowed theirs down has done so for a
    // reason, and a hardcoded 500 ms would ignore it.
    [DllImport("user32.dll")] private static extern uint GetDoubleClickTime();
}
