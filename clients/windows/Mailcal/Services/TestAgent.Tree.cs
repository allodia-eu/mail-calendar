// The agent's view of the windows: every registered window's automation peers, open popups
// included, walked in the order UI Automation reports them. A WebView2's content is not walked: it
// is the browser's own tree, and nothing a suite looks up lives in it.

#if DEBUG
using System;
using System.Collections.Generic;
using System.Linq;
using System.Runtime.CompilerServices;
using System.Text.Json.Nodes;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Automation.Peers;
using Microsoft.UI.Xaml.Controls.Primitives;
using Microsoft.UI.Xaml.Media;

namespace Allodia.Mailcal.Services;

internal static partial class TestAgent
{
    // A handle names a peer for as long as the peer lives, and never keeps it alive: a closed
    // window has to be freed whatever the suite still holds (ReleaseProbe).
    private static readonly Dictionary<int, WeakReference<AutomationPeer>> Handles = new();
    private static readonly ConditionalWeakTable<AutomationPeer, StrongBox<int>> HandleOf = new();
    private static readonly ConditionalWeakTable<AutomationPeer, WeakReference<AutomationPeer>> ParentOf = new();
    private static readonly ConditionalWeakTable<AutomationPeer, WeakReference<Window>> WindowOf = new();
    private static int _nextHandle;

    /// <summary>The windows still open, the mailbox first.</summary>
    private static List<Window> OpenWindows()
    {
        var open = new List<Window>();
        Tracked.RemoveAll(reference => !reference.TryGetTarget(out _));
        foreach (var reference in Tracked.ToList())
        {
            if (!reference.TryGetTarget(out var window))
            {
                continue;
            }
            try
            {
                if (window.Content is not null && window.AppWindow is not null)
                {
                    open.Add(window);
                }
            }
            catch (System.Runtime.InteropServices.COMException)
            {
                // Closing, between its close and the Closed event that untracks it.
                Tracked.Remove(reference);
            }
        }
        return open;
    }

    /// <summary>Every peer under the roots, depth first, each visited once.</summary>
    private static IEnumerable<AutomationPeer> Walk(IEnumerable<(AutomationPeer Peer, Window Window)> roots)
    {
        var seen = new HashSet<AutomationPeer>(ReferenceEqualityComparer.Instance);
        var stack = new Stack<(AutomationPeer Peer, Window Window)>(roots.Reverse());
        while (stack.Count > 0)
        {
            var (peer, window) = stack.Pop();
            if (!seen.Add(peer))
            {
                continue;
            }
            WindowOf.AddOrUpdate(peer, new WeakReference<Window>(window));
            yield return peer;
            if (IsBrowser(peer) || ChildrenOf(peer) is not { } children)
            {
                continue;
            }
            for (var i = children.Count - 1; i >= 0; i--)
            {
                if (children[i] is not { } child)
                {
                    continue;
                }
                ParentOf.AddOrUpdate(child, new WeakReference<AutomationPeer>(peer));
                stack.Push((child, window));
            }
        }
    }

    /// <summary>A peer's children. A popup's peer reports none in-process, where UI Automation
    /// shows what the popup holds, so its content stands in.</summary>
    private static IList<AutomationPeer>? ChildrenOf(AutomationPeer peer)
    {
        IList<AutomationPeer>? children;
        try
        {
            children = peer.GetChildren();
        }
        catch (Exception e) when (e is OutOfMemoryException or System.Runtime.InteropServices.COMException)
        {
            // The invitation card's Expander, while it is offscreen, answers E_OUTOFMEMORY, which
            // WinRT raises as OutOfMemoryException, every time it is asked. UI Automation passes over
            // a subtree it cannot read, and so does the walk: one such peer would otherwise fail
            // every query until the card is drawn again.
            return null;
        }
        if (children is { Count: > 0 })
        {
            return children;
        }
        return peer is FrameworkElementAutomationPeer { Owner: Popup { Child: UIElement content } }
            ? TopPeers(content).ToList()
            : children;
    }

    /// <summary>Whether the element is in UI Automation's raw view only (an avatar, a print page):
    /// the control view a screen reader reads leaves it out, so every lookup does too. Its children
    /// keep their own view.</summary>
    private static bool HiddenFromAssistiveTechnology(AutomationPeer peer) =>
        peer is FrameworkElementAutomationPeer { Owner: { } owner }
        && Microsoft.UI.Xaml.Automation.AutomationProperties.GetAccessibilityView(owner) == AccessibilityView.Raw;

    private static string? SignalOf(AutomationPeer peer) =>
        peer is FrameworkElementAutomationPeer { Owner: { } owner } && Signals.TryGetValue(owner, out var state) ? state : null;

