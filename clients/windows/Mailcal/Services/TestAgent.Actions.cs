// What the agent does to an element, through the same providers UI Automation calls, and the two
// ways it waits: for a condition, and for the windows to stop changing.

#if DEBUG
using System;
using System.Diagnostics;
using System.Linq;
using System.Text;
using System.Text.Json.Nodes;
using System.Threading;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Automation.Peers;
using Microsoft.UI.Xaml.Automation.Provider;
using Microsoft.UI.Xaml.Controls;

namespace Allodia.Mailcal.Services;

internal static partial class TestAgent
{
    /// <summary>The state a suite asserts on, where the element has it.</summary>
    private static void Detail(AutomationPeer peer, JsonObject described)
    {
        described["focused"] = peer.HasKeyboardFocus();
        described["help"] = peer.GetHelpText();
        // The weight the text is drawn in, which UI Automation's text pattern reports for a TextBlock.
        if (peer is FrameworkElementAutomationPeer { Owner: TextBlock drawn })
        {
            described["fontWeight"] = drawn.FontWeight.Weight;
        }
        if (peer.GetPattern(PatternInterface.Value) is IValueProvider value)
        {
            described["value"] = value.Value;
            described["readOnly"] = value.IsReadOnly;
        }
        else if (TextOwner(peer) is { } text)
        {
            described["value"] = text.Get();
            described["readOnly"] = false;
        }
        if (peer.GetPattern(PatternInterface.Toggle) is IToggleProvider toggle)
        {
            described["toggle"] = toggle.ToggleState.ToString();
        }
        if (peer.GetPattern(PatternInterface.SelectionItem) is ISelectionItemProvider item)
        {
            described["selected"] = item.IsSelected;
        }
        if (peer.GetPattern(PatternInterface.ExpandCollapse) is IExpandCollapseProvider expand)
        {
            described["expanded"] = expand.ExpandCollapseState.ToString();
        }
        // What a picker shows, never its own name, which is its header. A closed drop-down has no
        // containers, so the item stands in for its own when there is none.
        if (peer is FrameworkElementAutomationPeer { Owner: Microsoft.UI.Xaml.Controls.Primitives.Selector selector }
            && selector.SelectedItem is { } chosen)
        {
            described["selection"] = selector.ContainerFromItem(chosen) is UIElement container
                && FrameworkElementAutomationPeer.CreatePeerForElement(container) is { } shown
                    ? shown.GetName()
                    : chosen.ToString();
        }
    }

    /// <summary>Presses an element: Invoke where it has it, else selects it. A list row opens on
    /// its item click, which only Invoke raises, so Invoke is tried first.</summary>
    private static JsonObject Invoke(JsonObject request)
    {
        var peer = PeerOf(HandleIn(request));
        if (peer.GetPattern(PatternInterface.Invoke) is IInvokeProvider invoke)
        {
            invoke.Invoke();
        }
        else if (peer.GetPattern(PatternInterface.SelectionItem) is ISelectionItemProvider item)
        {
            item.Select();
        }
        else
        {
            throw new AgentException($"'{peer.GetName()}' can be neither invoked nor selected; is it a label rather than a control?");
        }
        return new JsonObject { ["ok"] = true };
    }

    private static JsonObject Select(JsonObject request)
    {
        var peer = PeerOf(HandleIn(request));
        var item = peer.GetPattern(PatternInterface.SelectionItem) as ISelectionItemProvider
            ?? throw new AgentException($"'{peer.GetName()}' cannot be selected");
        item.Select();
        return new JsonObject { ["ok"] = true };
    }

    /// <summary>Sets a text box's value the way UI Automation does, which the box takes as an edit.</summary>
    private static JsonObject SetValue(JsonObject request)
    {
        var peer = PeerOf(HandleIn(request));
        var text = (string?)request["value"] ?? "";
        if (peer.GetPattern(PatternInterface.Value) is IValueProvider value)
        {
            value.SetValue(text);
        }
        else
        {
            var owner = TextOwner(peer) ?? throw new AgentException($"'{peer.GetName()}' takes no value");
            owner.Set(text);
        }
        return new JsonObject { ["ok"] = true };
    }

    /// <summary>
    /// The text a box holds, for the boxes whose value UI Automation reads from the framework rather
    /// than from the peer: in-process, their peer offers no value provider, so the box is read and
    /// set directly, which is what UI Automation's value pattern does to it.
    /// </summary>
    private static (Func<string> Get, Action<string> Set)? TextOwner(AutomationPeer peer) =>
        (peer as FrameworkElementAutomationPeer)?.Owner switch
        {
            TextBox box => (() => box.Text, value => box.Text = value),
            PasswordBox box => (() => box.Password, value => box.Password = value),
            _ => null,
        };

    private static JsonObject Toggle(JsonObject request)
    {
        var peer = PeerOf(HandleIn(request));
        var toggle = peer.GetPattern(PatternInterface.Toggle) as IToggleProvider
            ?? throw new AgentException($"'{peer.GetName()}' does not toggle");
        toggle.Toggle();
        return new JsonObject { ["toggle"] = toggle.ToggleState.ToString() };
    }

    /// <summary>Opens a combo box or a drop-down, whose items have no peers until it is open, or
    /// with <paramref name="open"/> false closes it again.</summary>
    private static JsonObject Expand(JsonObject request, bool open)
    {
        var peer = PeerOf(HandleIn(request));
        var expand = peer.GetPattern(PatternInterface.ExpandCollapse) as IExpandCollapseProvider
            ?? throw new AgentException($"'{peer.GetName()}' does not expand");
        if (open)
        {
            expand.Expand();
        }
        else
        {
            expand.Collapse();
        }
        return new JsonObject { ["ok"] = true };
    }

