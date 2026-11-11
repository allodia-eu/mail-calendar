// How the agent describes an element (docs/test-agent.md, "Elements"), and what a lookup that found
// nothing might have meant.

#if DEBUG
using System;
using System.Collections.Generic;
using System.Linq;
using System.Text.Json.Nodes;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Automation.Peers;
using Microsoft.UI.Xaml.Controls;

namespace Allodia.Mailcal.Services;

internal static partial class TestAgent
{
    /// <summary>
    /// What a caller reads off an element. <c>bounds</c> is in the logical pixels of its window, as
    /// the protocol has it; <c>screen</c> is the same rectangle in the physical screen pixels a press
    /// is aimed in, which is this agent's own, and <c>window</c> is the HWND it is drawn in.
    /// </summary>
    private static JsonObject Describe(AutomationPeer peer, bool detail = false)
    {
        // A peer measures from its window's client area, in physical pixels.
        var rect = peer.GetBoundingRectangle();
        var origin = new ClientPoint();
        var window = WindowFor(peer);
        var scale = 1.0;
        if (window is not null)
        {
            ClientToScreen((nint)Hwnd(window), ref origin);
            scale = window.Content?.XamlRoot?.RasterizationScale ?? 1.0;
        }
        var described = new JsonObject
        {
            ["h"] = HandleFor(peer),
            ["type"] = peer.GetClassName(),
            ["role"] = peer.GetAutomationControlType().ToString(),
            ["showing"] = !peer.IsOffscreen(),
            ["enabled"] = peer.IsEnabled(),
            ["screen"] = new JsonArray(rect.X + origin.X, rect.Y + origin.Y, rect.Width, rect.Height),
            ["window"] = window is null ? 0 : Hwnd(window),
        };
        if (peer.GetName() is { Length: > 0 } name)
        {
            described["text"] = new JsonArray(name);
        }
        if (peer.GetAutomationId() is { Length: > 0 } id)
        {
            described["id"] = id;
        }
        if (Tooltip(peer) is { } tooltip)
        {
            described["tooltip"] = tooltip;
        }
        if (window is not null && rect.Width * rect.Height > 0)
        {
            described["bounds"] = new JsonArray(rect.X / scale, rect.Y / scale, rect.Width / scale, rect.Height / scale);
        }
        if (SignalOf(peer) is { } signal)
        {
            described["signal"] = signal;
        }
        if (detail)
        {
            Detail(peer, described);
        }
        return described;
    }

    /// <summary>What a person reads on the element, which <c>text</c> and <c>textLike</c> match: its
    /// accessible name, and its tooltip where it has one of words.</summary>
    private static IEnumerable<string> Said(AutomationPeer peer)
    {
        if (peer.GetName() is { Length: > 0 } name)
        {
            yield return name;
        }
        if (Tooltip(peer) is { } tooltip)
        {
            yield return tooltip;
        }
    }

    private static string? Tooltip(AutomationPeer peer) =>
        peer is FrameworkElementAutomationPeer { Owner: { } owner }
        && ToolTipService.GetToolTip(owner) is string { Length: > 0 } tooltip
            ? tooltip
            : null;

    /// <summary>Whether the element is of class <paramref name="type"/>, or of a class derived from
    /// it: the class UI Automation reports, then the element's own type and its bases.</summary>
    private static bool IsOfClass(AutomationPeer peer, string type)
    {
        if (peer.GetClassName() == type)
        {
            return true;
        }
        for (var at = (peer as FrameworkElementAutomationPeer)?.Owner?.GetType(); at is not null; at = at.BaseType)
        {
            if (at.Name == type)
            {
                return true;
            }
        }
        return false;
    }

    /// <summary>What a lookup that found nothing might have meant: elements on screen whose text
    /// shares a word with the one asked for. The common miss is a label that changed, and this names
    /// the new one.</summary>
    private static JsonArray Near(JsonObject request)
    {
        var wanted = Text(request, "text") ?? Text(request, "textLike");
        var words = (wanted ?? "").ToLowerInvariant()
            .Split(' ', StringSplitOptions.RemoveEmptyEntries)
            .Where(word => word.Length > 2)
            .ToList();
        var near = new JsonArray();
        if (words.Count == 0)
        {
            return near;
        }
        var scope = request.DeepClone().AsObject();
        foreach (var field in scope.Select(field => field.Key).Where(key => key is not ("under" or "window")).ToList())
        {
            scope.Remove(field);
        }
        foreach (var peer in Walk(RootsOf(scope)))
        {
            if (near.Count >= 8)
            {
                break;
            }
            if (!peer.IsOffscreen() && Said(peer).Any(said => words.Any(word => said.Contains(word, StringComparison.OrdinalIgnoreCase))))
            {
                near.Add($"[{HandleFor(peer)}] {peer.GetAutomationControlType()} \"{peer.GetName()}\" #{peer.GetAutomationId()}");
            }
        }
        return near;
    }
}
#endif
