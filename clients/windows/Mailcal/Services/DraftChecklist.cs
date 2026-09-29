// A drafted reply's card (docs/ai.md, "Summary and checklist" and "Where they show"): the summary,
// the checklist in the core's order, what the person has done about it, and whether Send has asked.
// Held once per composer. WinUI-free and L10n-free so Mailcal.Tests can pin the rules, each silent
// when wrong: an item left open after its placeholder is gone, an item the editor cannot see ticked
// by it, an answer for an earlier draft ticking a later one's items, and Send asking twice.

using System;
using System.Collections.Generic;
using System.Linq;
using System.Text.Json;
using uniffi.mailcal_bindings;

namespace Allodia.Mailcal.Services;

/// <summary>One row of the checklist.</summary>
internal sealed class DraftChecklistItem
{
    internal DraftChecklistItem(int id, DraftTaskKind kind, string text)
    {
        Id = id;
        Kind = kind;
        Text = text;
    }

    /// <summary>Its place in the core's order.</summary>
    internal int Id { get; }

    /// <summary>What kind of thing it is.</summary>
    internal DraftTaskKind Kind { get; }

    /// <summary>The placeholder exactly as the reply carries it for a fill-in item, otherwise the task.</summary>
    internal string Text { get; }

    /// <summary>Whether it is done.</summary>
    internal bool Ticked { get; set; }
}

/// <summary>The card a drafted reply brings, for one composer's life.</summary>
internal sealed class DraftChecklist
{
    /// <summary>The height, in DIPs, the editor keeps before the card's body gives up any more:
    /// its toolbar, which wraps to two rows in a narrow pane, and a few lines of the reply.</summary>
    internal const double EditorFloor = 240;

    /// <summary>The card's body at its shortest: a line or two, with the rest a scroll away.</summary>
    internal const double CardBodyFloor = 56;

    /// <summary>The card's body at its tallest, however much room there is.</summary>
    internal const double CardBodyMax = 180;

    private DraftChecklistItem[] _items = [];
    private int _attachmentsAtDraft;

    /// <summary>What the message being answered asks; empty when none came.</summary>
    internal string Summary { get; private set; } = string.Empty;

    /// <summary>The items, in the core's order.</summary>
    internal IReadOnlyList<DraftChecklistItem> Items => _items;

    /// <summary>Which draft the card is for, so an answer about an earlier one is dropped.</summary>
    internal int Draft { get; private set; }

    /// <summary>Whether Send has asked about open items. Once per composer, whatever the answer,
    /// so a later draft keeps it.</summary>
    internal bool HasAsked { get; private set; }

    /// <summary>Whether there is nothing to show, and so no card.</summary>
    internal bool IsEmpty => Summary.Length == 0 && _items.Length == 0;

    /// <summary>The placeholders the editor is asked about.</summary>
    internal string[] Placeholders =>
        _items.Where(item => item.Kind == DraftTaskKind.FillIn).Select(item => item.Text).ToArray();

    /// <summary>Whether a fill-in item is still open, which is what keeps the editor being asked.</summary>
    internal bool AwaitsPlaceholders => _items.Any(item => item.Kind == DraftTaskKind.FillIn && !item.Ticked);

    /// <summary>Every item not yet done.</summary>
    internal int OpenCount => _items.Count(item => !item.Ticked);

    /// <summary>Whether Send asks before it sends. It never blocks: either answer ends the asking.</summary>
    internal bool AsksBeforeSend => !HasAsked && OpenCount > 0;

    /// <summary>Replaces the card with a draft's, in the core's order.</summary>
    /// <param name="summary">What the message asks.</param>
    /// <param name="tasks">What is left to do.</param>
    /// <param name="attachments">How many files the composer holds now.</param>
    internal void Show(string summary, IReadOnlyList<DraftTask> tasks, int attachments)
    {
        Summary = summary.Trim();
        _items = tasks.Select((task, index) => new DraftChecklistItem(index, task.Kind, task.Text)).ToArray();
        _attachmentsAtDraft = attachments;
        Draft++;
    }

