// What the banner a refused Allodia sign-in raises says (`entitlement.md`, "A refused
// sign-in is also a banner"). Words that name their key, so a test reads which sentence went where.

using Allodia.Mailcal.Services;
using Xunit;

namespace Allodia.Mailcal.Tests;

public class AllodiaSignInEndedBannerTests
{
    [Fact]
    public void TheBannerIsDrawnExactlyWhileTheCoreNamesAnAddress()
    {
        Assert.True(AllodiaSignInEndedBanner.Shown("person@example.test"));
        // Cleared by a sign-in, a sign-out or a call the service accepted: the banner goes with it.
        Assert.False(AllodiaSignInEndedBanner.Shown(null));
        Assert.False(AllodiaSignInEndedBanner.Shown(string.Empty));
    }

    [Fact]
    public void ASignInThatDidNotCompleteSaysSoAndOneThatDidOrWasAbandonedSaysNothing()
    {
        Assert.Null(AllodiaSignInEndedBanner.AfterAttempt(failed: false, stale: false, "signin_stale", "signin_expired_failed"));
        Assert.Equal(
            "signin_expired_failed",
            AllodiaSignInEndedBanner.AfterAttempt(failed: true, stale: false, "signin_stale", "signin_expired_failed"));
        // A browser that came back from another sign-in failed nothing: signing in again goes through.
        Assert.Equal(
            "signin_stale",
            AllodiaSignInEndedBanner.AfterAttempt(failed: true, stale: true, "signin_stale", "signin_expired_failed"));
    }

    [Fact]
    public void TheFailureFollowsThePrompt()
    {
        Assert.Equal("prompt", AllodiaSignInEndedBanner.Message("prompt", null));
        Assert.Equal("prompt signin_expired_failed", AllodiaSignInEndedBanner.Message("prompt", "signin_expired_failed"));
    }
}
