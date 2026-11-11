// The separate-server form's sign-in through the browser (docs/mail-oauth.md rule 13,
// docs/nextcloud-sign-in.md rule 2), which the core runs: offered once the core's section says so,
// the button exactly the core's control, the line and Cancel while the browser holds it, and what
// a form's going away cancels.

using Allodia.Mailcal.Services;
using uniffi.mailcal_bindings;
using Xunit;

namespace Allodia.Mailcal.Tests;

public class SeparateServerSignInTests
{
    private static readonly ControlState Free = new(true, false);
    private static readonly ControlState Running = new(false, true);
    private static readonly ControlState Held = new(false, false);

    private static SeparateServerStep Step(AccountSeparateSignIn? signIn, params AccountCapability[] uses) =>
        new("alice@imap", UnreachedUsesRulesTests.Section(uses) with { SignIn = signIn },
            UnreachedUsesRules.Title(uses) ?? UnreachedTitle.Both);

    private static BrowserSignIn Out(BrowserSignInStage stage, ulong attempt = 3) =>
        new(attempt, "alice@imap", stage, new ControlState(stage is not BrowserSignInStage.Completing, false));

    private static readonly BrowserSignInStage AtProvider =
        new BrowserSignInStage.InBrowser("https://login.example.org/authorize", "eu.allodia.mailcal:/oauth/imap");

    private static readonly BrowserSignInStage AtNextcloud =
        new BrowserSignInStage.InBrowser("https://cloud.example.org/login/v2/flow", null);

    /// <summary>The section while the core holds a sign-in: Connect and Skip held, the button as
    /// <paramref name="button"/>, and <paramref name="browser"/> out.</summary>
    private static SeparateServerStep Holding(SeparateServerStep step, ControlState button, BrowserSignIn browser) =>
        step with
        {
            Section = step.Section with { Connect = Held, StopUsing = Held, SignInControl = button, Browser = browser },
        };

    [Fact]
    public void TheSignInIsOfferedOnceTheCoreDrawsIt()
    {
        var both = Step(null, AccountCapability.Calendar, AccountCapability.Contacts);
        // Unanswered, the form stays as it is: the password path alone.
        Assert.False(SeparateServerSignIn.Offers(both));
        Assert.False(SeparateServerSignIn.Offers(null));
        Assert.Equal(SeparateSignInOffer.None, SeparateServerSignIn.Row(both).Offer);

        Assert.Equal(SeparateSignInOffer.Provider, SeparateServerSignIn.Row(Step(AccountSeparateSignIn.Provider, AccountCapability.Calendar)).Offer);
        var nextcloud = Step(AccountSeparateSignIn.Nextcloud, AccountCapability.Calendar);
        Assert.Equal(SeparateSignInOffer.Nextcloud, SeparateServerSignIn.Offer(nextcloud));
        Assert.True(SeparateServerSignIn.Offers(nextcloud));

        // Offered and nothing out: the button is free, with the note and no Cancel.
        var row = SeparateServerSignIn.Row(nextcloud);
        Assert.Equal(Free, row.Button);
        Assert.Null(row.Waiting);
        Assert.Null(row.Line);
    }

    [Fact]
    public void TheButtonRunsAsTheCoreSaysWithNoLineBeforeTheBrowser()
    {
        var step = Step(AccountSeparateSignIn.Provider, AccountCapability.Calendar);
        foreach (var stage in new BrowserSignInStage[] { new BrowserSignInStage.Starting(), new BrowserSignInStage.Completing() })
        {
            var row = SeparateServerSignIn.Row(Holding(step, Running, Out(stage)));
            Assert.Equal(Running, row.Button);
            // Nothing waits on the person yet, or any more: no line, no Cancel.
            Assert.Null(row.Waiting);
            Assert.Null(row.Line);
        }
    }