    /// <summary>
    /// Takes the editor's answer about <paramref name="draft"/>: a fill-in item is ticked exactly
    /// while its placeholder is gone from the reply, so one that comes back, by an undo, opens
    /// again. Returns whether the answer was about the draft on the card.
    /// </summary>
    internal bool PlaceholdersLeft(IReadOnlyCollection<string> left, int draft)
    {
        if (draft != Draft)
        {
            return false;
        }
        foreach (var item in _items.Where(item => item.Kind == DraftTaskKind.FillIn))
        {
            item.Ticked = !left.Contains(item.Text);
        }
        return true;
    }

    /// <summary>The person's tick. A fill-in item follows the reply alone.</summary>
    internal void Toggle(int id)
    {
        if (_items.FirstOrDefault(item => item.Id == id) is { Kind: not DraftTaskKind.FillIn } item)
        {
            item.Ticked = !item.Ticked;
        }
    }

    /// <summary>
    /// Ticks every attach item once as many files have been added since the draft as there are
    /// attach items: which file answers which item cannot be told, so fewer files tick none.
    /// </summary>
    internal void AttachmentsChanged(int count)
    {
        var attach = _items.Where(item => item.Kind == DraftTaskKind.Attach).ToArray();
        if (attach.Length == 0 || count - _attachmentsAtDraft < attach.Length)
        {
            return;
        }
        foreach (var item in attach)
        {
            item.Ticked = true;
        }
    }

    /// <summary>Send has asked, and whatever the answer was, it does not ask again.</summary>
    internal void SendAsked() => HasAsked = true;

    /// <summary>
    /// How tall the card's body may be, given the height the editor and the body take between them
    /// now. The editor is the composer's one star row and the card sits under it at its own height,
    /// so in a short window an uncapped card takes the height the reply needs. The body yields down
    /// to <see cref="CardBodyFloor"/> before the editor goes under <see cref="EditorFloor"/>. The sum
    /// does not change when the cap does, since the editor takes what the body leaves, so applying
    /// the answer does not move it.
    /// </summary>
    internal static double CardBodyCeiling(double editorAndBody) =>
        Math.Clamp(editorAndBody - EditorFloor, CardBodyFloor, CardBodyMax);

    /// <summary>
    /// Whether a new card opens folded to its heading: when the editor and the body together have
    /// less than both floors, an open card would leave the reply a line or two. The person can open
    /// it, and it is then capped as <see cref="CardBodyCeiling"/> says.
    /// </summary>
    internal static bool CardOpensFolded(double editorAndBody) => editorAndBody < EditorFloor + CardBodyFloor;

    /// <summary>
    /// The editor call that answers which of <paramref name="placeholders"/> are still in the reply
    /// above the signature and the quote. The list goes in as JSON, so no placeholder's text can
    /// end the argument.
    /// </summary>
    internal static string LeftScript(IReadOnlyList<string> placeholders) =>
        $"window.composerPlaceholdersLeft({JsonSerializer.Serialize(placeholders)})";

    /// <summary>
    /// Reads <c>composerPlaceholdersLeft</c>'s answer as the WebView returns it, JSON-encoded:
    /// the placeholders still there, or <c>null</c> when the editor gave no list, so a failed read
    /// ticks nothing.
    /// </summary>
    internal static string[]? ReadLeft(string? encoded)
    {
        if (string.IsNullOrWhiteSpace(encoded))
        {
            return null;
        }
        try
        {
            using var document = JsonDocument.Parse(encoded);
            if (document.RootElement.ValueKind != JsonValueKind.Array)
            {
                return null;
            }
            var left = new List<string>();
            foreach (var element in document.RootElement.EnumerateArray())
            {
                if (element.ValueKind == JsonValueKind.String && element.GetString() is { } text)
                {
                    left.Add(text);
                }
            }
            return left.ToArray();
        }
        catch (JsonException)
        {
            return null;
        }
    }
}
