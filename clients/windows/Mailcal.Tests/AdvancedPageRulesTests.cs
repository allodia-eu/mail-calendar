// Settings → Advanced drawn from the core's AdvancedPage: the words for each server status and
// reset outcome, and which changes redraw a part of the page rather than move its switches in place.

using Allodia.Mailcal.Services;
using uniffi.mailcal_bindings;
using Xunit;

namespace Allodia.Mailcal.Tests;

public class AdvancedPageRulesTests
{
    private static readonly McpStatusWords Status = new("off", "starting", "listening", "another", "unavailable");
    private static readonly ResetOutcomeWords Outcome = new("done", "failed");

    private static McpPanel Panel(bool enabled = true, params (string Id, bool Exposed)[] accounts) => new(
        enabled,
        McpStatus.Listening,
        accounts.Select(account => new McpAccountRow(account.Id, account.Id + "@test.local", account.Exposed)).ToArray(),
        !accounts.Any(account => account.Exposed),
        false,
        true,
        false,
        "{\"mcpServers\":{}}");

    private static AdvancedPage Page(ControlState reset, ResetOutcome? outcome) => new(null, reset, outcome);

    [Fact]
    public void EachStatusIsSaidInItsOwnWords()
    {
        Assert.Equal("off", AdvancedPageRules.StatusText(McpStatus.Off, Status));
        Assert.Equal("starting", AdvancedPageRules.StatusText(McpStatus.Starting, Status));
        Assert.Equal("listening", AdvancedPageRules.StatusText(McpStatus.Listening, Status));
        Assert.Equal("another", AdvancedPageRules.StatusText(McpStatus.AnotherInstance, Status));
        Assert.Equal("unavailable", AdvancedPageRules.StatusText(McpStatus.Unavailable, Status));
    }

    [Fact]
    public void OnlyAServerNoAssistantCanReachIsSaidAtFullStrength()
    {
        Assert.True(AdvancedPageRules.StatusNeedsAttention(McpStatus.AnotherInstance));
        Assert.True(AdvancedPageRules.StatusNeedsAttention(McpStatus.Unavailable));
        Assert.False(AdvancedPageRules.StatusNeedsAttention(McpStatus.Off));
        Assert.False(AdvancedPageRules.StatusNeedsAttention(McpStatus.Starting));
        Assert.False(AdvancedPageRules.StatusNeedsAttention(McpStatus.Listening));
    }

    [Fact]
    public void AResetSaysWhatItCameToAndNothingBeforeOneEnds()
    {
        Assert.Equal("done", AdvancedPageRules.OutcomeText(ResetOutcome.Done, Outcome));
        Assert.Equal("failed", AdvancedPageRules.OutcomeText(ResetOutcome.Failed, Outcome));
        Assert.Null(AdvancedPageRules.OutcomeText(null, Outcome));
    }

    [Fact]
    public void ASwitchOrABoxMovingIsDrawnInPlace()
    {
        var drawn = AdvancedPageRules.GrantsFingerprint(Panel(true, ("work", false)));
        Assert.Equal(drawn, AdvancedPageRules.GrantsFingerprint(Panel(true, ("work", true))));
        Assert.Equal(
            drawn,
            AdvancedPageRules.GrantsFingerprint(
                Panel(true, ("work", false)) with { AllowDirectSend = true, KnownRecipientEditable = true, Status = McpStatus.Unavailable }));
    }

    [Fact]
    public void TheGroupsAreDrawnAgainWhenTheyAppearOrWhatTheyListMoves()
    {
        var drawn = AdvancedPageRules.GrantsFingerprint(Panel(true, ("work", false)));
        Assert.NotEqual(drawn, AdvancedPageRules.GrantsFingerprint(Panel(false, ("work", false))));
        Assert.NotEqual(drawn, AdvancedPageRules.GrantsFingerprint(Panel(true, ("work", false), ("home", false))));
        Assert.NotEqual(
            drawn, AdvancedPageRules.GrantsFingerprint(Panel(true, ("work", false)) with { Snippet = "{}" }));
        Assert.Equal(AdvancedPageRules.GrantsFingerprint(null), AdvancedPageRules.GrantsFingerprint(Panel(false)));
    }

    [Fact]
    public void TheResetIsDrawnAgainAsItStartsAndEnds()
    {
        var free = new ControlState(true, false);
        var running = new ControlState(false, true);
        var idle = AdvancedPageRules.ResetFingerprint(Page(free, null));
        Assert.NotEqual(idle, AdvancedPageRules.ResetFingerprint(Page(running, null)));
        Assert.NotEqual(idle, AdvancedPageRules.ResetFingerprint(Page(free, ResetOutcome.Done)));
        Assert.NotEqual(
            AdvancedPageRules.ResetFingerprint(Page(free, ResetOutcome.Done)),
            AdvancedPageRules.ResetFingerprint(Page(free, ResetOutcome.Failed)));
    }
}