    private static bool IsBrowser(AutomationPeer peer) =>
        peer.GetClassName() is "WebView2" or "Microsoft.UI.Xaml.Controls.WebView2";

    /// <summary>Each window's content, then whatever it has open over it: a dialog, a flyout, a
    /// tooltip lives in a popup of its own.</summary>
    private static IEnumerable<(AutomationPeer, Window)> WindowRoots(Window window)
    {
        if (window.Content is not UIElement content)
        {
            yield break;
        }
        foreach (var peer in TopPeers(content))
        {
            yield return (peer, window);
        }
        if (content.XamlRoot is not { } root)
        {
            yield break;
        }
        // From the popup itself rather than its child: a dialog's AutomationId is on its popup.
        foreach (var popup in VisualTreeHelper.GetOpenPopupsForXamlRoot(root))
        {
            foreach (var peer in TopPeers(popup))
            {
                yield return (peer, window);
            }
        }
    }

    /// <summary>
    /// The highest peers at or under <paramref name="element"/>. A layout panel has no peer of its
    /// own, and a window's content is usually one, so its children's peers are the roots, found the
    /// way a peer finds its own children: down the visual tree to the first element that has one.
    /// </summary>
    private static IEnumerable<AutomationPeer> TopPeers(UIElement element)
    {
        if (FrameworkElementAutomationPeer.CreatePeerForElement(element) is { } peer)
        {
            yield return peer;
            yield break;
        }
        var count = VisualTreeHelper.GetChildrenCount(element);
        for (var i = 0; i < count; i++)
        {
            if (VisualTreeHelper.GetChild(element, i) is UIElement child)
            {
                foreach (var below in TopPeers(child))
                {
                    yield return below;
                }
            }
        }
    }

    /// <summary>The roots a request searches: under one element, in one window, or in all of them.</summary>
    private static IEnumerable<(AutomationPeer, Window)> RootsOf(JsonObject request)
    {
        if (request["under"] is not null)
        {
            var peer = PeerOf(HandleIn(request, "under"));
            var window = WindowFor(peer) ?? OpenWindows()[0];
            return [(peer, window)];
        }
        return request["window"] switch
        {
            null => OpenWindows().SelectMany(WindowRoots),
            JsonValue value when value.TryGetValue<long>(out var hwnd) =>
                WindowRoots(OpenWindows().FirstOrDefault(w => Hwnd(w) == hwnd)
                    ?? throw new AgentException($"no open window has handle {hwnd}", "no-element")),
            JsonValue value when value.TryGetValue<string>(out var title) =>
                OpenWindows().Where(w => w.AppWindow.Title.Contains(title, StringComparison.OrdinalIgnoreCase)).SelectMany(WindowRoots),
            _ => throw new AgentException("bad arguments: 'window' is part of a title, or a window's h", "bad-request"),
        };
    }

    /// <summary>The peers a request's filter accepts, in walk order. Under an element, the element
    /// itself is not a match: a caller asking what is inside a row does not mean the row.</summary>
    private static IEnumerable<AutomationPeer> Matches(JsonObject request)
    {
        Allowed(request, FilterArgs, "a filter");
        var id = Text(request, "id");
        var text = Text(request, "text");
        var like = Text(request, "textLike");
        var type = Text(request, "type");
        var role = Text(request, "role");
        var icon = Text(request, "icon");
        var enabled = Flag(request, "enabled");
        var hidden = Flag(request, "hidden") ?? false;
        var signal = Text(request, "signal");
        var has = request["has"] switch
        {
            null => null,
            JsonObject inner => inner,
            _ => throw new AgentException("bad arguments: 'has' is a filter", "bad-request"),
        };
        var under = request["under"] is not null ? PeerOf(HandleIn(request, "under")) : null;
        foreach (var peer in Walk(RootsOf(request)))
        {
            if (ReferenceEquals(peer, under)
                || HiddenFromAssistiveTechnology(peer)
                || (!hidden && peer.IsOffscreen())
                || (id is not null && peer.GetAutomationId() != id)
                || (text is not null && !Said(peer).Contains(text))
                || (like is not null && !Said(peer).Any(said => said.Contains(like, StringComparison.OrdinalIgnoreCase)))
                || (type is not null && !IsOfClass(peer, type))
                || (role is not null && peer.GetAutomationControlType().ToString() != role)
                // Nothing here draws a named icon, so a filter that asks for one matches nothing.
                || icon is not null
                || (enabled is { } wanted && peer.IsEnabled() != wanted)
                || (signal is not null && SignalOf(peer) != signal)
                || (has is not null && !Contains(peer, has)))
            {
                continue;
            }
            yield return peer;
        }
    }

