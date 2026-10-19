// Settings → Allodia account (docs/settings.md slot 1): the category for the product's own account,
// and what it draws signed out. The failures this is for are the client's: the category built but
// never reached, the panel reached without the way out, or the category surviving into a build that
// has no route to offer. A build from source has no category at all, and that is a pass, so each
// case states the rule for the build this checkout makes. The showcase signs nobody in.

using Allodia.Mailcal.UiTests.Driver;
using Xunit;

namespace Allodia.Mailcal.UiTests.Suites;

[Collection(ShowcaseCollection.Name)]
[Trait("Dataset", "showcase")]
public sealed class SettingsAllodiaTests(ShowcaseApp showcase)
{
    private static readonly bool Registered = App.AllodiaRegistered();

    private void Case(string name, Action<App, SetupFlow> body) =>
        showcase.Case(name, app => body(app, new SetupFlow(app)), app => app.CloseDialogs());

    private static List<string> Buttons(App app, Element dialog) =>
        app.Agent.FindAll(new Filter { Type = "Button", Under = dialog }).Select(b => b.Name).ToList();

    /// <summary>The Allodia panel, or null in a build with no route, after asserting it is absent
    /// there: a category that opened an empty panel would read as a broken screen.</summary>
    private static Element? AllodiaPanel(SetupFlow settings)
    {
        var categories = settings.Categories();
        if (!Registered)
        {
            Assert.True(!categories.Contains("allodia"), $"this build has no Allodia registration, so the category is not drawn at all; Settings holds {string.Join(" | ", categories)}");
            return null;
        }
        return settings.OpenCategory("allodia");
    }

    [ShowcaseFact]
    public void TheAllodiaAccountIsTheFirstCategoryOrTheBuildHasNoRouteAndItIsAbsent() => Case(nameof(TheAllodiaAccountIsTheFirstCategoryOrTheBuildHasNoRouteAndItIsAbsent), (_, settings) =>
    {
        // First, above the mail settings: it is the identity the rest of the app hangs off.
        if (AllodiaPanel(settings) is not null)
        {
            var categories = settings.Categories();
            Assert.True(categories.FirstOrDefault() == "allodia", $"the Allodia account comes first; the order is {string.Join(" | ", categories)}");
        }
    });

    [ShowcaseFact]
    public void SignedOutTheCategoryOffersAWayInAndAWayToCreate() => Case(nameof(SignedOutTheCategoryOffersAWayInAndAWayToCreate), (app, settings) =>
    {
        if (AllodiaPanel(settings) is not { } panel)
        {
            return;
        }
        // Both: someone with no account pressing a lone Sign in lands on a password they never set,
        // which reads as the app being broken rather than the wrong button.
        var buttons = Buttons(app, panel);
        Assert.True(buttons.Contains(Catalog.English("settings_allodia_sign_in")), $"the signed-out panel offers a way in; it offers {string.Join(" | ", buttons)}");
        Assert.True(buttons.Contains(Catalog.English("settings_allodia_create")), $"and a way to create one; it offers {string.Join(" | ", buttons)}");
    });

    [ShowcaseFact]
    public void SignedOutTheCategorySaysNothingAboutASubscription() => Case(nameof(SignedOutTheCategorySaysNothingAboutASubscription), (app, settings) =>
    {
        if (AllodiaPanel(settings) is not { } panel)
        {
            return;
        }
        // A subscription belongs to an account; a card built regardless reads "Checking your
        // subscription…" forever, waiting on a read that needs somebody signed in (purchasing.md).
        var texts = app.Agent.FindAll(new Filter { Type = "Text", Under = panel }).Select(t => t.Name).ToList();
        Assert.DoesNotContain(Catalog.English("settings_subscription_heading"), texts);
    });

    [ShowcaseFact]
    public void AccountsIsMailAccountsAgainTheAllodiaCardHasLeftIt() => Case(nameof(AccountsIsMailAccountsAgainTheAllodiaCardHasLeftIt), (app, settings) =>
    {
        // Left in both, it would read as two accounts, and an Allodia account holds no mailbox.
        var allodia = new[] { "settings_allodia_sign_in", "settings_allodia_create", "settings_allodia_manage", "settings_allodia_sign_out" }
            .Select(Catalog.English).ToHashSet();
        var strays = Buttons(app, settings.OpenCategory("accounts")).Where(allodia.Contains).ToList();
        Assert.True(strays.Count == 0, $"Accounts holds mail accounts only; it offers {string.Join(" | ", strays)}");
    });
}
