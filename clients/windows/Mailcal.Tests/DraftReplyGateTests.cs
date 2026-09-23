// The composer's Draft a reply rules (docs/ai.md, "Drafting a reply"). Each is silent when wrong:
// the control offered on a forward, a draft written in the wrong account's style, and, the one that
// costs somebody their text, a draft replacing what they wrote because the editor's answer was read
// as "nothing there".

using Allodia.Mailcal.Services;
using Allodia.Mailcal.ViewModels;
using Xunit;

namespace Allodia.Mailcal.Tests;

public class DraftReplyGateTests
{
    [Theory]
    [InlineData(RichComposeKind.Reply, true)]
    [InlineData(RichComposeKind.ReplyAll, true)]
    [InlineData(RichComposeKind.Forward, false)]
    [InlineData(RichComposeKind.New, false)]
    public void OnlyAReplyOffersADraft(RichComposeKind kind, bool offered) =>
        Assert.Equal(offered, DraftReplyGate.Offered(kind, hasRoute: true));

    // With nowhere for a request to go there is nothing to offer, reply or not.
    [Fact]
    public void NothingIsOfferedWithoutARoute() =>
        Assert.False(DraftReplyGate.Offered(RichComposeKind.Reply, hasRoute: false));

    // The core drafts in the style of the account holding the message unless told otherwise, so a
    // sender changed in the From picker has to be named, and an unchanged one must not be.
    [Fact]
    public void TheSenderIsNamedOnlyWhenThePersonChangedIt()
    {
        Assert.Null(DraftReplyGate.From("work", answered: "work"));
        Assert.Null(DraftReplyGate.From(null, answered: "work"));
        Assert.Equal("home", DraftReplyGate.From("home", answered: "work"));
    }

    [Fact]
    public void AnEmptyIntentIsNoIntent()
    {
        Assert.Null(DraftReplyGate.Intent(null));
        Assert.Null(DraftReplyGate.Intent("   "));
        Assert.Equal("Yes", DraftReplyGate.Intent("  Yes "));
    }

    // WebView2 hands the answer back JSON-encoded, and a hook that failed answers "null". Only an
    // explicit false lets a draft go in without asking.
    [Theory]
    [InlineData("false", false)]
    [InlineData("true", true)]
    [InlineData("null", true)]
    [InlineData(null, true)]
    public void OnlyAnExplicitNoSkipsTheQuestion(string? encoded, bool asks) =>
        Assert.Equal(asks, DraftReplyGate.LeadHasText(encoded));
}
