// Why a window that opened in front is no longer in front.
//
// This is a diagnostic, not a mechanism: it changes nothing about the windows and only writes at
// DEBUG level, so a normal run carries none of it. It exists because the fault does not reproduce
// on the seeded harness. A mailbox that is quiet reconciles its list a handful of times; one
// syncing a real account reconciles it continuously, and only the second loses the window.
//
// The z-order itself no longer depends on the answer: WindowChrome.Own settles that by a rule of
// the system. What is still open is WHY the mailbox takes the foreground unasked, which is worth
// knowing because it is what ownership is paying for, and because the day WinUI stops doing it the
// windows can go back to being peers (docs/reading-window.md, "Known gaps").
//
// WHAT IT IS FOR. WinUI 3 reassigns focus when the element holding it is removed, and that
// reassignment activates the window the element was in. So the question is never "did the mailbox
// come forward", it is "what did the mailbox do immediately before it came forward". These lines
// pair a mailbox activation with the state Windows reports for it and with the list's last
// reconcile, which is what tells a removed row from a user's click:
//
//   window order: mailbox activated (code), focus on WebView2, rows 9s ago, sidebar 11ms ago
//   window order: mailbox activated (pointer), focus on ListViewItem, rows 9s ago, sidebar 9s ago
//
// A `pointer` line is the reader clicking the mailbox, which is allowed to bring it forward. A
// `code` line is the fault, and the two halves after it say why: what holds the focus the mailbox
// just took, and which of its collections moved most recently. A collection that changed
// milliseconds before the activation is the one whose elements were torn down.

using System;
using System.Collections.Specialized;
using Allodia.Mailcal.Services;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Input;

namespace Allodia.Mailcal;

public sealed partial class MainWindow
{
    private readonly System.Collections.Generic.Dictionary<string, DateTimeOffset> _changedAt = new();
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
        Track("rows", Model.Rows);
        Track("accounts", Model.Accounts);
        Track("folders", Model.Folders);
        Track("sidebar", SidebarItems);
        Activated += (_, e) =>
        {
            if (e.WindowActivationState == WindowActivationState.Deactivated)
            {
                return;
            }
            var state = e.WindowActivationState == WindowActivationState.PointerActivated
                ? "pointer"
                : "code";
            Log.Debug($"window order: mailbox activated ({state}), focus on {FocusHere()}, {Ages()}");
        };
    }

    private void Track(string name, INotifyCollectionChanged collection)
    {
        _changedAt[name] = DateTimeOffset.MinValue;
        collection.CollectionChanged += (_, _) => _changedAt[name] = DateTimeOffset.Now;
    }

    // Every tracked collection and how long ago it last moved, so one line says which of them
    // changed just before the activation and which have been still for minutes.
    private string Ages() =>
        string.Join(
            ", ",
            System.Linq.Enumerable.Select(
                _changedAt,
                pair => pair.Value == DateTimeOffset.MinValue
                    ? $"{pair.Key} never"
                    : $"{pair.Key} {(DateTimeOffset.Now - pair.Value).TotalMilliseconds:F0}ms"));

    // What holds the keyboard focus inside the mailbox at this moment. The type is the useful
    // half: a control that was rebuilt announces itself by being the thing focus landed on.
    private string FocusHere()
    {
        try
        {
            var focused = FocusManager.GetFocusedElement(Content.XamlRoot);
            if (focused is null)
            {
                return "nothing";
            }
            var name = focused is FrameworkElement element && !string.IsNullOrEmpty(element.Name)
                ? $" '{element.Name}'"
                : string.Empty;
            return focused.GetType().Name + name;
        }
        catch (Exception ex)
        {
            return $"unreadable ({ex.GetType().Name})";
        }
    }
}
