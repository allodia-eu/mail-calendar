// How a composer opens from the core's answer (docs/drafts.md): a fresh reply quotes its original,
// a reopened message opens through its own HTML with its quote's attribution completed, and a
// prefill's text opens as text. One answer, whichever way the composer was reached.

using System.Text.Json;
using Allodia.Mailcal.Services;
using Allodia.Mailcal.ViewModels;
using uniffi.mailcal_bindings;
using Xunit;

namespace Allodia.Mailcal.Tests;

public class ComposeOpeningSeedTests
{
    private static readonly QuoteSource Original = new(
        From: "Bob <bob@example.test>",
        Date: "2026-03-01T09:30:00Z",
        To: "Ada <ada@example.test>",
        Cc: string.Empty,
        Subject: "Plans",
        BodyHtml: "<p>Original</p>",
        BodyPlain: "Original");

    private static readonly QuoteWords Words = new(
        "On 1 March, Bob wrote:",
        [("From", "Bob <bob@example.test>"), ("Subject", "Plans")]);

    private static ComposeOpening Opening(
        ComposeKind kind = ComposeKind.Reply,
        string bodyHtml = "",
        string bodyText = "",
        QuoteSource? quote = null) =>
        new(
            Composition: "c1",
            Kind: kind,
            Account: "acct-1",
            To: "bob@example.test",
            Cc: string.Empty,
            Bcc: string.Empty,
            Subject: "Re: Plans",
            BodyHtml: bodyHtml,
            BodyText: bodyText,
            Quote: quote,
            Attachments: [],
            AttachmentsFailed: false,
            SeedsSignature: bodyHtml.Length == 0);

    [Fact]
    public void Each_kind_the_core_names_opens_the_composer_of_that_kind()
    {
        Assert.Equal(RichComposeKind.New, ComposeOpeningSeed.KindOf(ComposeKind.New));
        Assert.Equal(RichComposeKind.Reply, ComposeOpeningSeed.KindOf(ComposeKind.Reply));
        Assert.Equal(RichComposeKind.ReplyAll, ComposeOpeningSeed.KindOf(ComposeKind.ReplyAll));
        Assert.Equal(RichComposeKind.Forward, ComposeOpeningSeed.KindOf(ComposeKind.Forward));
    }

    [Fact]
    public void A_fresh_reply_quotes_its_original_with_both_attributions_and_the_prefill_above()
    {
        var seed = ComposeOpeningSeed.QuoteSeed(Opening(quote: Original), Words, "Indented", "Thursday works.");

        var quote = JsonDocument.Parse(seed!).RootElement;
        Assert.Equal("Indented", quote.GetProperty("style").GetString());
        Assert.Equal("On 1 March, Bob wrote:", quote.GetProperty("attribution").GetProperty("line").GetString());
        Assert.Equal(2, quote.GetProperty("attribution").GetProperty("headers").GetArrayLength());
        Assert.Equal("<p>Original</p>", quote.GetProperty("body_html").GetString());
        Assert.Equal("Thursday works.", quote.GetProperty("initial_text").GetString());
        Assert.Null(ComposeOpeningSeed.AttributionCompletion(Opening(quote: Original), Words));
    }

    [Fact]
    public void A_reopened_message_opens_through_its_own_html_and_completes_its_quote()
    {
        var opening = Opening(bodyHtml: "<p>Saved</p>", bodyText: "Saved", quote: Original);

        Assert.Null(ComposeOpeningSeed.QuoteSeed(opening, Words, "Indented", opening.BodyText));
        var completion = JsonDocument.Parse(ComposeOpeningSeed.AttributionCompletion(opening, Words)!).RootElement;
        Assert.Equal("On 1 March, Bob wrote:", completion.GetProperty("line").GetString());
        Assert.Equal("Subject", completion.GetProperty("headers")[1].GetProperty("label").GetString());
        Assert.Null(ComposeOpeningSeed.PlainBody(opening, null));
    }

    [Fact]
    public void A_prefill_with_nothing_to_quote_opens_as_text()
    {
        var opening = Opening(kind: ComposeKind.New, bodyText: "From a link");

        Assert.Null(ComposeOpeningSeed.QuoteSeed(opening, null, "Indented", opening.BodyText));
        Assert.Equal("From a link", ComposeOpeningSeed.PlainBody(opening, null));
        Assert.Null(ComposeOpeningSeed.PlainBody(Opening(kind: ComposeKind.New), null));
    }

    [Fact]
    public void A_reopened_composers_style_picker_reads_the_style_its_quote_was_saved_in()
    {
        const string document =
            "{\"blocks\":[{\"Paragraph\":{\"content\":[]}},{\"Quote\":{\"style\":\"LineAndHeader\"," +
            "\"attribution\":{\"line\":\"\",\"headers\":[]},\"body_html\":\"\",\"body_plain\":\"\"}}],\"attachments\":[]}";

        Assert.Equal("LineAndHeader", ComposeOpeningSeed.QuoteStyleOf(document));
        Assert.Null(ComposeOpeningSeed.QuoteStyleOf("{\"blocks\":[],\"attachments\":[]}"));
        Assert.Null(ComposeOpeningSeed.QuoteStyleOf("not json"));
    }
}
