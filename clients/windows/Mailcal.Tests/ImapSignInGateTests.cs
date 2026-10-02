// What a mail account's setup form asks for, once its server has answered. Every case here is a
// rule that is invisible once wrong: a button offered where it dead-ends, a password field taken
// away from somebody who needs it, or an answer applied to an account nobody asked about.
//
// `Mailcal.Tests` cannot link WinUI, which is exactly why the decision lives in a plain class:
// what is asserted here is the whole decision, not a reflection of it.

using Allodia.Mailcal.Services;
using Xunit;

namespace Allodia.Mailcal.Tests;

public sealed class ImapSignInGateTests
{
    private static ImapSignInGate Asked(ImapAuthAnswer answer)
    {
        var gate = new ImapSignInGate();
        gate.FieldsChanged("alice@example.com", "imap.example.com");
        var key = gate.BeginAsking();
        Assert.NotNull(key);
        gate.Answered(key!, answer);
        return gate;
    }

    [Fact]
    public void NothingIsOfferedUntilTheServerHasAnswered()
    {
        // Not even the password field: one that appears and is then taken away reads as the app
        // changing its mind, and the answer decides whether it belongs there at all.
        var gate = new ImapSignInGate();
        gate.FieldsChanged("alice@example.com", "imap.example.com");
        gate.BeginAsking();

        Assert.Equal(ImapAuthAnswer.Unknown, gate.Answer);
        Assert.False(gate.ShowButton);
        Assert.False(gate.ShowPassword);
    }

    [Fact]
    public void AServerTakingOnlyAPasswordOffersNoSignIn()
    {
        var gate = Asked(ImapAuthAnswer.Password);
        Assert.False(gate.ShowButton);
        Assert.False(gate.ShowRegistrationNeeded);
        Assert.True(gate.ShowPassword);
    }

    [Fact]
    public void AServerOfferingBothLeadsWithSignInAndKeepsThePasswordBehindIt()
    {
        // A field on screen reads as "type your password here" whatever the buttons around it say,
        // so beside a sign-in it waits until somebody asks for it (docs/mail-oauth.md rule 2).
        var gate = Asked(ImapAuthAnswer.SignInOrPassword);
        Assert.True(gate.ShowButton);
        Assert.True(gate.ButtonEnabled);
        Assert.True(gate.SignInLeads);
        Assert.True(gate.ShowPasswordInstead);
        Assert.False(gate.ShowPassword);
    }

    [Fact]
    public void ChoosingThePasswordDrawsTheFieldAndLeavesTheSignInAsAnOrdinaryButton()
    {
        var gate = Asked(ImapAuthAnswer.SignInOrPassword);
        gate.RevealPassword();

        Assert.True(gate.ShowPassword);
        Assert.False(gate.ShowPasswordInstead);
        Assert.True(gate.ShowButton);
        Assert.False(gate.SignInLeads);
    }

    [Fact]
    public void APasswordOnlyServerHasNothingToChooseBetween()
    {
        var gate = Asked(ImapAuthAnswer.Password);
        Assert.False(gate.ShowPasswordInstead);
        Assert.False(gate.SignInLeads);
    }

    [Fact]
    public void AServerThatRefusesPasswordsIsNotOfferedAPasswordField()
    {
        // Microsoft 365's shape. That field is a dead end nobody finds until they have typed one
        // into it and watched the connect fail.
        var gate = Asked(ImapAuthAnswer.SignInOnly);
        Assert.True(gate.ShowButton);
        Assert.False(gate.ShowPassword);
    }

    [Fact]
    public void AClosedSignInIsExplainedAndStillOffersThePassword()
    {
        // No button, because there is no sign-in we can start, and a line saying why. Without the
        // line this screen is indistinguishable from a provider that has no OAuth at all.
        var gate = Asked(ImapAuthAnswer.RegistrationNeeded);
        Assert.False(gate.ShowButton);
        Assert.True(gate.ShowRegistrationNeeded);
        Assert.True(gate.ShowPassword);
    }

