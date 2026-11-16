// The Allodia account's card, kept by the core (AllodiaCard): which state Settings and the first-run
// card draw from it and how a failure is worded. Every state itself is the core's, pinned in
// mailcal-bindings' app_allodia_sign_in_tests.rs.

using Allodia.Mailcal.Services;
using uniffi.mailcal_bindings;
using Xunit;

namespace Allodia.Mailcal.Tests;

public class AllodiaCardRulesTests
{
    private static readonly ControlState Free = new(true, false);
    private static readonly AllodiaAccount Anna = new("id-1", "anna@example.org", "Anna");

    private static readonly AllodiaFailureWords Words = new(
        "stale", "browser failed", "timed out", error => $"failed: {error}", refusal => refusal.GetType().Name);

    private static BrowserSignIn Out(ulong attempt = 3) => new(
        attempt,
        "allodia-account",
        new BrowserSignInStage.InBrowser("https://account.example.org/authorize", "eu.allodia.mailcal://account-oauth"),
        Free);

    private static AllodiaCard Card(
        AllodiaCardState? state = null,
        BrowserSignIn? browser = null,
        AllodiaFailure? failure = null,
        AllodiaEndSession? endSession = null,
        AllodiaOnboarding? onboarding = null) => new(
            state ?? new AllodiaCardState.SignedOut(),
            browser,
            Free,
            Free,
            Free,
            failure,
            endSession,
            onboarding ?? new AllodiaOnboarding.Offering());

    [Fact]
    public void SignedOutOffersBothWaysIn()
    {
        var card = Card();
        Assert.Equal(AllodiaCardShows.SignedOut, AllodiaCardRules.Shows(card));
        Assert.Null(AllodiaCardRules.Account(card));
    }

    [Fact]
    public void ASignInOutIsDrawnAsSigningInWhateverTheCardSaysBeneathIt()
    {
        Assert.Equal(AllodiaCardShows.SigningIn, AllodiaCardRules.Shows(Card(browser: Out())));
        // Signing in again over an ended sign-in draws the same, not the ended card's buttons.
        Assert.Equal(
            AllodiaCardShows.SigningIn,
            AllodiaCardRules.Shows(Card(new AllodiaCardState.Ended(Anna), Out())));
    }

    [Fact]
    public void SignedInAndEndedNameTheAccount()
    {
        var signedIn = Card(new AllodiaCardState.SignedIn(Anna));
        var ended = Card(new AllodiaCardState.Ended(Anna));
        Assert.Equal(AllodiaCardShows.SignedIn, AllodiaCardRules.Shows(signedIn));
        Assert.Equal(AllodiaCardShows.Ended, AllodiaCardRules.Shows(ended));
        Assert.Equal("anna@example.org", AllodiaCardRules.Account(signedIn)?.Email);
        Assert.Equal("anna@example.org", AllodiaCardRules.Account(ended)?.Email);
    }

    [Fact]
    public void EachFailureIsSaidInTheCardsWords()
    {
        Assert.Equal("stale", AllodiaCardRules.Said(new AllodiaFailure.SignIn(new AccountChangeRefusal.StaleSignIn()), Words));
        Assert.Equal("browser failed", AllodiaCardRules.Said(new AllodiaFailure.SignIn(new AccountChangeRefusal.BrowserNotOpened()), Words));
        Assert.Equal("timed out", AllodiaCardRules.Said(new AllodiaFailure.SignIn(new AccountChangeRefusal.NotFinished()), Words));
        // Any other refusal goes through the account page's own wording of it.
        Assert.Equal("failed: SignInRefused", AllodiaCardRules.Said(new AllodiaFailure.SignIn(new AccountChangeRefusal.SignInRefused()), Words));
        Assert.Equal("failed: store refused", AllodiaCardRules.Said(new AllodiaFailure.SignOut("store refused"), Words));
        // A cancelled sign-in, or one whose browser was closed, says nothing.
        Assert.Null(AllodiaCardRules.Said(null, Words));
    }

    [Fact]
    public void ASignInOfferedElsewhereSaysOnlyASignInsFailure()
    {
        Assert.Equal("stale", AllodiaCardRules.SignInSaid(new AllodiaFailure.SignIn(new AccountChangeRefusal.StaleSignIn()), Words));
        Assert.Null(AllodiaCardRules.SignInSaid(new AllodiaFailure.SignOut("store refused"), Words));
    }

    [Fact]
    public void TheFirstRunCardDrawsItsWayBackOnlyOnceTheCoreSaysSo()
    {
        Assert.Null(AllodiaCardRules.OnboardingCancel(Card(browser: Out(), onboarding: new AllodiaOnboarding.SigningIn(false))));
        var escape = Card(browser: Out(), onboarding: new AllodiaOnboarding.SigningIn(true));
        Assert.Equal(Free, AllodiaCardRules.OnboardingCancel(escape));
        Assert.Null(AllodiaCardRules.OnboardingCancel(Card(onboarding: new AllodiaOnboarding.Checking())));
    }

    [Fact]
    public void TheServicesSignOutPageOpensOncePerSignOut()
    {
        var card = Card(endSession: new AllodiaEndSession(2, "https://account.example.org/end"));
        Assert.Equal("https://account.example.org/end", AllodiaCardRules.EndSessionToOpen(card, 0)?.Url);
        Assert.Equal("https://account.example.org/end", AllodiaCardRules.EndSessionToOpen(card, 1)?.Url);
        Assert.Null(AllodiaCardRules.EndSessionToOpen(card, 2));
        Assert.Null(AllodiaCardRules.EndSessionToOpen(Card(), 0));
    }

    [Fact]
    public void TheAllodiaRedirectIsTheCoresSignInsAndNotASetupFlows()
    {
        // The driver hands a redirect to the core's sign-in when it waits at the same scheme and
        // host, so the Allodia account's returns there and Microsoft's, on the same scheme, does not.
        Assert.True(BrowserSignInRules.Returns(Out(), new Uri("eu.allodia.mailcal://account-oauth?code=abc&state=xyz")));
        Assert.False(BrowserSignInRules.Returns(Out(), new Uri("eu.allodia.mailcal://auth?code=abc&state=xyz")));
    }

    [Fact]
    public void TheFirstRunCardRedrawsWhenWhatItDrawsMoves()
    {
        var offering = AllodiaCardRules.OnboardingFingerprint(Card());
        Assert.Equal(offering, AllodiaCardRules.OnboardingFingerprint(Card()));
        Assert.NotEqual(offering, AllodiaCardRules.OnboardingFingerprint(Card(browser: Out(), onboarding: new AllodiaOnboarding.SigningIn(false))));
        Assert.NotEqual(
            AllodiaCardRules.OnboardingFingerprint(Card(browser: Out(), onboarding: new AllodiaOnboarding.SigningIn(false))),
            AllodiaCardRules.OnboardingFingerprint(Card(browser: Out(), onboarding: new AllodiaOnboarding.SigningIn(true))));
        Assert.NotEqual(offering, AllodiaCardRules.OnboardingFingerprint(Card(failure: new AllodiaFailure.SignIn(new AccountChangeRefusal.NotFinished()))));
    }
}
