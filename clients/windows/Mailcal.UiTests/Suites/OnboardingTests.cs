// First run: the screen that adds the first mail account (docs/onboarding.md). The panel, its order
// on screen and what a screen reader reads are invisible to Mailcal.Tests, and the contract names
// the worst failure itself: the whole panel hangs on one question, and gating only part of it leaves
// a lone "or connect directly" heading under nothing, which draws perfectly.
//
// An emptied store, because a first run is defined by there being nothing in it. Nothing presses
// Continue or the sign-in: the first leaves the screen under test, the second opens a browser at the
// real account service. So the Cancel that ends a sign-in in progress is verified by hand: press
// "Already have one? Sign in", then Cancel, and the card comes back with no error.
//
// Whether this build carries an Allodia registration decides which rule holds, as it decided
// whether the core was compiled with the route: a build from source shows none of the three, and
// the cases about their order and wording skip there, saying so.

using Allodia.Mailcal.UiTests.Driver;
using Xunit;

namespace Allodia.Mailcal.UiTests.Suites;

[Trait("Dataset", "first-run")]
[TestCaseOrderer(DeclarationOrderer.Name, DeclarationOrderer.Assembly)]
public sealed class OnboardingTests(FirstRunApp firstRun) : IClassFixture<FirstRunApp>
{
    private static readonly bool Registered = App.AllodiaRegistered();
    private static readonly string CardTitle = Catalog.English("setup_allodia_title");
    private static readonly string CardSubtitle = Catalog.English("setup_allodia_subtitle");
    private static readonly string SignInLine = Catalog.English("setup_allodia_have_one");
    private static readonly string Divider = Catalog.English("setup_allodia_divider");

    private void Case(string name, Action<App> body) =>
        firstRun.Case(name, body, app =>
        {
            if (app.Agent.Find(Filter.ById("DetectEmail", "Edit")) is { } box)
            {
                app.Agent.SetValue(box, "");
            }
        });

    /// <summary>A case about the card, skipped where the build has none to draw.</summary>
    private void CardCase(string name, Action<App> body)
    {
        Skip.IfNot(App.HasBuild(), "no Debug build to launch (clients/windows/build-and-run.ps1 -NoRun)");
        Skip.IfNot(Registered, "this build carries no Allodia registration, so there is nothing above the address field; the case before this one asserts that");
        Case(name, body);
    }

    /// <summary>The topmost drawn element with that name and type, or null. Typed, since a control's
    /// label carries its name too.</summary>
    private static Element? Drawn(App app, string name, string type) =>
        app.Agent.FindAll(new Filter { Name = name, Type = type, OnScreen = true }).FirstOrDefault(e => e.Rendered);

    private static Element Field(App app, string id, string type) =>
        app.Agent.Find(Filter.ById(id, type) with { OnScreen = true }) ?? throw new InvalidOperationException($"the setup form draws no #{id}");

    [OfflineFact]
    public void TheScreenUnderTestIsTheAccountSetupForm() => Case(nameof(TheScreenUnderTestIsTheAccountSetupForm), app =>
        // First: on the welcome screen before it, every rule below passes by describing nothing.
        Field(app, "DetectEmail", "Edit"));

    [OfflineFact]
    public void TheCardTheSignInLineAndTheDividerAreAbsentTogetherOrPresentTogether() => Case(nameof(TheCardTheSignInLineAndTheDividerAreAbsentTogetherOrPresentTogether), app =>
    {
        var parts = new Dictionary<string, Element?>
        {
            ["the recommendation card"] = Drawn(app, CardTitle, "Group"),
            ["the sign-in line"] = Drawn(app, SignInLine, "Hyperlink"),
            ["the divider"] = Drawn(app, Divider, "Text"),
        };
        // Items 1 to 3 hang off one question (allodia_sign_in_available); a client gating only the
        // card leaves a heading naming a choice nobody was offered.
        var found = parts.Where(p => p.Value is not null).Select(p => p.Key).ToList();
        Assert.True(found.Count == (Registered ? 3 : 0),
            Registered
                ? $"this build carries an Allodia registration, so items 1 to 3 are all on the screen; drawn: {string.Join(", ", found)}"
                : $"this build carries no Allodia registration, so items 1 to 3 are absent together; still drawn: {string.Join(", ", found)}");
    });

