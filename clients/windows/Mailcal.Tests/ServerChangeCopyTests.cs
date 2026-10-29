// The question a calendar or address book server found on another site asks
// (docs/account-autodetect.md rule 14): which sentence each change earns, and where each site sits
// in its line.

using Allodia.Mailcal.Services;
using uniffi.mailcal_bindings;
using Xunit;

namespace Allodia.Mailcal.Tests;

public class ServerChangeCopyTests
{
    // Words that say which key they stand for, so a test reads which sentence went where.
    private static readonly ServerChangeWords Words = new(
        uses => $"server_changed_title_{Key(uses)}",
        (uses, address) => $"server_changed_body_{Key(uses)}({address})",
        site => $"Before: {site}",
        site => $"Now: {site}",
        "server_changed_advice",
        uses => $"server_changed_unsure_{Key(uses)}");

    private static string Key(ServerChangeUses uses) => uses.ToString().ToLowerInvariant();

    [Theory]
    [InlineData(true, false, "calendar")]
    [InlineData(false, true, "contacts")]
    [InlineData(true, true, "both")]
    public void TheTitleBodyAndClosingLineFollowWhatTheChangeMoves(bool calendar, bool contacts, string key)
    {
        var copy = ServerChangeCopy.Of(
            new ServerChange("example.org", "example.net", calendar, contacts), "alice@example.org", Words);

        Assert.Equal($"server_changed_title_{key}", copy.Title);
        Assert.Equal($"server_changed_body_{key}(alice@example.org)", copy.Body);
        Assert.Equal($"server_changed_unsure_{key}", copy.Unsure);
        Assert.Equal("server_changed_advice", copy.Advice);
    }

    [Fact]
    public void BeforeNamesThePreviousSiteAndNowTheProposedOne()
    {
        var copy = ServerChangeCopy.Of(
            new ServerChange("example.org", "example.net", true, false), "alice@example.org", Words);

        Assert.Equal(new SiteLine("Before: ", "example.org", string.Empty), copy.Before);
        Assert.Equal(new SiteLine("Now: ", "example.net", string.Empty), copy.Now);
    }

    [Fact]
    public void TheSiteIsSplitOutWhereverTheTranslationPutsIt()
    {
        Assert.Equal(new SiteLine("Vorher (", "example.org", ")"), ServerChangeCopy.Around(site => $"Vorher ({site})", "example.org"));
        // A translation whose own words contain the site's text still splits at the placeholder.
        Assert.Equal(new SiteLine("org: ", "org", string.Empty), ServerChangeCopy.Around(site => $"org: {site}", "org"));
        // One without the placeholder keeps the site, after its words.
        Assert.Equal(new SiteLine("Before ", "example.org", string.Empty), ServerChangeCopy.Around(_ => "Before", "example.org"));
    }
}
