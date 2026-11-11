// What the agent says about an element, and how a suite asks for one, in the protocol's words
// (docs/test-agent.md, "Elements" and "Filters").

using System.Text.Json.Nodes;

namespace Allodia.Mailcal.UiTests.Driver;

/// <summary>
/// An element as the agent found it. <see cref="Type"/> is its UI Automation control type (the
/// protocol's <c>role</c>) and <see cref="Class"/> its class (the protocol's <c>type</c>). The
/// rectangle is the agent's <c>screen</c>: physical screen pixels, which a press is aimed in.
/// </summary>
internal sealed record Element(
    int Handle, string Id, string Name, string Type, string Class,
    double X, double Y, double Width, double Height, bool Offscreen, bool Enabled, long Window)
{
    /// <summary>Whether it is drawn: in the tree, on screen, with a size. A collapsed element stays
    /// in the tree with an empty rectangle, and one scrolled out of view is offscreen.</summary>
    internal bool Rendered => !Offscreen && Width > 0 && Height > 0;

    internal static Element From(JsonObject e)
    {
        var screen = e["screen"]!.AsArray();
        return new(
            (int)e["h"]!, (string?)e["id"] ?? "", (string?)e["text"]?.AsArray().FirstOrDefault() ?? "",
            (string)e["role"]!, (string?)e["type"] ?? "",
            (double)screen[0]!, (double)screen[1]!, (double)screen[2]!, (double)screen[3]!,
            !(bool)e["showing"]!, (bool)e["enabled"]!, (long)e["window"]!);
    }
}

/// <summary>An element's state as a suite asserts on it. <see cref="Active"/> is a toggle's state
/// and <see cref="Expanded"/> an expander's, each null where the element has none.</summary>
internal sealed record ElementState(
    Element Element, string? Value, bool? Active, bool? Selected, bool Focused, string? Selection, bool? Expanded, string Help,
    int? FontWeight)
{
    internal static ElementState From(JsonObject e) => new(
        Element.From(e), (string?)e["value"], (bool?)e["active"], (bool?)e["selected"], (bool?)e["focused"] ?? false,
        (string?)e["selection"], (bool?)e["expanded"], (string?)e["help"] ?? "", (int?)e["fontWeight"]);
}

/// <summary>One of the app's top-level windows.</summary>
internal sealed record AppWindow(long Hwnd, string Title, string Kind);

/// <summary>
/// Which elements a lookup accepts. Every condition given has to hold. <see cref="Name"/> is one of
/// the element's texts exactly; <see cref="TextLike"/> is part of one, case ignored.
/// </summary>
/// <remarks>
/// Always give a <see cref="Type"/> for an element about to be pressed: a control's label carries
/// the same name as the control and can be pressed by nothing.
/// </remarks>
internal sealed record Filter
{
    internal string? Id { get; init; }
    internal string? Name { get; init; }
    internal string? TextLike { get; init; }

    /// <summary>The UI Automation control type: the protocol's <c>role</c>.</summary>
    internal string? Type { get; init; }
    internal Element? Under { get; init; }
    internal long? Window { get; init; }

    /// <summary>Only what is on screen. Off, a lookup reaches offscreen elements too, which is
    /// what lets a suite scroll one into view.</summary>
    internal bool OnScreen { get; init; }
    internal Filter? Has { get; init; }

    /// <summary>A state the app has marked the element with (TestAgent.Signal): a moment it knows
    /// and the screen does not show.</summary>
    internal string? Signal { get; init; }

    internal static Filter ById(string id, string? type = null) => new() { Id = id, Type = type };

    internal static Filter Named(string name, string? type = null) => new() { Name = name, Type = type };

    internal JsonObject ToJson()
    {
        var json = new JsonObject
        {
            ["id"] = Id,
            ["text"] = Name,
            ["textLike"] = TextLike,
            ["role"] = Type,
            ["under"] = Under?.Handle,
            ["window"] = Window,
            ["hidden"] = !OnScreen,
            ["has"] = Has?.ToJson(),
            ["signal"] = Signal,
        };
        foreach (var unset in json.Where(field => field.Value is null).Select(field => field.Key).ToList())
        {
            json.Remove(unset);
        }
        return json;
    }
}
