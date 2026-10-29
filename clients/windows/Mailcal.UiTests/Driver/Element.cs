// What the agent says about an element, and how a suite asks for one.

using System.Text.Json.Nodes;

namespace Allodia.Mailcal.UiTests.Driver;

/// <summary>An element as the agent found it. The rectangle is in physical screen pixels.</summary>
internal sealed record Element(
    int Handle, string Id, string Name, string Type, string Class,
    double X, double Y, double Width, double Height, bool Offscreen, bool Enabled, long Window)
{
    /// <summary>Whether it is drawn: in the tree, on screen, with a size. A collapsed element stays
    /// in the tree with an empty rectangle, and one scrolled out of view is offscreen.</summary>
    internal bool Rendered => !Offscreen && Width > 0 && Height > 0;

    internal static Element From(JsonObject e) => new(
        (int)e["h"]!, (string?)e["id"] ?? "", (string?)e["name"] ?? "", (string)e["type"]!, (string?)e["class"] ?? "",
        (double)e["x"]!, (double)e["y"]!, (double)e["w"]!, (double)e["ht"]!,
        (bool)e["offscreen"]!, (bool)e["enabled"]!, (long)e["window"]!);
}

/// <summary>An element's state as a suite asserts on it.</summary>
internal sealed record ElementState(
    Element Element, string? Value, string? Toggle, bool? Selected, bool Focused, string? Selection, string? Expanded, string Help,
    int? FontWeight)
{
    internal static ElementState From(JsonObject e) => new(
        Element.From(e), (string?)e["value"], (string?)e["toggle"], (bool?)e["selected"], (bool?)e["focused"] ?? false,
        (string?)e["selection"], (string?)e["expanded"], (string?)e["help"] ?? "", (int?)e["fontWeight"]);
}

/// <summary>One of the app's top-level windows.</summary>
internal sealed record AppWindow(long Hwnd, string Title, string Kind);

/// <summary>
/// Which elements a lookup accepts. Every condition given has to hold. A name pattern reads as
/// PowerShell's <c>-like</c> does: <c>*</c> and <c>?</c>, case ignored.
/// </summary>
/// <remarks>
/// Always give a <see cref="Type"/> for an element about to be pressed: a control's label carries
/// the same name as the control and can be pressed by nothing.
/// </remarks>
internal sealed record Filter
{
    internal string? Id { get; init; }
    internal string? Name { get; init; }
    internal string? NameLike { get; init; }
    internal string? Type { get; init; }
    internal Element? Under { get; init; }
    internal long? Window { get; init; }
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
            ["name"] = Name,
            ["nameLike"] = NameLike,
            ["type"] = Type,
            ["under"] = Under?.Handle,
            ["window"] = Window,
            ["onScreen"] = OnScreen,
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