    /// <summary>Whether something under <paramref name="peer"/> matches <paramref name="filter"/>:
    /// a row holding a subject, a dialog holding a button.</summary>
    private static bool Contains(AutomationPeer peer, JsonObject filter)
    {
        var inner = filter.DeepClone().AsObject();
        inner["under"] = HandleFor(peer);
        inner.Remove("index");
        return Matches(inner).Any();
    }

    /// <summary>Every match, or with <c>index</c> only the one at that position.</summary>
    private static JsonObject Query(JsonObject request)
    {
        var found = Number(request, "index") is { } index
            ? Matches(request).Skip(index).Take(1).ToList()
            : Matches(request).ToList();
        return new JsonObject { ["elements"] = new JsonArray(found.Select(p => (JsonNode)Describe(p)).ToArray()) };
    }

    private static JsonObject Read(JsonObject request) => Describe(PeerOf(HandleIn(request)), detail: true);

    /// <summary>One line per peer in walk order, indented by depth, for a caller that has to say
    /// what was on screen. Lists what lookups leave out, too.</summary>
    private static JsonObject Dump(JsonObject request)
    {
        var hidden = Flag(request, "hidden") ?? false;
        var lines = new List<string>();
        foreach (var peer in Walk(RootsOf(request)))
        {
            if (!hidden && peer.IsOffscreen())
            {
                continue;
            }
            var depth = 0;
            for (var at = peer; ParentOf.TryGetValue(at, out var up) && up.TryGetTarget(out var parent); at = parent)
            {
                depth++;
            }
            lines.Add($"{new string(' ', depth * 2)}[{HandleFor(peer)}] {peer.GetAutomationControlType()} #{peer.GetAutomationId()} '{peer.GetName()}'{(peer.IsOffscreen() ? " (offscreen)" : "")}");
        }
        return new JsonObject { ["text"] = string.Join('\n', lines) + "\n" };
    }

    private static JsonObject ListWindows() => new()
    {
        ["windows"] = new JsonArray(OpenWindows().Select(w => (JsonNode)new JsonObject
        {
            ["h"] = Hwnd(w),
            ["title"] = w.AppWindow.Title,
            ["kind"] = w.GetType().Name,
        }).ToArray()),
    };

    private static long Hwnd(Window window) => (long)WinRT.Interop.WindowNative.GetWindowHandle(window);

    private static int HandleFor(AutomationPeer peer)
    {
        var box = HandleOf.GetValue(peer, _ => new StrongBox<int>(++_nextHandle));
        Handles[box.Value] = new WeakReference<AutomationPeer>(peer);
        return box.Value;
    }

    private static int HandleIn(JsonObject request, string field = "h") =>
        Number(request, field) ?? throw new AgentException($"bad arguments: no element in '{field}'", "bad-request");

    private static AutomationPeer PeerOf(int handle)
    {
        if (!Handles.TryGetValue(handle, out var reference))
        {
            throw new AgentException($"no element has handle {handle}", "no-element");
        }
        return reference.TryGetTarget(out var peer)
            ? peer
            : throw new AgentException($"element {handle} has gone: query again", "gone");
    }

    /// <summary>
    /// The open window <paramref name="peer"/>'s element is drawn in, or null when it is in none.
    /// Read off the element's own root, climbing to the nearest walked ancestor that has an element
    /// where the peer has none, as a list's item peers do. The window a walk met it in comes last:
    /// that one may have closed since, and a closed window's handle reads as 0.
    /// </summary>
    private static Window? WindowFor(AutomationPeer peer)
    {
        var open = OpenWindows();
        for (AutomationPeer? at = peer; at is not null; at = ParentPeer(at))
        {
            if (at is FrameworkElementAutomationPeer { Owner: { XamlRoot: { } root } })
            {
                return open.FirstOrDefault(w => ReferenceEquals(w.Content?.XamlRoot, root));
            }
        }
        return WindowOf.TryGetValue(peer, out var walked) && walked.TryGetTarget(out var window) && open.Contains(window)
            ? window
            : null;
    }

    /// <summary>
    /// The peer above <paramref name="peer"/>: the one the walk met it under, else the one it names
    /// itself. The walk's record is keyed on the managed wrapper, and a weak reference to a WinRT
    /// object gives back a new wrapper for the same peer once the walked one is collected, so after a
    /// collection the record is gone.
    /// </summary>
    private static AutomationPeer? ParentPeer(AutomationPeer peer) =>
        ParentOf.TryGetValue(peer, out var up) && up.TryGetTarget(out var parent) ? parent : peer.GetParent();

    private struct ClientPoint
    {
        public int X;
        public int Y;
    }

    [System.Runtime.InteropServices.DllImport("user32.dll")]
    private static extern bool ClientToScreen(nint hwnd, ref ClientPoint point);
}
#endif
