// Builds the seed for the composer's quoted original on reply/forward. The quoted body is the
// reading view's already-sanitised HTML (and plain text) for the open message; the attribution
// is localised here, the Rust core carries no runtime localisation, so, like date display, the
// client formats it (L10n + the date already localised onto OpenedMessage). The shape matches the
// Rust composer's Block::Quote so it round-trips through the shared editor; the core re-sanitizes
// the body on submit (docs/composer-security.md, Gate 10). The Windows twin of macOS's QuoteSeed.swift.

using System.Text.Json.Nodes;
using Allodia.Mailcal.ViewModels;

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
    /// The seed JSON for <c>window.setComposerQuote</c>, or <c>null</c> when there is nothing to
    /// quote yet (the body hasn't loaded for this message). <paramref name="isForward"/> swaps the
    /// one-line attribution for a "Forwarded message" marker; the header block is the
    /// same either way. <paramref name="initialText"/> pre-fills the paragraph above the quote; only
    /// showcase mode passes it, and the editor assigns it as text, never markup (Gate 11).
    /// </summary>
    public static string? SeedJson(
        QuoteStyleChoice style,
        OpenedMessage message,
        ReadingBody? reading,
        bool isForward,
        string? initialText = null)
    {
        if (reading is null || reading.Key != message.Key)
        {
            return null;
        }
        var bodyHtml = reading.Html ?? string.Empty;
        var bodyPlain = reading.Plain ?? string.Empty;
        if (bodyHtml.Length == 0 && bodyPlain.Length == 0)
        {
            return null;
        }

        var line = isForward
            ? L10n.QuoteForwarded()
            : L10n.QuoteAttribution(message.DateText, message.From);

        var headers = new JsonArray
        {
            Header(L10n.QuoteFrom(), message.From),
            Header(L10n.QuoteSent(), message.DateText),
        };
        if (reading.To.Length > 0)
        {
            headers.Add(Header(L10n.QuoteTo(), reading.To));
        }
        if (reading.Cc.Length > 0)
        {
            headers.Add(Header(L10n.QuoteCc(), reading.Cc));
        }
        headers.Add(Header(L10n.QuoteSubject(), message.Subject));

        var payload = new JsonObject
        {
            ["style"] = Token(style),
            ["attribution"] = new JsonObject { ["line"] = line, ["headers"] = headers },
            ["body_html"] = bodyHtml,
            ["body_plain"] = bodyPlain,
        };
        if (!string.IsNullOrEmpty(initialText))
        {
            payload["initial_text"] = initialText;
        }
        return payload.ToJsonString();
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

    private static JsonObject Header(string label, string value) => new() { ["label"] = label, ["value"] = value };
}