    [Fact]
    public void AFailedSignInBringsThePasswordFieldBackEvenWhereTheServerRefusedOne()
    {
        // The route left. A server that said "OAuth only" and then would not sign us in must not
        // leave somebody with nothing at all to try.
        var gate = Asked(ImapAuthAnswer.SignInOnly);
        gate.SignInStarted();
        Assert.False(gate.ButtonEnabled);
        gate.SignInFinished(ImapSignInOutcome.Failed);

        Assert.True(gate.ShowFailure);
        Assert.True(gate.ShowPassword);
        Assert.True(gate.PasswordEnabled);
    }

    [Fact]
    public void ACancelledSignInIsNotAFailure()
    {
        // Closing the browser is a decision, not an error, and a red note over it would be wrong.
        var gate = Asked(ImapAuthAnswer.SignInOrPassword);
        gate.SignInStarted();
        gate.SignInFinished(ImapSignInOutcome.Cancelled);

        Assert.False(gate.ShowFailure);
        Assert.True(gate.ButtonEnabled);
    }

    [Fact]
    public void AnAnswerForAnAccountNobodyIsLookingAtAnyMoreIsInert()
    {
        // The pre-flight dials a mail server, so by the time it answers the person may have typed
        // on. A stale "sign in here" would light a button for a server nobody asked about.
        var gate = new ImapSignInGate();
        gate.FieldsChanged("alice@example.com", "imap.example.com");
        var first = gate.BeginAsking()!;
        gate.FieldsChanged("alice@example.com", "imap.other.example");
        gate.BeginAsking();
        gate.Answered(first, ImapAuthAnswer.SignInOrPassword);

        Assert.Equal(ImapAuthAnswer.Unknown, gate.Answer);
        Assert.False(gate.ShowButton);
    }

    [Fact]
    public void EditingTheAccountKeepsAPasswordFieldWhileTheServerIsAskedAgain()
    {
        // Rule 8: a credential field that appears and is then taken away reads as the app
        // changing its mind. On the manual form the server sits above the password, so correcting
        // it after typing a password is the ordinary order of events.
        var gate = Asked(ImapAuthAnswer.Password);
        gate.FieldsChanged("alice@example.com", "imap.other.example");
        Assert.NotNull(gate.BeginAsking());

        Assert.Equal(ImapAuthAnswer.Unknown, gate.Answer);
        Assert.True(gate.Checking);
        Assert.True(gate.ShowPassword);
        Assert.False(gate.ShowButton);
    }

    [Fact]
    public void AChosenPasswordStaysThroughAnEditAndASecondOfferOfBoth()
    {
        var gate = Asked(ImapAuthAnswer.SignInOrPassword);
        gate.RevealPassword();
        gate.FieldsChanged("bob@example.com", "imap.example.com");
        var key = gate.BeginAsking()!;
        Assert.True(gate.ShowPassword);

        gate.Answered(key, ImapAuthAnswer.SignInOrPassword);
        Assert.True(gate.ShowPassword);
        Assert.False(gate.ShowPasswordInstead);
    }

    [Fact]
    public void OnlyAServerThatRefusesPasswordsTakesAShownFieldAway()
    {
        var gate = Asked(ImapAuthAnswer.RegistrationNeeded);
        gate.FieldsChanged("alice@example.com", "outlook.office365.com");
        var key = gate.BeginAsking()!;
        Assert.True(gate.ShowPassword);

        gate.Answered(key, ImapAuthAnswer.SignInOnly);
        Assert.False(gate.ShowPassword);

        // And from there the next edit has nothing to hold: the form waits for its answer again.
        gate.FieldsChanged("alice@example.com", "imap.example.com");
        gate.BeginAsking();
        Assert.False(gate.ShowPassword);
    }

    [Fact]
    public void AFieldNeverShownIsNotHeldThroughAnEdit()
    {
        // Behind "Use a password instead" and never chosen: nothing is on screen to keep.
        var gate = Asked(ImapAuthAnswer.SignInOrPassword);
        gate.FieldsChanged("bob@example.com", "imap.example.com");
        gate.BeginAsking();
        Assert.False(gate.ShowPassword);
    }

