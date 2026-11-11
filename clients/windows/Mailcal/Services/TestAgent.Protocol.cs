// Reading a request, routing it to its verb, and the shape of every reply (docs/test-agent.md).
// Every verb names the arguments it takes, so a misspelt one is refused rather than ignored: a
// filter that silently dropped a field would match everything.

#if DEBUG
using System;
using System.Collections.Generic;
using System.Linq;
using System.Text.Json;
using System.Text.Json.Nodes;
using System.Threading;

namespace Allodia.Mailcal.Services;

internal static partial class TestAgent
{
    /// <summary>What a filter takes. <c>signal</c> is this agent's own: a state the app marked an
    /// element with (<see cref="Signal"/>).</summary>
    private static readonly string[] FilterArgs =
        ["text", "textLike", "id", "type", "role", "icon", "window", "under", "has", "enabled", "hidden", "index", "signal"];

    private sealed record Verb(string Name, string About, string[] Args, Func<JsonObject, JsonObject> Run);

    private static readonly Verb[] Verbs =
    [
        new("verbs", "{} -> {verbs: [{verb, about}]}: every verb and its arguments", [], _ => ListVerbs()),
        new("ping", "{} -> {pid}", [], _ => new JsonObject { ["pid"] = Environment.ProcessId }),
        new("windows", "{} -> {windows: [{h, title, kind}]}: h is the window's HWND, which `window` takes as well as a title", [],
            _ => OnUi(ListWindows)),
        new("query", "{filter} -> {elements}. text matches the accessible name or the tooltip exactly; textLike is a "
            + "case-insensitive part of one; id is the AutomationId; type is the class and matches a subclass; role is the "
            + "UI Automation control type (Button, Edit, ListItem); window is part of a title, or an HWND from windows; has "
            + "is a filter a descendant must match; hidden includes elements that are offscreen; signal is a state the app "
            + "marked the element with", FilterArgs, request => OnUi(() => Query(request))),
        new("wait", "{filter, absent?, timeoutMs=10000} -> {elements, ms}: returns the moment the filter matches, or "
            + "with absent matches nothing", [.. FilterArgs, "absent", "timeoutMs"], Wait),
        new("read", "{h} -> the element, with its value, toggle, selection, expansion and focus", ["h"],
            request => OnUi(() => Read(request))),
        new("dump", "{under?, window?, hidden?} -> {text}: the tree, one element a line, indented", ["under", "window", "hidden"],
            request => OnUi(() => Dump(request))),
        new("activate", "{h} -> {how}: Invoke where the element has it, else it is selected, as a click does", ["h"],
            request => OnUi(() => Activate(request))),
        new("set-text", "{h, text, activate?} -> {}: a text box takes it as an edit; activate then presses Return in it",
            ["h", "text", "activate"], request => OnUi(() => SetText(request))),
        new("toggle", "{h, active?} -> {active}: a check box, toggle button or switch", ["h", "active"],
            request => OnUi(() => Toggle(request))),
        new("select", "{h, index?} -> {}: the item at index in a list or picker, or with no index the element itself",
            ["h", "index"], request => OnUi(() => Select(request))),
        new("focus", "{h} -> {focused}", ["h"], request => OnUi(() => Focus(request))),
        new("settle", "{quietMs=300, capMs=10000, floorMs=0} -> {ms, settled}: waits until the windows have looked "
            + "the same for quietMs, and at least floorMs has passed", ["quietMs", "capMs", "floorMs"], Settle),
        new("screenshot", "{path, h?} -> {path, width, height}: renders h, or the mailbox, to a PNG", ["path", "h"],
            request => OnUiAsync(() => Screenshot(request))),
        new("quit", "{} -> {}: closes the mailbox, which is how a person quits the app", [], _ => AskQuit()),
        new("expand", "{h} -> {}: opens a combo box, a drop-down or a tree row", ["h"],
            request => OnUi(() => Expand(request, open: true))),
        new("collapse", "{h} -> {}: closes what expand opened", ["h"], request => OnUi(() => Expand(request, open: false))),
        new("scrollIntoView", "{h} -> {scrolled}: through the nearest scroll item at or above h", ["h"],
            request => OnUi(() => ScrollIntoView(request))),
        new("scrollTo", "{h, percent=100} -> {scrolled}: the first vertical scroller at or under h", ["h", "percent"],
            request => OnUi(() => ScrollTo(request))),
        new("focused", "{} -> {elements}: the element with the keyboard, or none", [], _ => OnUi(Focused)),
        new("parent", "{h} -> {elements}: the nearest ancestor that has a peer, or none", ["h"],
            request => OnUi(() => Parent(request))),
        new("collect", "{} -> {}: a garbage collection, for the release probe", [], _ => OnUi(Collect)),
    ];

