// An existing account's browser sign-in, held by the core (docs/waiting.md): what this host still
// decides about it. The redirect each kind starts with, the browser opened once per attempt, a
// redirect the OS hands the app routed to the sign-in or left for setup, the loopback kept only
// while the sign-in returns to it, and what the Sign-in section says while it waits. Every step the
// sign-in moves through is the core's, pinned in mailcal-bindings' app_account_page_browser_tests.rs.

using Allodia.Mailcal.Services;
using uniffi.mailcal_bindings;
using Xunit;

namespace Allodia.Mailcal.Tests;

public class BrowserSignInRulesTests
{
    private const string Loopback = "http://127.0.0.1:50123/";

    /// <summary>Attempt 7 of alice's sign-in at <paramref name="stage"/>, its Cancel free.</summary>
    internal static BrowserSignIn At(BrowserSignInStage stage, ulong attempt = 7) =>
        new(attempt, "alice@example.org@imap.example.org", stage, new ControlState(true, false));

    private static BrowserSignIn Waiting(string? redirect, ulong attempt = 7) =>
        At(new BrowserSignInStage.InBrowser("https://auth.example.org/authorize", redirect), attempt);

    [Fact]
    public void EachKindStartsWithTheRedirectItsProviderReturnsTo()
    {
        Assert.Equal(SignInRedirect.Microsoft, BrowserSignInRules.RedirectOf(AccountKind.Microsoft));
        Assert.Equal(SignInRedirect.Google, BrowserSignInRules.RedirectOf(AccountKind.Google));
        Assert.Equal(SignInRedirect.Imap, BrowserSignInRules.RedirectOf(AccountKind.Imap));
        Assert.Equal(SignInRedirect.Jmap, BrowserSignInRules.RedirectOf(AccountKind.Jmap));
        // A calendar and contacts account signs in with Nextcloud, which returns nothing.
        Assert.Equal(SignInRedirect.None, BrowserSignInRules.RedirectOf(AccountKind.Dav));
    }

    [Fact]
    public void TheBrowserOpensOnceForEachAttemptThatWaitsOnIt()
    {
        var waiting = Waiting("app://imap-oauth");
        Assert.Equal("https://auth.example.org/authorize", BrowserSignInRules.ToOpen(waiting, null)?.Url);
        // A redraw of the same attempt opens nothing more; the next attempt opens again.
        Assert.Null(BrowserSignInRules.ToOpen(waiting, 7));
        Assert.NotNull(BrowserSignInRules.ToOpen(Waiting("app://imap-oauth", attempt: 8), 7));
        // Nothing to open before the browser step, at the question or while it completes.
        Assert.Null(BrowserSignInRules.ToOpen(At(new BrowserSignInStage.Starting()), null));
        Assert.Null(BrowserSignInRules.ToOpen(At(new BrowserSignInStage.Asking([AccountCapability.Calendar])), null));
        Assert.Null(BrowserSignInRules.ToOpen(At(new BrowserSignInStage.Completing()), null));
        Assert.Null(BrowserSignInRules.ToOpen(null, null));
        // Nextcloud's opens too, though nothing comes back from it.
        Assert.NotNull(BrowserSignInRules.ToOpen(Waiting(null), null));
    }

    [Fact]
    public void ARedirectGoesToTheSignInOnlyWhenItWaitsAtThatRedirect()
    {
        var incoming = new Uri("eu.allodia.mailcal://imap-oauth?code=abc&state=xyz");
        Assert.True(BrowserSignInRules.Returns(Waiting("eu.allodia.mailcal://imap-oauth"), incoming));
        Assert.True(BrowserSignInRules.Returns(Waiting("EU.Allodia.Mailcal://IMAP-OAUTH"), incoming));
        // Another host is a setup flow's: the JMAP and Microsoft redirects share the scheme.
        Assert.False(BrowserSignInRules.Returns(Waiting("eu.allodia.mailcal://jmap-oauth"), incoming));
        Assert.False(BrowserSignInRules.Returns(Waiting("eu.allodia.mailcal://auth"), incoming));
        // The dev scheme is another build's.
        Assert.False(BrowserSignInRules.Returns(Waiting("eu.allodia.mailcal.dev://imap-oauth"), incoming));
        // A sign-in that does not wait on the browser, or waits on Nextcloud, takes nothing.
        Assert.False(BrowserSignInRules.Returns(Waiting(null), incoming));
        Assert.False(BrowserSignInRules.Returns(At(new BrowserSignInStage.Completing()), incoming));
        Assert.False(BrowserSignInRules.Returns(At(new BrowserSignInStage.Starting()), incoming));
        Assert.False(BrowserSignInRules.Returns(null, incoming));
    }

