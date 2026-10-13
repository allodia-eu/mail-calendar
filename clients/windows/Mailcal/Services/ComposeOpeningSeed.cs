// How a composer opens from the core's ComposeOpening, as plain rules: which kind it is, what the
// editor is seeded with, and the quote's attribution. WinUI- and L10n-free so Mailcal.Tests can link
// it; the localised words are passed in (Dialogs/QuoteSeed.cs builds them).

using System.Text.Json;
using System.Text.Json.Nodes;
using Allodia.Mailcal.ViewModels;
using uniffi.mailcal_bindings;

namespace Allodia.Mailcal.Services;

/// <summary>A quote's attribution in both styles, in the user's language: the one-line "On …
/// wrote:" and the labelled header block.</summary>
internal sealed record QuoteWords(string Line, IReadOnlyList<(string Label, string Value)> Headers);

internal static class ComposeOpeningSeed
{
    /// <summary>The composer kind the core's answer names.</summary>
    internal static RichComposeKind KindOf(ComposeKind kind) => kind switch
    {
        ComposeKind.Reply => RichComposeKind.Reply,
        ComposeKind.ReplyAll => RichComposeKind.ReplyAll,
        ComposeKind.Forward => RichComposeKind.Forward,
        _ => RichComposeKind.New,
    };

    /// <summary>The seed for <c>setComposerQuote</c>: a fresh reply's or forward's quoted original,
    /// with <paramref name="initialText"/> above it (what a prefill brought), or <c>null</c> when
    /// the composer does not open that way. A stored message opens through its own HTML instead,
    /// quote included.</summary>
    internal static string? QuoteSeed(ComposeOpening opening, QuoteWords? words, string styleToken, string? initialText)
    {
        if (opening.BodyHtml.Length > 0 || opening.Quote is not { } quote || words is null)
        {
            return null;
        }
        if (quote.BodyHtml.Length == 0 && quote.BodyPlain.Length == 0)
        {
            return null;
        }
        var payload = new JsonObject
        {
            ["style"] = styleToken,
            ["attribution"] = Attribution(words),
            ["body_html"] = quote.BodyHtml,
            ["body_plain"] = quote.BodyPlain,
        };
        if (!string.IsNullOrEmpty(initialText))
        {
            payload["initial_text"] = initialText;
        }
        return payload.ToJsonString();
    }

    /// <summary>The argument for <c>completeComposerQuoteAttribution</c>: a reopened message's quote
    /// is missing the attribution of the style it was not saved in, and the original says what it
    /// is. <c>null</c> for anything else.</summary>
    internal static string? AttributionCompletion(ComposeOpening opening, QuoteWords? words) =>
        opening.BodyHtml.Length > 0 && opening.Quote is not null && words is not null
            ? Attribution(words).ToJsonString()
            : null;

    /// <summary>The text the editor opens with when it is seeded neither with a stored body nor
    /// with a quote: a link's, a share's or an assistant's.</summary>
    internal static string? PlainBody(ComposeOpening opening, string? quoteSeed) =>
        opening.BodyHtml.Length == 0 && quoteSeed is null && opening.BodyText.Length > 0
            ? opening.BodyText
            : null;

    /// <summary>The style of the quote a composer's document holds, as the editor's token
    /// (<c>Indented</c> or <c>LineAndHeader</c>), or <c>null</c> when it holds none: what a reopened
    /// composer's style picker opens on.</summary>
    internal static string? QuoteStyleOf(string? documentJson)
    {
        if (string.IsNullOrEmpty(documentJson))
        {
            return null;
        }
        try
        {
            using var document = JsonDocument.Parse(documentJson);
            foreach (var block in document.RootElement.GetProperty("blocks").EnumerateArray())
            {
                if (block.TryGetProperty("Quote", out var quote)
                    && quote.TryGetProperty("style", out var style))
                {
                    return style.GetString();
                }
            }
        }
        catch (JsonException)
        {
        }
        catch (KeyNotFoundException)
        {
        }
        return null;
    }

    private static JsonObject Attribution(QuoteWords words)
    {
        var headers = new JsonArray();
        foreach (var (label, value) in words.Headers)
        {
            headers.Add(new JsonObject { ["label"] = label, ["value"] = value });
        }
        return new JsonObject { ["line"] = words.Line, ["headers"] = headers };
    }
}