    [Fact]
    public void TheDeadlineAnswersWithAPasswordAndALateReplyChangesNothing()
    {
        // A server that accepts the connection and never speaks must not leave the form with
        // nothing to act on. Only the first answer for an account counts.
        var gate = new ImapSignInGate();
        gate.FieldsChanged("alice@example.com", "imap.example.com");
        var key = gate.BeginAsking()!;

        Assert.True(gate.DeadlinePassed(key));
        Assert.Equal(ImapAuthAnswer.Password, gate.Answer);
        Assert.True(gate.ShowPassword);
        Assert.False(gate.Checking);

        gate.Answered(key, ImapAuthAnswer.SignInOnly);
        Assert.Equal(ImapAuthAnswer.Password, gate.Answer);
        Assert.True(gate.ShowPassword);
        Assert.Null(gate.BeginAsking());
    }

    [Fact]
    public void ADeadlineForAQuestionNobodyIsAskingAnyMoreDecidesNothing()
    {
        var gate = new ImapSignInGate();
        gate.FieldsChanged("alice@example.com", "imap.example.com");
        var first = gate.BeginAsking()!;
        gate.Answered(first, ImapAuthAnswer.SignInOnly);
        Assert.False(gate.DeadlinePassed(first));
        Assert.Equal(ImapAuthAnswer.SignInOnly, gate.Answer);

        gate.FieldsChanged("alice@example.com", "imap.other.example");
        var second = gate.BeginAsking()!;
        gate.FieldsChanged("alice@example.com", "imap.third.example");
        gate.BeginAsking();
        Assert.False(gate.DeadlinePassed(second));
        Assert.Equal(ImapAuthAnswer.Unknown, gate.Answer);
    }

    [Fact]
    public void TheDeadlineIsTheContractsTenSeconds()
    {
        Assert.Equal(TimeSpan.FromSeconds(10), ImapSignInGate.Deadline);
    }

    [Fact]
    public void CheckingIsAboutTheAccountAsItNowReads()
    {
        var gate = new ImapSignInGate();
        gate.FieldsChanged("alice@example.com", "imap.example.com");
        Assert.False(gate.Checking);
        var key = gate.BeginAsking()!;
        Assert.True(gate.Checking);

        gate.FieldsChanged("alice@example.com", "imap.other.example");
        Assert.False(gate.Checking);

        gate.FieldsChanged("alice@example.com", "imap.example.com");
        gate.Answered(key, ImapAuthAnswer.Password);
        Assert.False(gate.Checking);
    }

    [Fact]
    public void AReopenedFormHoldsNoFieldFromTheLastAccount()
    {
        var gate = Asked(ImapAuthAnswer.Password);
        gate.Reset();
        gate.FieldsChanged("bob@example.com", "imap.example.com");
        gate.BeginAsking();
        Assert.False(gate.ShowPassword);
    }

    [Fact]
    public void EditingTheAccountTakesAFailureNoteWithIt()
    {
        var gate = Asked(ImapAuthAnswer.SignInOrPassword);
        gate.SignInStarted();
        gate.SignInFinished(ImapSignInOutcome.Failed);
        Assert.True(gate.ShowFailure);

        gate.FieldsChanged("bob@example.com", "imap.example.com");
        Assert.False(gate.ShowFailure);
    }

    [Fact]
    public void AnAccountWithNothingToDialIsNotWorthADial()
    {
        // Each of these opens a TLS connection to whatever host the field momentarily spells.
        var gate = new ImapSignInGate();
        gate.FieldsChanged("alice", "imap.example.com");
        Assert.Null(gate.BeginAsking());

        gate.FieldsChanged("alice@example.com", "");
        Assert.Null(gate.BeginAsking());
    }

    [Fact]
    public void TheSameAccountIsNeverAskedAboutTwice()
    {
        // Leaving a field and coming back must not spend another dial at the provider.
        var gate = new ImapSignInGate();
        gate.FieldsChanged("alice@example.com", "imap.example.com");
        var key = gate.BeginAsking()!;
        Assert.Null(gate.BeginAsking());
        gate.Answered(key, ImapAuthAnswer.Password);
        Assert.Null(gate.BeginAsking());

        // Case alone is not a different account, so retyping it in another case asks nothing.
        gate.FieldsChanged("Alice@Example.com", "IMAP.Example.com");
        Assert.Null(gate.BeginAsking());
    }
}