    private static readonly Dictionary<string, Verb> ByName = Verbs.ToDictionary(verb => verb.Name);

    private static JsonObject Answer(string line)
    {
        try
        {
            var request = JsonNode.Parse(line) as JsonObject
                ?? throw new AgentException("a request is a JSON object", "bad-request");
            var verb = request["verb"] is JsonValue named && named.TryGetValue<string>(out var name)
                ? name
                : throw new AgentException("a request names its verb: {\"verb\": \"verbs\"} lists them", "bad-request");
            if (!ByName.TryGetValue(verb, out var known))
            {
                throw new AgentException($"no verb '{verb}': {{\"verb\": \"verbs\"}} lists them", "unknown-verb");
            }
            request.Remove("verb");
            Allowed(request, known.Args, verb);
            var reply = known.Run(request);
            if (!reply.ContainsKey("ok"))
            {
                reply["ok"] = true;
            }
            return reply;
        }
        catch (AgentException e)
        {
            return e.Reply();
        }
        catch (JsonException e)
        {
            return new AgentException($"not JSON: {e.Message}", "bad-request").Reply();
        }
    }

    /// <summary>Refuses an argument <paramref name="verb"/> does not take.</summary>
    private static void Allowed(JsonObject request, string[] args, string verb)
    {
        if (request.Select(field => field.Key).FirstOrDefault(key => !args.Contains(key)) is { } unknown)
        {
            throw new AgentException(
                args.Length == 0
                    ? $"bad arguments: {verb} takes none, and was given '{unknown}'"
                    : $"bad arguments: {verb} takes {string.Join(", ", args)}, and was given '{unknown}'",
                "bad-request");
        }
    }

    private static JsonObject ListVerbs() => new()
    {
        ["verbs"] = new JsonArray(Verbs.Select(verb => (JsonNode)new JsonObject { ["verb"] = verb.Name, ["about"] = verb.About }).ToArray()),
    };

    // Set by quit and acted on once its reply has been written.
    private static int _quitAsked;

    private static JsonObject AskQuit()
    {
        Interlocked.Exchange(ref _quitAsked, 1);
        return new JsonObject();
    }

    /// <summary>Closes the mailbox, which takes every other window with it and ends the app, as a
    /// person closing it does.</summary>
    private static void CloseMailbox()
    {
        if (OpenWindows().FirstOrDefault() is { } mailbox)
        {
            mailbox.Close();
        }
    }

    // Each argument read through one of these, so a wrong type is a refusal naming the argument
    // rather than an exception from the JSON reader.

    private static string? Text(JsonObject request, string field) => Typed<string>(request, field, "text");

    private static bool? Flag(JsonObject request, string field) =>
        request[field] is null ? null : Typed<bool>(request, field, "true or false");

    private static int? Number(JsonObject request, string field) =>
        request[field] is null ? null : Typed<int>(request, field, "a whole number");

    private static double? Real(JsonObject request, string field) =>
        request[field] is null ? null : Typed<double>(request, field, "a number");

    private static T? Typed<T>(JsonObject request, string field, string what)
    {
        if (request[field] is not { } node)
        {
            return default;
        }
        return node is JsonValue value && value.TryGetValue<T>(out var typed)
            ? typed
            : throw new AgentException($"bad arguments: '{field}' is {what}", "bad-request");
    }
}
#endif