    [Fact]
    public void WhileTheBrowserWaitsTheLineAndItsCancelAreDrawnAndAFreshPressGoes()
    {
        var step = Step(AccountSeparateSignIn.Provider, AccountCapability.Calendar);
        var browser = Out(AtProvider);
        var row = SeparateServerSignIn.Row(Holding(step, Free, browser));
        Assert.Equal(SignInWaitLine.Redirect, row.Line);
        Assert.Same(browser, row.Waiting);
        Assert.True(row.Waiting!.Cancel.Enabled);
        // The core holds Connect while the sign-in is out; the button is its own control and stays
        // pressable, so a press starts a fresh sign-in in place of one abandoned in the browser.
        Assert.True(row.Button.Enabled);
        Assert.False(Holding(step, Free, browser).Section.Connect.Enabled);
    }

    [Fact]
    public void NextcloudsWaitIsSaidAsItsOwn()
    {
        var step = Step(AccountSeparateSignIn.Nextcloud, AccountCapability.Calendar, AccountCapability.Contacts);
        var row = SeparateServerSignIn.Row(Holding(step, Free, Out(AtNextcloud)));
        Assert.Equal(SeparateSignInOffer.Nextcloud, row.Offer);
        // Nothing returns to the app, so the line says the app notices when the person is done.
        Assert.Equal(SignInWaitLine.Nextcloud, row.Line);
        Assert.NotNull(row.Waiting);
    }

    [Fact]
    public void ARedrawFollowsTheSignInAndAReadThatChangedNothingKeepsIt()
    {
        var step = Step(AccountSeparateSignIn.Provider, AccountCapability.Calendar);
        var waiting = Holding(step, Free, Out(AtProvider));
        Assert.Equal(SeparateServerSignIn.Row(waiting).Key, SeparateServerSignIn.Row(Holding(step, Free, Out(AtProvider))).Key);
        Assert.NotEqual(SeparateServerSignIn.Row(step).Key, SeparateServerSignIn.Row(waiting).Key);
        // A fresh press is a new attempt, whose Cancel names it.
        Assert.NotEqual(SeparateServerSignIn.Row(waiting).Key, SeparateServerSignIn.Row(Holding(step, Free, Out(AtProvider, 4))).Key);
        Assert.NotEqual(
            SeparateServerSignIn.Row(Holding(step, Running, Out(new BrowserSignInStage.Starting()))).Key,
            SeparateServerSignIn.Row(waiting).Key);

        // The page and setup's step are drawn again when the sign-in moves.
        Assert.NotEqual(UnreachedUsesRules.Fingerprint(step.Section), UnreachedUsesRules.Fingerprint(waiting.Section));
        var running = step.Section with { SignInControl = Running };
        Assert.NotEqual(UnreachedUsesRules.Fingerprint(step.Section), UnreachedUsesRules.Fingerprint(running));
    }

    [Fact]
    public void TheFormsGoingAwayCancelsASignInStillOutButNotOneBeingCompleted()
    {
        var step = Step(AccountSeparateSignIn.Provider, AccountCapability.Calendar);
        Assert.Null(SeparateServerSignIn.ToCancel(null));
        Assert.Null(SeparateServerSignIn.ToCancel(step.Section));
        var starting = Out(new BrowserSignInStage.Starting());
        Assert.Same(starting, SeparateServerSignIn.ToCancel(Holding(step, Running, starting).Section));
        var waiting = Out(AtProvider);
        Assert.Same(waiting, SeparateServerSignIn.ToCancel(Holding(step, Free, waiting).Section));
        Assert.Null(SeparateServerSignIn.ToCancel(Holding(step, Running, Out(new BrowserSignInStage.Completing())).Section));
    }

    [Fact]
    public void ThePagesCancelWaitsOnlyForWhatItWouldNotStop()
    {
        var step = Step(AccountSeparateSignIn.Provider, AccountCapability.Calendar);
        Assert.True(SeparateServerSignIn.PageCancel(step.Section).Enabled);
        Assert.False(SeparateServerSignIn.PageCancel(step.Section with { Connect = Running }).Enabled);
        // Closing the form cancels a sign-in the browser holds, so it stays pressable then.
        Assert.True(SeparateServerSignIn.PageCancel(Holding(step, Free, Out(AtProvider)).Section).Enabled);
        Assert.False(SeparateServerSignIn.PageCancel(Holding(step, Running, Out(new BrowserSignInStage.Completing())).Section).Enabled);
        // It only closes the form: it never runs.
        Assert.False(SeparateServerSignIn.PageCancel(step.Section).Running);
    }
}
