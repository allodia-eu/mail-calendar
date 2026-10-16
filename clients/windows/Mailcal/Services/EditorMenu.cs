// What the editor's right-click menu keeps of WebView2's own, and where its Undo and Redo come from.
//
// WebView2-free, so Mailcal.Tests holds the set; EditorWebView.cs applies it to the live menu.

using System;
using System.Collections.Generic;
using System.Text.Json;

namespace Allodia.Mailcal.Services;

/// <summary>The editor's right-click menu, by WebView2's own command names.</summary>
internal static class EditorMenu
{
    /// The items kept as WebView2 draws them. The labels are the platform's own, so they are already
    /// in the user's language.
    ///
    /// **The link item is the one that has to be here.** A link inside a quoted original cannot be
    /// clicked open in the composer (navigation is blocked), so without a way to copy its address
    /// it is text the user can see and not use.
    private static readonly HashSet<string> Kept =
        new(StringComparer.Ordinal) { "cut", "copy", "paste", "selectAll", "copyLinkLocation" };

    /// <summary>Whether the item WebView2 names <paramref name="name"/> stays as WebView2 draws it.</summary>
    /// <remarks>Anything else is not an editing action and goes (docs/composer-security.md, Gate 14).</remarks>
    internal static bool Keeps(string name) => Kept.Contains(name);

    /// <summary>The history step the item WebView2 names <paramref name="name"/> stands for, if any.</summary>
    /// <remarks>
    /// WebView2's own Undo and Redo read the engine's stack, which the editor never uses
    /// (docs/undo.md rule 3): Redo would never be offered and Undo would be offered with nothing
    /// to take back. So each is replaced, under its own label, by an item over the bundle's history.
    /// WebView2 and the bundle name the two steps alike.
    /// </remarks>
    internal static HistoryStep? Step(string name) => name switch
    {
        "undo" => HistoryStep.Undo,
        "redo" => HistoryStep.Redo,
        _ => null,
    };

    /// <summary>The script that takes <paramref name="step"/> in the bundle's history.</summary>
    internal static string Script(HistoryStep step) => $"window.stepComposerHistory('{Name(step)}')";

    /// <summary>
    /// Reads what <c>composerHistorySteps()</c> answered, as <c>ExecuteScriptAsync</c> hands it back:
    /// a JSON string holding the JSON object, into the steps there are to take. Anything else, a page
    /// that has not parsed included, offers neither.
    /// </summary>
    internal static IReadOnlySet<HistoryStep> ParseSteps(string? encoded)
    {
        var steps = new HashSet<HistoryStep>();
        try
        {
            if (encoded is null || JsonSerializer.Deserialize<string>(encoded) is not { } inner)
            {
                return steps;
            }
            using var answer = JsonDocument.Parse(inner);
            if (answer.RootElement.ValueKind != JsonValueKind.Object)
            {
                return steps;
            }
            foreach (var step in Enum.GetValues<HistoryStep>())
            {
                if (answer.RootElement.TryGetProperty(Name(step), out var value) && value.ValueKind == JsonValueKind.True)
                {
                    steps.Add(step);
                }
            }
            return steps;
        }
        catch (JsonException)
        {
            return new HashSet<HistoryStep>();
        }
    }

    private static string Name(HistoryStep step) => step == HistoryStep.Undo ? "undo" : "redo";
}