    [SkippableFact]
    public void TheScreenIsInTheOrderTheContractFixesCardSignInLineDividerAddress() => CardCase(nameof(TheScreenIsInTheOrderTheContractFixesCardSignInLineDividerAddress), app =>
    {
        var card = Drawn(app, CardTitle, "Group")!.Y;
        var signIn = Drawn(app, SignInLine, "Hyperlink")!.Y;
        var divider = Drawn(app, Divider, "Text")!.Y;
        var address = Field(app, "DetectEmail", "Edit").Y;
        Assert.True(card < signIn, $"item 2 is a line under the card, not a second card (card at {card}, sign-in line at {signIn})");
        Assert.True(signIn < divider, $"the divider names what follows it, so it sits below the way back to an existing account (sign-in line at {signIn}, divider at {divider})");
        Assert.True(divider < address, $"the direct route is item 4 and is not promoted above the card (divider at {divider}, address at {address})");
    });

    [SkippableFact]
    public void TheCardIsOneControlLabelledWithItsActionAndDescribedByItsSubtitle() => CardCase(nameof(TheCardIsOneControlLabelledWithItsActionAndDescribedByItsSubtitle), app =>
    {
        // One control, so a screen reader announces the offer and its action together; named by the
        // action, never the "Recommended" marker, which would announce a judgement first.
        var card = Drawn(app, CardTitle, "Group") ?? throw new InvalidOperationException("the card is not one control (docs/onboarding.md, Accessibility)");
        Assert.Equal(CardTitle, card.Name);
        Assert.Equal(CardSubtitle, app.Agent.Read(card).Help);
    });

    [SkippableFact]
    public void TheCardClaimsTheAccountListOnPhoneAndComputerAndNeverTheWeb() => CardCase(nameof(TheCardClaimsTheAccountListOnPhoneAndComputerAndNeverTheWeb), app =>
    {
        // Read off the screen, never the catalog: a rule checked against a string this file already
        // holds cannot fail. Rule 3: the copy may not out-run the capability matrix, and there is no
        // web client. Rule 4: what travels is the account list, never mail and never a password.
        var subtitle = Drawn(app, CardSubtitle, "Text") ?? throw new InvalidOperationException("the card does not say what the account does (item 1)");
        Assert.DoesNotMatch(@"(?i)\bweb\b", subtitle.Name);
        Assert.Matches("(?i)mail accounts", subtitle.Name);
    });

    [OfflineFact]
    public void SkippingIsOneActionTheDirectRouteIsLiveWhileTheCardIsUp() => Case(nameof(SkippingIsOneActionTheDirectRouteIsLiveWhileTheCardIsUp), app =>
    {
        // Nothing about the card gates the address field, so an address and Continue is the whole of
        // declining; Continue itself is not pressed, which would leave the screen.
        var box = Field(app, "DetectEmail", "Edit");
        Assert.True(box.Enabled, "the address field is never gated behind an answer to the card (rule 1)");
        app.Agent.SetValue(box, "someone@example.com");
        Assert.True(App.Eventually(() => app.Agent.Find(Filter.ById("ContinueButton", "Button")) is { Enabled: true }, TimeSpan.FromSeconds(3)),
            "an address alone is enough to continue: there is no confirmation and no second ask");
    });

    [OfflineFact]
    public void TheSecondStepGoesBackToTheAddressWhichKeepsWhatWasTyped() => Case(nameof(TheSecondStepGoesBackToTheAddressWhichKeepsWhatWasTyped), app =>
    {
        // "Set up manually" is the one way onto the second step that needs no network, and whatever
        // that step is, the address is one action back (docs/account-autodetect.md, rule 12).
        const string address = "someone@example.com";
        app.Agent.SetValue(Field(app, "DetectEmail", "Edit"), address);
        app.Agent.Invoke(Field(app, "ManualButton", "Button"));
        Assert.True(app.Agent.Wait(Filter.ById("ImapHost", "Edit"), TimeSpan.FromSeconds(5)) is not null, "Set up manually opens the manual form");
        var back = app.Agent.Find(Filter.ById("BackButton", "Button"));
        Assert.True(back is { Enabled: true }, "the manual form has a way back to the address step, on a first run as on any later add");
        app.Agent.Invoke(back!);
        var box = app.Agent.Wait(Filter.ById("DetectEmail", "Edit") with { OnScreen = true }, TimeSpan.FromSeconds(5))
            ?? throw new InvalidOperationException("Back did not return to the address step");
        Assert.True(app.Agent.WaitGone(Filter.ById("ImapHost", "Edit") with { OnScreen = true }, TimeSpan.FromSeconds(5)), "the manual form is gone, not drawn under the address step");
        Assert.Equal(address, app.Agent.Read(box).Value);
    });
}
