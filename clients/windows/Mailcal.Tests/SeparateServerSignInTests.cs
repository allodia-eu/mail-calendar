// The separate-server form's sign-in at the server's own provider (docs/mail-oauth.md rule 13):
// offered only after the core has said yes for the form's uses, an answer from an attempt that was
// superseded or retired does nothing, and success goes where Connect's success goes.

using Allodia.Mailcal.Services;
using uniffi.mailcal_bindings;
using Xunit;

namespace Allodia.Mailcal.Tests;

public class SeparateServerSignInTests
{
    private static SeparateServerStep Step(params AccountCapability[] uses) => new(
        "alice@imap", uses, UnreachedUsesRules.Title(uses) ?? UnreachedTitle.Both, "dav.example.org", "alice@example.org");

    [Fact]
    public void TheSignInIsOfferedOnlyAfterTheCoreSaysYesForTheFormsUses()
    {
        var signIn = new SeparateServerSignIn();
        var both = Step(AccountCapability.Calendar, AccountCapability.Contacts);

        // Unanswered, the form stays as it is: the password path alone.
        Assert.True(signIn.Asks(both));
        Assert.False(signIn.Offers(both));
        // A redraw does not ask again.
        Assert.False(signIn.Asks(both));

        signIn.Answered(both, true);
        Assert.True(signIn.Offers(both));
        // The same uses in another order are the same form.
        Assert.True(signIn.Offers(Step(AccountCapability.Contacts, AccountCapability.Calendar)));
        // Other uses, or another account, have not been asked about.
        Assert.False(signIn.Offers(Step(AccountCapability.Calendar)));
        Assert.False(signIn.Offers(both with { AccountId = "bob@imap" }));
        Assert.False(signIn.Offers(null));

        var calendar = Step(AccountCapability.Calendar);
        Assert.True(signIn.Asks(calendar));
        signIn.Answered(calendar, false);
        Assert.False(signIn.Offers(calendar));
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
    public void SuccessGoesWhereConnectsSuccessGoes()
    {
        var signIn = new SeparateServerSignIn();
        var attempt = signIn.Start();
        signIn.Advanced(attempt, SeparateSignInPhase.Waiting);
        signIn.Advanced(attempt, SeparateSignInPhase.Completing);
        var next = signIn.Ended(attempt, SeparateSignInEnd.Added("alice@dav"));

        Assert.Equal(SeparateServerNext.AfterConnect(null), next);
        Assert.True(next?.GoesOn);
        // A failure stays on the form, as a refused password does.
        var failed = signIn.Ended(signIn.Start(), SeparateSignInEnd.Failed("no"));
        Assert.Equal(SeparateServerNext.AfterConnect("no"), failed);
        Assert.False(failed?.GoesOn);
    }

    [Fact]
    public void AFailureSaysTheSignInFailedUnlessItCameBackAsAnotherAccount()
    {
        Assert.Equal("other", SeparateServerSignIn.Said(new MailcalException.DifferentAccount(), "failed", "other"));
        Assert.Equal("failed", SeparateServerSignIn.Said(new MailcalException.Connect("token exchange"), "failed", "other"));
        Assert.Equal("failed", SeparateServerSignIn.Said(new TimeoutException(), "failed", "other"));
    }
}
