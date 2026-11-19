// The writing assistant's failures and remedies in words (docs/ai.md rule 27). Which failure
// happened and which remedy it offers are the core's, pinned in mailcal-app; what is pinned here is
// that each is worded by its own catalog key, and a refusal by the mode that refused it.

using Allodia.Mailcal.Services;
using uniffi.mailcal_bindings;
using Xunit;

namespace Allodia.Mailcal.Tests;

public class AiWordsTests
{
    private static readonly AiWords Words = new(
        "relay sign-in", "key refused", "eu native only", "eu only", "nowhere", "unreachable", "server error",
        "too many", "unreadable", "no credits", "not entitled", "no sent folder", "nothing to learn",
        "too large", "no style", "no style for account", "message gone",
        "sign in", "open endpoint", "try again", "choose account", "choose range", "learn style", "choose style");

    // The generated failures are internal, so a theory names them rather than taking them.
    [Theory]
    [InlineData("relay sign-in")]
    [InlineData("key refused")]
    [InlineData("nowhere")]
    [InlineData("unreachable")]
    [InlineData("server error")]
    [InlineData("too many")]
    [InlineData("unreadable")]
    [InlineData("no credits")]
    [InlineData("not entitled")]
    [InlineData("no sent folder")]
    [InlineData("nothing to learn")]
    [InlineData("too large")]
    [InlineData("no style")]
    [InlineData("no style for account")]
    [InlineData("message gone")]
    public void EachFailureIsWordedByItsOwnKey(string said)
    {
        AiFailure failure = said switch
        {
            "relay sign-in" => new AiFailure.RelaySignIn(),
            "key refused" => new AiFailure.EndpointKeyRefused(),
            "nowhere" => new AiFailure.NowhereToSend(),
            "unreachable" => new AiFailure.Unreachable(),
            "server error" => new AiFailure.ServerError(),
            "too many" => new AiFailure.TooManyRequests(),
            "unreadable" => new AiFailure.Unreadable(),
            "no credits" => new AiFailure.OutOfCredits(),
            "not entitled" => new AiFailure.NotEntitled(),
            "no sent folder" => new AiFailure.NoSentFolder(AnotherAccount: true),
            "nothing to learn" => new AiFailure.NothingToLearn(),
            "too large" => new AiFailure.SampleTooLarge(),
            "no style" => new AiFailure.NoStyle(),
            "no style for account" => new AiFailure.NoStyleForAccount(),
            _ => new AiFailure.MessageGone(),
        };
        Assert.Equal(said, Words.Of(failure));
    }

    // An endpoint run in the EU from outside it is refused only where everything must be run from
    // the EU; anything else is refused by the limit the record names.
    [Fact]
    public void ARefusalSaysWhatTheLimitThatRefusedItAdmits()
    {
        Assert.Equal("eu native only", Words.Of(new AiFailure.Refused(new JurisdictionRefusal.EuHosted())));
        Assert.Equal("eu native only", Words.Of(new AiFailure.Refused(new JurisdictionRefusal.NonEu(JurisdictionLimit.EuNative))));
        Assert.Equal("eu only", Words.Of(new AiFailure.Refused(new JurisdictionRefusal.NonEu(JurisdictionLimit.EuHosted))));
        Assert.Equal("eu only", Words.Of(new AiFailure.Refused(new JurisdictionRefusal.Unknown(JurisdictionLimit.EuHosted))));
    }

    [Fact]
    public void EachRemedyIsWordedByItsOwnKey()
    {
        Assert.Equal("sign in", Words.Of(AiRemedy.SignInToAllodia));
        Assert.Equal("open endpoint", Words.Of(AiRemedy.OpenAiEndpoint));
        Assert.Equal("try again", Words.Of(AiRemedy.TryAgain));
        Assert.Equal("choose account", Words.Of(AiRemedy.ChooseAccount));
        Assert.Equal("choose range", Words.Of(AiRemedy.ChooseRange));
        Assert.Equal("learn style", Words.Of(AiRemedy.LearnStyle));
        Assert.Equal("choose style", Words.Of(AiRemedy.ChooseStyle));
    }
}
