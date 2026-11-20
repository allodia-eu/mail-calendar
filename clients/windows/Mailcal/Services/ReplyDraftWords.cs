// Draft a reply's suggestions in words, and where each failure's remedy takes the person from the
// composer (docs/ai.md rules 18 and 22). Which suggestions and which remedy are the core's; this
// words and routes them. WinUI-free and L10n-free, so Mailcal.Tests can link it; the catalog's words
// arrive as a record (ComposerView.ReplyDraft.cs builds it in the app).

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

/// <summary>Where a remedy is taken from the composer, which keeps the draft either way.</summary>
internal enum ReplyDraftRemedyPlace
{
    /// <summary>The instruction's input again, in place.</summary>
    Retry,

    /// <summary>The Allodia sign-in, over the composer.</summary>
    SignIn,

    /// <summary>Settings → Advanced, where the own endpoint is set up.</summary>
    Endpoint,

    /// <summary>Settings → Writing style, where an account's style is chosen.</summary>
    WritingStyle,

    /// <summary>The learning flow, opened in Settings → Writing style.</summary>
    Learn,
}

internal static class ReplyDraftRemedies
{
    internal static ReplyDraftRemedyPlace PlaceOf(AiRemedy remedy) => remedy switch
    {
        AiRemedy.TryAgain => ReplyDraftRemedyPlace.Retry,
        AiRemedy.SignInToAllodia => ReplyDraftRemedyPlace.SignIn,
        AiRemedy.OpenAiEndpoint => ReplyDraftRemedyPlace.Endpoint,
        AiRemedy.LearnStyle => ReplyDraftRemedyPlace.Learn,
        AiRemedy.ChooseStyle or AiRemedy.ChooseAccount or AiRemedy.ChooseRange => ReplyDraftRemedyPlace.WritingStyle,
        _ => throw new ArgumentOutOfRangeException(nameof(remedy), remedy, null),
    };
}
