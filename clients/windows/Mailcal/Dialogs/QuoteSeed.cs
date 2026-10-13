// The words of a quoted original's attribution, in the user's language. The core hands the
// composer the message it quotes (ComposeOpening.Quote); the Rust core carries no runtime
// localisation, so, like date display, the client words it. Services/ComposeOpeningSeed.cs turns
// the words into what the editor is seeded with. The Windows twin of macOS's QuoteSeed.swift.

using Allodia.Mailcal.Services;
using Allodia.Mailcal.ViewModels;
using uniffi.mailcal_bindings;

namespace Allodia.Mailcal.Dialogs;

/// <summary>A worked example of a quoted original, for the settings dialog to render so the user can
/// see what each style looks like instead of guessing from its name. Built by
/// <see cref="ComposerQuote.Example"/> from the same catalog keys as a real quote, so the example
/// cannot drift from the real thing.</summary>
/// <param name="Line">The one-line attribution the indented style shows.</param>
/// <param name="Headers">The labelled From/Sent/To/Subject rows the line-and-header style shows.</param>
/// <param name="Body">The quoted message body.</param>
internal readonly record struct QuoteExample(
    string Line,
    IReadOnlyList<(string Label, string Value)> Headers,
    string Body);

internal static class ComposerQuote
{
    /// <summary>
    /// A quote's attribution in both styles, in the user's language, built from the message it
    /// quotes. <paramref name="isForward"/> swaps the one-line attribution for a "Forwarded
    /// message" marker; the header block is the same either way. <paramref name="dateText"/> is the
    /// original's date as the reading header shows it.
    /// </summary>
    public static QuoteWords Words(QuoteSource quote, string dateText, bool isForward)
    {
        var line = isForward ? L10n.QuoteForwarded() : L10n.QuoteAttribution(dateText, quote.From);
        var headers = new List<(string, string)>
        {
            (L10n.QuoteFrom(), quote.From),
            (L10n.QuoteSent(), dateText),
        };
        if (quote.To.Length > 0)
        {
            headers.Add((L10n.QuoteTo(), quote.To));
        }
        if (quote.Cc.Length > 0)
        {
            headers.Add((L10n.QuoteCc(), quote.Cc));
        }
        headers.Add((L10n.QuoteSubject(), quote.Subject));
        return new QuoteWords(line, headers);
    }
    /// <summary>The style token the editor's <c>setComposerQuote</c>/<c>setComposerQuoteStyle</c>
    /// expect. These are the Rust <c>QuoteStyle</c> variant names, which serialize verbatim into the
    /// seed JSON, a rename on either side has to move both (mailcal-composer pins them with a
    /// test).</summary>
    public static string Token(QuoteStyleChoice style) =>
        style == QuoteStyleChoice.LineAndHeader ? "LineAndHeader" : "Indented";

    /// <summary>Whether a composer shows its per-message style picker. Both have to hold: the
    /// message must carry a quoted original (a new message has nothing to style), and the user must
    /// have opted into per-message styling in Settings, off by default, so an ordinary reply just
    /// uses the app default and the composer stays uncluttered.</summary>
    public static bool ShowsStylePicker(bool hasQuote, bool perMessage) => hasQuote && perMessage;

    /// <summary>The sample quote the settings dialog renders under each style. Only the sender,
    /// date, subject and body are stand-ins: the attribution line and the header <em>labels</em>
    /// come from the very keys <see cref="SeedJson"/> uses, so what settings shows is what a real
    /// reply produces.</summary>
    public static QuoteExample Example()
    {
        var sender = L10n.QuotePreviewSender();
        var date = L10n.QuotePreviewDate();
        return new QuoteExample(
            L10n.QuoteAttribution(date, sender),
            new (string Label, string Value)[]
            {
                (L10n.QuoteFrom(), sender),
                (L10n.QuoteSent(), date),
                (L10n.QuoteTo(), L10n.QuotePreviewTo()),
                (L10n.QuoteSubject(), L10n.QuotePreviewSubject()),
            },
            L10n.QuotePreviewBody());
    }
}
