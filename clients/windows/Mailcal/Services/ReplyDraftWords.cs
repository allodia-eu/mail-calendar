// Draft a reply's suggestions in words (docs/ai.md rules 18 and 22). Which suggestions are the
// core's; this words them. WinUI-free and L10n-free, so Mailcal.Tests can link it; the catalog's
// words arrive as a record (ComposerView.ReplyDraft.cs builds it in the app).

using uniffi.mailcal_bindings;

namespace Allodia.Mailcal.Services;

/// <summary>The catalog's <c>composer_draft_chip_*</c> words, and <c>composer_task_fill_in</c>
/// given a gap's text.</summary>
internal sealed record ReplyDraftWords(string Yes, string No, string MoreInformation, string Holding, Func<string, string> FillIn)
{
    /// <summary>A checklist item's line: a gap to fill in named as the reply carries it, any other
    /// item as the core wrote it.</summary>
    internal string Of(ReplyDraftItem item) => item.Kind == ReplyDraftTaskKind.FillIn ? FillIn(item.Text) : item.Text;

    /// <summary>The line a suggestion puts in the instruction.</summary>
    internal string Of(ReplyDraftSuggestion suggestion) => suggestion switch
    {
        ReplyDraftSuggestion.Yes => Yes,
        ReplyDraftSuggestion.No => No,
        ReplyDraftSuggestion.MoreInformation => MoreInformation,
        ReplyDraftSuggestion.Holding => Holding,
        _ => throw new ArgumentOutOfRangeException(nameof(suggestion), suggestion, null),
    };
}
