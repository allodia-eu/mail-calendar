// Which sentence a writing-style failure earns (docs/ai.md: a failure is a variant, never a
// server's sentence). Describing the exception instead would put UniFFI's generated field names on
// screen, as AllodiaWriteFailureTests records; and two of the mappings are silent when wrong: a
// refusal that names the wrong mode, and a refused sign-in worded for the wrong route.

using System;
using Allodia.Mailcal.Services;
using uniffi.mailcal_bindings;
using Xunit;

namespace Allodia.Mailcal.Tests;

public class WritingStyleFailureTests
{
    // One [Fact] over the pairs rather than a [Theory]: the generated failure types are `internal`,
    // so they cannot cross a public test signature (CS0051).
    [Fact]
    public void EachVariantEarnsItsOwnSentence()
    {
        var expected = new (Exception Failure, AiProblem Problem)[]
        {
            (new WritingStyleFailure.Unavailable(), AiProblem.Unavailable),
            (new WritingStyleFailure.Busy(), AiProblem.Busy),
            (new WritingStyleFailure.NoSentFolder(), AiProblem.NoSentFolder),
            (new WritingStyleFailure.NothingToLearn(), AiProblem.NothingToLearn),
            (new WritingStyleFailure.NoStyle(), AiProblem.NoStyle),
            (new WritingStyleFailure.NotFound(), AiProblem.NotFound),
            (new WritingStyleFailure.OutOfCredits(), AiProblem.OutOfCredits),
            (new WritingStyleFailure.NotEntitled(), AiProblem.NotEntitled),
            (new WritingStyleFailure.RateLimited(), AiProblem.RateLimited),
            (new WritingStyleFailure.Unreachable(), AiProblem.Unreachable),
            (new WritingStyleFailure.Malformed(), AiProblem.Malformed),
            (new WritingStyleFailure.Cancelled(), AiProblem.Cancelled),
        };
        foreach (var (failure, problem) in expected)
        {
            Assert.Equal(problem, AiFailure.For(failure, AiRoute.OwnEndpoint).Problem);
        }
    }

    // The refusal says which rule refused, and the two modes' sentences name different places.
    [Fact]
    public void ARefusalNamesTheModeInForce()
    {
        Assert.Equal(
            AiProblem.RefusedEuNative,
            AiFailure.For(
                new WritingStyleFailure.Refused(JurisdictionMode.EuNative, JurisdictionClass.NonEu),
                AiRoute.OwnEndpoint).Problem);
        Assert.Equal(
            AiProblem.RefusedEuHosted,
            AiFailure.For(
                new WritingStyleFailure.Refused(JurisdictionMode.EuHosted, JurisdictionClass.NonEu),
                AiRoute.OwnEndpoint).Problem);
    }

    // The snapshot's standing verdict is worded the same way as a refused request.
    [Fact]
    public void TheSnapshotsRefusalReadsLikeARefusedRequest()
    {
        Assert.Equal(AiProblem.RefusedEuNative, AiFailure.Refusal(JurisdictionMode.EuNative).Problem);
        Assert.Equal(AiProblem.RefusedEuHosted, AiFailure.Refusal(JurisdictionMode.EuHosted).Problem);
    }

    // On the relay the remedy is signing in to the Allodia account again; on an own endpoint it is
    // the key, which the person set up themselves.
    [Fact]
    public void ARefusedSignInIsWordedForTheRoute()
    {
        Assert.Equal(
            AiProblem.SignInAgain,
            AiFailure.For(new WritingStyleFailure.Unauthorized(), AiRoute.Relay).Problem);
        Assert.Equal(
            AiProblem.KeyRefused,
            AiFailure.For(new WritingStyleFailure.Unauthorized(), AiRoute.OwnEndpoint).Problem);
        Assert.Equal(
            AiProblem.KeyRefused,
            AiFailure.For(new WritingStyleFailure.Unauthorized(), null).Problem);
    }

    [Fact]
    public void AStatusCarriesItsCode()
    {
        var failure = AiFailure.For(new WritingStyleFailure.Status(503), AiRoute.OwnEndpoint);
        Assert.Equal(AiProblem.Status, failure.Problem);
        Assert.Equal(503, failure.StatusCode);
    }

    // A panic crossing the FFI has text of its own, and none of it is shown.
    [Fact]
    public void AnythingTheCoreDidNotNameIsUnavailable() =>
        Assert.Equal(
            new AiFailure(AiProblem.Unavailable),
            AiFailure.For(new InvalidOperationException("internal detail"), AiRoute.Relay));
}
