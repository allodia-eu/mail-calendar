// The call that seeds a composer's body (docs/drafts.md). A reopened message that went in as
// text opened without its formatting, pictures, quote and signature, and its next save took them
// off the draft; nothing on screen said anything had been lost.

using System.Text.Json;
using Allodia.Mailcal.Services;
using Xunit;

namespace Allodia.Mailcal.Tests;

public class ComposerBodySeedTests
{
    [Fact]
    public void A_reopened_message_is_seeded_with_its_html_and_its_text_as_data()
    {
        var script = ComposerBodySeed.Script("<p><b>Agreed</b></p>\")</script>", "Agreed");

        Assert.NotNull(script);
        Assert.StartsWith("window.setComposerBody(", script);
        var argument = script!["window.setComposerBody(".Length..^1];
        var seed = JsonDocument.Parse(JsonSerializer.Deserialize<string>(argument)!).RootElement;
        Assert.Equal("<p><b>Agreed</b></p>\")</script>", seed.GetProperty("html").GetString());
        Assert.Equal("Agreed", seed.GetProperty("text").GetString());
    }

    [Fact]
    public void A_body_with_no_html_is_seeded_as_text()
    {
        Assert.Equal("window.setPlainText(\"Hello\")", ComposerBodySeed.Script(null, "Hello"));
        Assert.Equal("window.setPlainText(\"Hello\")", ComposerBodySeed.Script("  ", "Hello"));
    }

    [Fact]
    public void An_empty_composer_sends_no_seed()
    {
        Assert.Null(ComposerBodySeed.Script(null, null));
        Assert.Null(ComposerBodySeed.Script("", ""));
    }
}