    /// <summary>Scrolls the element into its list's view, through the nearest scroll item: the
    /// element a suite presses is often a text inside a row.</summary>
    private static JsonObject ScrollIntoView(JsonObject request)
    {
        AutomationPeer? at = PeerOf(HandleIn(request));
        while (at is not null)
        {
            if (at.GetPattern(PatternInterface.ScrollItem) is IScrollItemProvider scroll)
            {
                scroll.ScrollIntoView();
                return new JsonObject { ["ok"] = true };
            }
            at = ParentPeer(at);
        }
        return new JsonObject { ["ok"] = false };
    }

    private static JsonObject Focus(JsonObject request)
    {
        PeerOf(HandleIn(request)).SetFocus();
        return new JsonObject { ["ok"] = true };
    }

    /// <summary>The nearest ancestor with a peer of an element the walk has met, or none: a layout
    /// panel between them has no peer, so the answer can be several levels up.</summary>
    private static JsonObject Parent(JsonObject request)
    {
        var peer = PeerOf(HandleIn(request));
        var parent = ParentPeer(peer);
        return new JsonObject { ["elements"] = parent is null ? new JsonArray() : new JsonArray(Describe(parent)) };
    }

    /// <summary>The element that has the keyboard, in whichever window has it, or none.</summary>
    private static JsonObject Focused()
    {
        var focused = Walk(OpenWindows().SelectMany(WindowRoots)).FirstOrDefault(peer => peer.HasKeyboardFocus());
        return new JsonObject { ["elements"] = focused is null ? new JsonArray() : new JsonArray(Describe(focused)) };
    }

    /// <summary>Scrolls the first vertically scrollable element at or under the given one to
    /// <c>percent</c> of its height, its end when none is given: as a person scrolls a long form to
    /// the section at its foot, or a list to the row a virtualised list has not built yet.</summary>
    private static JsonObject ScrollTo(JsonObject request)
    {
        var percent = (double?)request["percent"] ?? 100;
        var root = PeerOf(HandleIn(request));
        var window = WindowFor(root) ?? OpenWindows()[0];
        foreach (var peer in Walk([(root, window)]))
        {
            if (peer.GetPattern(PatternInterface.Scroll) is IScrollProvider { VerticallyScrollable: true } scroll)
            {
                scroll.SetScrollPercent(-1, percent);
                return new JsonObject { ["ok"] = true };
            }
        }
        return new JsonObject { ["ok"] = false };
    }

    /// <summary>
    /// Waits until the query finds something (or, with <c>absent</c>, nothing), checking on every
    /// turn of the UI thread rather than on a timer a suite has to guess. Answers with what it found
    /// last, and <c>ok</c> false when the time ran out.
    /// </summary>
    private static JsonObject Wait(JsonObject request)
    {
        var absent = (bool?)request["absent"] ?? false;
        var timeout = TimeSpan.FromMilliseconds((int?)request["timeoutMs"] ?? 30_000);
        var watch = Stopwatch.StartNew();
        while (true)
        {
            var answer = OnUi(() => Query(request));
            if (answer["error"] is not null)
            {
                return answer;
            }
            var found = answer["elements"]!.AsArray().Count > 0;
            if (found != absent || watch.Elapsed >= timeout)
            {
                answer["ok"] = found != absent;
                answer["ms"] = (int)watch.ElapsedMilliseconds;
                return answer;
            }
            Thread.Sleep(15);
        }
    }

    /// <summary>
    /// Waits until the windows have looked the same for <c>quietMs</c>, and at least <c>floorMs</c>
    /// has passed: an action whose effect arrives after a round trip has not changed anything yet
    /// when it returns, so an early quiet proves nothing. Gives up at <c>capMs</c>.
    /// </summary>
    private static JsonObject Settle(JsonObject request)
    {
        var quiet = TimeSpan.FromMilliseconds((int?)request["quietMs"] ?? 150);
        var floor = TimeSpan.FromMilliseconds((int?)request["floorMs"] ?? 0);
        var cap = TimeSpan.FromMilliseconds((int?)request["capMs"] ?? 2_000);
        var watch = Stopwatch.StartNew();
        var changed = Stopwatch.StartNew();
        string? last = null;
        while (true)
        {
            var shape = (string)OnUi(() => new JsonObject { ["shape"] = Shape() })["shape"]!;
            if (shape != last)
            {
                last = shape;
                changed.Restart();
            }
            var settled = changed.Elapsed >= quiet && watch.Elapsed >= floor;
            if (settled || watch.Elapsed >= cap)
            {
                return new JsonObject { ["ok"] = settled, ["ms"] = (int)watch.ElapsedMilliseconds };
            }
            Thread.Sleep(15);
        }
    }

    /// <summary>What every open window shows, as one comparable string.</summary>
    private static string Shape()
    {
        var shape = new StringBuilder();
        foreach (var peer in Walk(OpenWindows().SelectMany(WindowRoots)))
        {
            shape.Append(peer.GetAutomationControlType()).Append('/').Append(peer.GetAutomationId())
                .Append('/').Append(peer.GetName()).Append(peer.IsOffscreen() ? "/off" : "").Append('\n');
        }
        return shape.ToString();
    }
}
#endif
