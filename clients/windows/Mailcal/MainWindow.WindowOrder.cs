// Whether the mailbox comes forward while a window is open beside it, and what it was doing when
// it did.
//
// This is a diagnostic, not a mechanism: it changes nothing about the windows and only writes at
// DEBUG level, so a normal run carries none of it. It stays wired because the rule it watches
// cannot be seen from outside the app. A steal lasts tens of milliseconds, so a screen read lands
// after the app has the front back and passes over a live fault; a log line is timestamped and
// cannot be missed by sampling (docs/reading-window.md, "Staying in front").
//
// WHAT IT SAYS. One line per activation, pairing it with what holds the mailbox's focus and how
// long ago each of its bound collections moved:
//
//   window order: mailbox code, focus on WebView2, rows 9s ago, sidebar 11ms ago
//   window order: mailbox pointer, focus on ListViewItem, rows 9s ago, sidebar 9s ago
//
// A `pointer` line is the reader clicking the mailbox, which is allowed to bring it forward. A
// `code` line after a window has opened is the fault, and what follows it says where to look: what
// holds the focus the mailbox just took, and which of its collections moved most recently.
//
// Ask what INPUT the mailbox still has in flight before suspecting anything it draws. The one such
// line this has caught was the list focusing the row under the pointer, from the second press of
// the double-click that had just opened the window, and every collection read "never" throughout,
// which is what ruled them out. The shell shows a window a dispatcher turn after it is asked for so
// that press finishes first (MainWindow.ReadingWindows.cs), and ReadingWindow.Tests reads these
// lines to hold it there.

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
        if (Content is UIElement root)
        {
            // WHETHER A PERSON ASKED. WinUI reports a pointer activation as a code one, so the
            // activation itself cannot say; a press in the mailbox, logged beside it, can.
            root.AddHandler(
                UIElement.PointerPressedEvent,
                new PointerEventHandler((_, _) => Log.Debug("window order: mailbox pressed")),
                handledEventsToo: true);
            // WHAT TOOK THE FOCUS, which is the half an activation alone cannot say. WinUI raises
            // this on the way IN, so the line lands beside the activation it explains, and the
            // state says who asked: Pointer is the reader, Programmatic is the app.
            root.GettingFocus += (_, e) => Log.Debug(
                $"window order: mailbox focus to {Describe(e.NewFocusedElement)}"
                + $" from {Describe(e.OldFocusedElement)}, {e.FocusState} by {e.InputDevice}");
        }
        Activated += (_, e) =>
        {
            var state = e.WindowActivationState switch
            {
                WindowActivationState.Deactivated => "lost the front",
                WindowActivationState.PointerActivated => "pointer",
                _ => "code",
            };
            Log.Debug($"window order: mailbox {state}, focus on {FocusHere()}, {Ages()}");
        };
    }

    // An element as one short token: the type, and its name when it has one.
    private static string Describe(object? element)
    {
        if (element is null)
        {
            return "nothing";
        }
        var name = element is FrameworkElement framework && !string.IsNullOrEmpty(framework.Name)
            ? $" '{framework.Name}'"
            : string.Empty;
        return element.GetType().Name + name;
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
