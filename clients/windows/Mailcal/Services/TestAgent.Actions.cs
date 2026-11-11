// What the agent does to an element, through the same providers UI Automation calls. Each acts only
// where a person could: a disabled control, a read-only field and a control without the action are
// refusals with a code, never a silent success.

#if DEBUG
using System;
using System.Linq;
using System.Runtime.InteropServices;
using System.Text.Json.Nodes;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Automation;
using Microsoft.UI.Xaml.Automation.Peers;
using Microsoft.UI.Xaml.Automation.Provider;
using Microsoft.UI.Xaml.Controls;

namespace Allodia.Mailcal.Services;

internal static partial class TestAgent
{
    /// <summary>The state a caller asserts on, where the element has it.</summary>
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
            described["readOnly"] = text.ReadOnly;
        }
        if (peer.GetPattern(PatternInterface.Toggle) is IToggleProvider toggle && toggle.ToggleState != ToggleState.Indeterminate)
        {
            described["active"] = toggle.ToggleState == ToggleState.On;
        }
        if (peer.GetPattern(PatternInterface.SelectionItem) is ISelectionItemProvider item)
        {
            described["selected"] = item.IsSelected;
        }
        if (peer.GetPattern(PatternInterface.ExpandCollapse) is IExpandCollapseProvider expand
            && expand.ExpandCollapseState != ExpandCollapseState.LeafNode)
        {
            described["expanded"] = expand.ExpandCollapseState != ExpandCollapseState.Collapsed;
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

    /// <summary>The element a request names, refused when a person could not act on it.</summary>
    private static AutomationPeer Usable(JsonObject request)
    {
        var peer = PeerOf(HandleIn(request));
        if (!peer.IsEnabled())
        {
            throw new AgentException($"'{peer.GetName()}' is disabled", "disabled", new JsonObject { ["element"] = Describe(peer) });
        }
        return peer;
    }

    /// <summary>What a click does: Invoke where the element has it, else it is selected. A list row
    /// opens on its item click, which only Invoke raises, so Invoke is tried first.</summary>
    private static JsonObject Activate(JsonObject request)
    {
        var peer = Usable(request);
        if (peer.GetPattern(PatternInterface.Invoke) is IInvokeProvider invoke)
        {
            invoke.Invoke();
            return new JsonObject { ["how"] = "invoked" };
        }
        if (peer.GetPattern(PatternInterface.SelectionItem) is ISelectionItemProvider item)
        {
            item.Select();
            return new JsonObject { ["how"] = "selected" };
        }
        throw new AgentException($"'{peer.GetName()}' can be neither invoked nor selected; is it a label rather than a control?", "not-activatable");
    }

    /// <summary>The item at <c>index</c> of a list or picker, or with no index the element itself.</summary>
    private static JsonObject Select(JsonObject request)
    {
        var peer = Usable(request);
        if (Number(request, "index") is { } index)
        {
            var list = (peer as FrameworkElementAutomationPeer)?.Owner as Microsoft.UI.Xaml.Controls.Primitives.Selector
                ?? throw new AgentException($"'{peer.GetName()}' holds no items to choose from", "not-selectable");
            if (index < 0 || index >= list.Items.Count)
            {
                throw new AgentException($"'{peer.GetName()}' has {list.Items.Count} items, so none at {index}", "no-element");
            }
            list.SelectedIndex = index;
            return new JsonObject();
        }
        var item = peer.GetPattern(PatternInterface.SelectionItem) as ISelectionItemProvider
            ?? throw new AgentException($"'{peer.GetName()}' cannot be selected", "not-selectable");
        item.Select();
        return new JsonObject();
    }

    /// <summary>Sets a text box's text the way UI Automation does, which the box takes as an edit;
    /// with <c>activate</c>, then presses Return in it, as a person submitting it does.</summary>
    private static JsonObject SetText(JsonObject request)
    {
        var peer = Usable(request);
        var text = Text(request, "text") ?? throw new AgentException("bad arguments: set-text needs 'text'", "bad-request");
        if (peer.GetPattern(PatternInterface.Value) is IValueProvider value)
        {
            if (value.IsReadOnly)
            {
                throw new AgentException($"'{peer.GetName()}' is read-only", "not-editable");
            }
            value.SetValue(text);
        }
        else if (TextOwner(peer) is { ReadOnly: false } owner)
        {
            owner.Set(text);
        }
        else
        {
            throw new AgentException($"'{peer.GetName()}' takes no text", "not-editable");
        }
        if (Flag(request, "activate") == true)
        {
            peer.SetFocus();
            PressReturn();
        }
        return new JsonObject();
    }

    /// <summary>
    /// The text a box holds, for the boxes whose value UI Automation reads from the framework rather
    /// than from the peer: in-process, their peer offers no value provider, so the box is read and
    /// set directly, which is what UI Automation's value pattern does to it.
    /// </summary>
    private static (Func<string> Get, Action<string> Set, bool ReadOnly)? TextOwner(AutomationPeer peer) =>
        (peer as FrameworkElementAutomationPeer)?.Owner switch
        {
            TextBox box => (() => box.Text, value => box.Text = value, box.IsReadOnly),
            PasswordBox box => (() => box.Password, value => box.Password = value, false),
            _ => null,
        };

    /// <summary>Toggles a check box, toggle button or switch, or with <c>active</c> puts it there
    /// whichever way it was.</summary>
    private static JsonObject Toggle(JsonObject request)
    {
        var peer = Usable(request);
        var toggle = peer.GetPattern(PatternInterface.Toggle) as IToggleProvider
            ?? throw new AgentException($"'{peer.GetName()}' does not toggle", "not-toggleable");
        var wanted = Flag(request, "active");
        // A three-state box passes through its third on the way round.
        for (var turns = 0; turns < 3; turns++)
        {
            if (wanted is null ? turns == 1 : (toggle.ToggleState == ToggleState.On) == wanted)
            {
                break;
            }
            toggle.Toggle();
        }
        return new JsonObject { ["active"] = toggle.ToggleState == ToggleState.On };
    }

    /// <summary>Opens a combo box or a drop-down, whose items have no peers until it is open, or
    /// with <paramref name="open"/> false closes it again.</summary>
    private static JsonObject Expand(JsonObject request, bool open)
    {
        var peer = Usable(request);
        var expand = peer.GetPattern(PatternInterface.ExpandCollapse) as IExpandCollapseProvider
            ?? throw new AgentException($"'{peer.GetName()}' does not expand", "not-expandable");
        if (open)
        {
            expand.Expand();
        }
        else
        {
            expand.Collapse();
        }
        return new JsonObject();
    }

    /// <summary>Scrolls the element into its list's view, through the nearest scroll item: the
    /// element a caller presses is often a text inside a row.</summary>
    private static JsonObject ScrollIntoView(JsonObject request)
    {
        for (AutomationPeer? at = PeerOf(HandleIn(request)); at is not null; at = ParentPeer(at))
        {
            if (at.GetPattern(PatternInterface.ScrollItem) is IScrollItemProvider scroll)
            {
                scroll.ScrollIntoView();
                return new JsonObject { ["scrolled"] = true };
            }
        }
        return new JsonObject { ["scrolled"] = false };
    }

    private static JsonObject Focus(JsonObject request)
    {
        var peer = PeerOf(HandleIn(request));
        peer.SetFocus();
        return new JsonObject { ["focused"] = peer.HasKeyboardFocus() };
    }

    /// <summary>The nearest ancestor with a peer of an element the walk has met, or none: a layout
    /// panel between them has no peer, so the answer can be several levels up.</summary>
    private static JsonObject Parent(JsonObject request)
    {
        var parent = ParentPeer(PeerOf(HandleIn(request)));
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
        var percent = Real(request, "percent") ?? 100;
        var root = PeerOf(HandleIn(request));
        var window = WindowFor(root) ?? OpenWindows()[0];
        foreach (var peer in Walk([(root, window)]))
        {
            if (peer.GetPattern(PatternInterface.Scroll) is IScrollProvider { VerticallyScrollable: true } scroll)
            {
                scroll.SetScrollPercent(-1, percent);
                return new JsonObject { ["scrolled"] = true };
            }
        }
        return new JsonObject { ["scrolled"] = false };
    }

    /// <summary>Return, pressed where the keyboard is, as the person would after typing.</summary>
    private static void PressReturn()
    {
        const ushort Return = 0x0D;
        const uint KeyUp = 0x0002;
        Input[] keys =
        [
            new() { Type = 1, Key = new KeyInput { VirtualKey = Return } },
            new() { Type = 1, Key = new KeyInput { VirtualKey = Return, Flags = KeyUp } },
        ];
        SendInput((uint)keys.Length, keys, Marshal.SizeOf<Input>());
    }

    [StructLayout(LayoutKind.Sequential)]
    private struct KeyInput
    {
        public ushort VirtualKey;
        public ushort Scan;
        public uint Flags;
        public uint Time;
        public nint ExtraInfo;
    }

    // INPUT's union is as wide as its mouse member, which the padding stands in for.
    [StructLayout(LayoutKind.Sequential)]
    private struct Input
    {
        public uint Type;
        public KeyInput Key;
        public long Padding;
    }

    [DllImport("user32.dll", SetLastError = true)]
    private static extern uint SendInput(uint count, Input[] inputs, int size);
}
#endif
