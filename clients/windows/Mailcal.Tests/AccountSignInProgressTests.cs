// What an account page's Sign-in section lets the person press while its sign-in is out
// (docs/provider-oauth.md rule 13, docs/onboarding.md "Nobody may be stranded on it"). Closing the
// browser tab sends nothing back, so buttons disabled until the sign-in ended were dead until the
// five-minute cap.

using Allodia.Mailcal.Services;
using Xunit;

namespace Allodia.Mailcal.Tests;

public class AccountSignInProgressTests
{
    private const string Account = "alice@example.org@imap.example.org";

    [Fact]
    public void OnlyABlockingCallDisablesTheButtons()
    {
        var progress = new AccountSignInProgress();
        Assert.True(progress.ControlsEnabled(Account));

        var attempt = progress.Start(Account);
        Assert.False(progress.ControlsEnabled(Account));
        Assert.False(progress.ShowsCancel(Account));

        progress.Step(attempt, SignInStep.Asking);
        Assert.True(progress.ControlsEnabled(Account));
        // The question has a Cancel of its own.
        Assert.False(progress.ShowsCancel(Account));

        progress.Step(attempt, SignInStep.Browser);
        Assert.True(progress.ControlsEnabled(Account));
        Assert.True(progress.ShowsCancel(Account));

        // The exchange after the redirect is a blocking call, and nothing can retire it.
        progress.Step(attempt, SignInStep.Calling);
        Assert.False(progress.ControlsEnabled(Account));
        Assert.False(progress.ShowsCancel(Account));

        Assert.True(progress.End(attempt));
        Assert.True(progress.ControlsEnabled(Account));
    }

    [Fact]
    public void AnotherAccountsPageIsNeverHeldByThisSignIn()
    {
        var progress = new AccountSignInProgress();
        progress.Start(Account);
        Assert.True(progress.ControlsEnabled("bob@example.org@imap.example.org"));
        Assert.False(progress.ShowsCancel("bob@example.org@imap.example.org"));
    }

    [Fact]
    public void ASupersededSignInNeitherMovesNorEndsTheOneThatReplacedIt()
    {
        var progress = new AccountSignInProgress();
        var first = progress.Start(Account);
        progress.Step(first, SignInStep.Browser);

        // The person closed the browser tab and pressed again.
        var second = progress.Start(Account);
        progress.Step(first, SignInStep.Browser);
        Assert.False(progress.ControlsEnabled(Account));

        Assert.False(progress.End(first));
        Assert.True(progress.IsOut(Account));

        progress.Step(second, SignInStep.Browser);
        Assert.True(progress.ShowsCancel(Account));
        Assert.True(progress.End(second));
        Assert.False(progress.IsOut(Account));
    }
}