    [Fact]
    public void ALoopbackIsKeptOnlyWhileTheSignInReturnsToIt()
    {
        // Bound before the start: kept while the sign-in starts or asks.
        Assert.True(BrowserSignInRules.KeepsLoopback(At(new BrowserSignInStage.Starting()), Loopback, null));
        Assert.True(BrowserSignInRules.KeepsLoopback(At(new BrowserSignInStage.Asking([AccountCapability.Calendar])), Loopback, null));
        // Waiting on the browser at it, before and after it is listened on for that attempt.
        Assert.True(BrowserSignInRules.KeepsLoopback(Waiting(Loopback), Loopback, null));
        Assert.True(BrowserSignInRules.KeepsLoopback(Waiting(Loopback), Loopback, 7));
        // Gone once the redirect is in, the sign-in ended or another took its place.
        Assert.False(BrowserSignInRules.KeepsLoopback(At(new BrowserSignInStage.Completing()), Loopback, 7));
        Assert.False(BrowserSignInRules.KeepsLoopback(null, Loopback, 7));
        Assert.False(BrowserSignInRules.KeepsLoopback(null, Loopback, null));
        Assert.False(BrowserSignInRules.KeepsLoopback(Waiting(Loopback, attempt: 8), Loopback, 7));
        Assert.False(BrowserSignInRules.KeepsLoopback(At(new BrowserSignInStage.Starting(), attempt: 8), Loopback, 7));
        Assert.False(BrowserSignInRules.KeepsLoopback(Waiting("eu.allodia.mailcal://auth"), Loopback, null));
    }

    [Fact]
    public void TheSectionSaysItWaitsOnlyWhileTheBrowserHoldsTheSignIn()
    {
        Assert.Equal(SignInWaitLine.Redirect, BrowserSignInRules.WaitingLine(Waiting("eu.allodia.mailcal://auth")));
        Assert.Equal(SignInWaitLine.Redirect, BrowserSignInRules.WaitingLine(Waiting(Loopback)));
        // Nothing returns from Nextcloud's: the line says the app notices when the person finished.
        Assert.Equal(SignInWaitLine.Nextcloud, BrowserSignInRules.WaitingLine(Waiting(null)));
        // Starting and completing are the route's ring; the question has a Cancel of its own.
        Assert.Null(BrowserSignInRules.WaitingLine(At(new BrowserSignInStage.Starting())));
        Assert.Null(BrowserSignInRules.WaitingLine(At(new BrowserSignInStage.Asking([AccountCapability.Contacts]))));
        Assert.Null(BrowserSignInRules.WaitingLine(At(new BrowserSignInStage.Completing())));
        Assert.Null(BrowserSignInRules.WaitingLine(null));
    }

    [Fact]
    public void TheQuestionIsAskedOnlyAtItsStepAboutTheCalendarAndContacts()
    {
        var asking = At(new BrowserSignInStage.Asking([AccountCapability.Contacts, AccountCapability.Mail, AccountCapability.Calendar]));
        Assert.Equal([AccountCapability.Calendar, AccountCapability.Contacts], BrowserSignInRules.Asking(asking));
        Assert.Null(BrowserSignInRules.Asking(Waiting(Loopback)));
        Assert.Null(BrowserSignInRules.Asking(null));
    }
}
