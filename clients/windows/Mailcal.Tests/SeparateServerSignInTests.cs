// The separate-server form's sign-in at the server's own provider (docs/mail-oauth.md rule 13):
// offered once the core's section says so, an answer from an attempt that was superseded or
// retired does nothing, and success ends the form.

using Allodia.Mailcal.Services;
using uniffi.mailcal_bindings;
using Xunit;

namespace Allodia.Mailcal.Tests;

public class SeparateServerSignInTests
{
    private static SeparateServerStep Step(params AccountCapability[] uses) =>
        new("alice@imap", UnreachedUsesRulesTests.Section(uses), UnreachedUsesRules.Title(uses) ?? UnreachedTitle.Both);

    [Fact]
    public void TheSignInIsOfferedOnceTheCoreDrawsIt()
    {
        var both = Step(AccountCapability.Calendar, AccountCapability.Contacts);
        // Unanswered, the form stays as it is: the password path alone.
        Assert.False(SeparateServerSignIn.Offers(both));
        Assert.False(SeparateServerSignIn.Offers(null));

        var provider = both with { Section = both.Section with { SignIn = AccountSeparateSignIn.Provider } };
        Assert.Equal(SeparateSignInOffer.Provider, SeparateServerSignIn.Offer(provider));
        var nextcloud = both with { Section = both.Section with { SignIn = AccountSeparateSignIn.Nextcloud } };
        Assert.Equal(SeparateSignInOffer.Nextcloud, SeparateServerSignIn.Offer(nextcloud));
        Assert.True(SeparateServerSignIn.Offers(nextcloud));
    }

    [Fact]
    public void AFormHoldsItsNextcloudGrantUnderItsOwnAccountAndUses()
    {
        var both = Step(AccountCapability.Calendar, AccountCapability.Contacts);
        Assert.Equal(
            SeparateServerSignIn.Key(both),
            SeparateServerSignIn.Key(Step(AccountCapability.Contacts, AccountCapability.Calendar)));
        Assert.NotEqual(SeparateServerSignIn.Key(both), SeparateServerSignIn.Key(both with { AccountId = "bob@imap" }));
    }

    [Fact]
    public void TheFormIsDisabledOnlyDuringTheBlockingCoreCalls()
    {
        var signIn = new SeparateServerSignIn();
        Assert.True(signIn.ButtonEnabled);
        Assert.True(signIn.FormEnabled);
        Assert.False(signIn.CancelShown);

        var attempt = signIn.Start();
        Assert.False(signIn.ButtonEnabled);
        Assert.False(signIn.FormEnabled);
        // The way back is there from the press, the begin included.
        Assert.True(signIn.CancelShown);
        Assert.True(signIn.Busy);

        Assert.True(signIn.Advanced(attempt, SeparateSignInPhase.Waiting));
        // The browser is open: a fresh press, the password and Skip all work.
        Assert.True(signIn.ButtonEnabled);
        Assert.True(signIn.FormEnabled);
        Assert.True(signIn.CancelShown);

        Assert.True(signIn.Advanced(attempt, SeparateSignInPhase.Completing));
        Assert.False(signIn.ButtonEnabled);
        Assert.False(signIn.FormEnabled);
        // The completion is bounded, so it draws no way back and is not retired.
        Assert.False(signIn.CancelShown);
        Assert.False(signIn.Retire());
        Assert.Equal(SeparateSignInPhase.Completing, signIn.Phase);
    }

    [Fact]
    public void AnAnswerFromASupersededAttemptDoesNothing()
    {
        var signIn = new SeparateServerSignIn();
        var first = signIn.Start();
        signIn.Advanced(first, SeparateSignInPhase.Waiting);

        // A fresh press while the browser is open.
        var second = signIn.Start();
        Assert.False(signIn.Advanced(first, SeparateSignInPhase.Completing));
        Assert.Equal(SeparateSignInPhase.Starting, signIn.Phase);
        Assert.Null(signIn.Ended(first, SeparateSignInEnd.Added("alice@dav")));
        Assert.Null(signIn.Ended(first, SeparateSignInEnd.Failed("refused")));
        Assert.Equal(SeparateSignInPhase.Starting, signIn.Phase);

        // The current one still ends as itself.
        Assert.True(signIn.Advanced(second, SeparateSignInPhase.Waiting));
        Assert.Equal("refused", signIn.Ended(second, SeparateSignInEnd.Failed("refused"))?.Error);
        Assert.Equal(SeparateSignInPhase.Idle, signIn.Phase);
    }

    [Fact]
    public void AnAnswerFromARetiredAttemptDoesNothing()
    {
        var signIn = new SeparateServerSignIn();
        // Nothing out: nothing to cancel.
        Assert.False(signIn.Retire());

        // Retired during the begin: the browser never opens, because the begin's answer finds a
        // different attempt.
        var starting = signIn.Start();
        Assert.True(signIn.Retire());
        Assert.Equal(SeparateSignInPhase.Idle, signIn.Phase);
        Assert.False(signIn.Advanced(starting, SeparateSignInPhase.Waiting));
        Assert.Equal(SeparateSignInPhase.Idle, signIn.Phase);

        // Retired while the browser is open: a redirect landing later adds nothing the form acts on.
        var waiting = signIn.Start();
        signIn.Advanced(waiting, SeparateSignInPhase.Waiting);
        Assert.True(signIn.Retire());
        Assert.Null(signIn.Ended(waiting, SeparateSignInEnd.Added("alice@dav")));
        Assert.Equal(SeparateSignInPhase.Idle, signIn.Phase);
        Assert.True(signIn.ButtonEnabled);
    }

    [Fact]
    public void AnAttemptRetiredOutsideTheFormGoesBackToTheOfferAndSaysNothing()
    {
        // The dialog's close cancels the flow without the form's Cancel: the flow reports Retired.
        var signIn = new SeparateServerSignIn();
        var attempt = signIn.Start();
        signIn.Advanced(attempt, SeparateSignInPhase.Waiting);
        Assert.Null(signIn.Ended(attempt, SeparateSignInEnd.Retired));
        Assert.Equal(SeparateSignInPhase.Idle, signIn.Phase);
        Assert.False(signIn.CancelShown);
    }

    [Fact]
    public void SuccessEndsTheFormAndAFailureKeepsIt()
    {
        var signIn = new SeparateServerSignIn();
        var attempt = signIn.Start();
        signIn.Advanced(attempt, SeparateSignInPhase.Waiting);
        signIn.Advanced(attempt, SeparateSignInPhase.Completing);
        var next = signIn.Ended(attempt, SeparateSignInEnd.Added("alice@dav"));

        Assert.Equal(new SeparateServerNext(null), next);
        Assert.True(next?.GoesOn);
        // A failure stays on the form, as a refused password does.
        var failed = signIn.Ended(signIn.Start(), SeparateSignInEnd.Failed("no"));
        Assert.Equal(new SeparateServerNext("no"), failed);
        Assert.False(failed?.GoesOn);
    }

    private static readonly CoreErrorWords Words = new(
        "refused", "set up", "other", "stale", "calendar unreachable", "contacts unreachable",
        "calendar own sign-in", "contacts own sign-in", "calendar server changed", "contacts server changed",
        "calendar none on server", "contacts none on server", error => error);

    [Fact]
    public void AFailureSaysTheSignInFailedUnlessTheCoreTypedWhatToDo()
    {
        Assert.Equal("other", SeparateServerSignIn.Said(new MailcalException.DifferentAccount(), "failed", Words));
        Assert.Equal("stale", SeparateServerSignIn.Said(new MailcalException.StaleSignIn(), "failed", Words));
        Assert.Equal("failed", SeparateServerSignIn.Said(new MailcalException.Connect("token exchange"), "failed", Words));
        Assert.Equal("failed", SeparateServerSignIn.Said(new TimeoutException(), "failed", Words));
        // A refused sign-in stays the form's own failure.
        Assert.Equal("failed", SeparateServerSignIn.Said(new MailcalException.SignInRefused("401"), "failed", Words));
    }

    [Theory]
    [InlineData(nameof(AccountCapability.Calendar), nameof(UseProblem.Unreachable), "calendar unreachable")]
    [InlineData(nameof(AccountCapability.Contacts), nameof(UseProblem.Unreachable), "contacts unreachable")]
    [InlineData(nameof(AccountCapability.Calendar), nameof(UseProblem.NeedsOwnSignIn), "calendar own sign-in")]
    [InlineData(nameof(AccountCapability.Contacts), nameof(UseProblem.NeedsOwnSignIn), "contacts own sign-in")]
    [InlineData(nameof(AccountCapability.Contacts), nameof(UseProblem.SignInRefused), "refused")]
    public void AUseThatDidNotConnectSaysWhichAndWhy(string capability, string problem, string said) =>
        Assert.Equal(said, SeparateServerSignIn.Said(NotConnected(capability, problem), "failed", Words));

    // Named rather than typed: the generated enums are internal, and a public test cannot take them.
    private static MailcalException.UseNotConnected NotConnected(string capability, string problem) =>
        new(Enum.Parse<AccountCapability>(capability), Enum.Parse<UseProblem>(problem));
}
