// How an account is shared with the person's other devices, and what those devices have to say,
// while nobody is signed in to an Allodia account (docs/settings.md category 10). The core draws
// both only while someone is signed in, so a client that drew either from a list of its own, or
// from a record it kept from before a sign-out, shows a control that changes nothing and a card
// that speaks for devices this one is no longer linked to.
//
// WHY HERE. The page is built in code-behind from the core's record; Mailcal.Tests cannot see what
// it puts on screen.
//
// WHY THE SHOWCASE. It signs nobody in and installs no sync store, so the rule holds in every build,
// with or without the Allodia sign-in. A refused sharing change needs a signed-in account, which
// no shared launch may hold; its wording is the core's typed answer, drawn by SharingFailed.

using Allodia.Mailcal.UiTests.Driver;
using Xunit;

namespace Allodia.Mailcal.UiTests.Suites;

[Collection(ShowcaseCollection.Name)]
[Trait("Dataset", "showcase")]
public sealed class SettingsAccountSyncTests(ShowcaseApp showcase)
{
    private void Case(string name, Action<App, SetupFlow> body) =>
        showcase.Case(name, app => body(app, new SetupFlow(app)), app => app.CloseDialogs());

    /// <summary>Every text drawn in the dialog, in tree order.</summary>
    private static List<string> Texts(App app, Element dialog) =>
        app.Agent.FindAll(new Filter { Type = "Text", Under = dialog }).Select(text => text.Name).ToList();

    [OfflineFact]
    public void SignedOutNoAccountsPageSaysHowItIsShared() => Case(nameof(SignedOutNoAccountsPageSaysHowItIsShared), (app, settings) =>
    {
        var rows = settings.AccountRows();
        Assert.True(rows.Count > 0, "the showcase lists accounts in Settings → Accounts");
        var heading = Catalog.English("settings_account_sync_heading");
        for (var index = 0; index < rows.Count; index++)
        {
            var address = rows[index].Name;
            // OpenAccount waits for the page's Remove, so an absence below is the page's, not a
            // page that had not been drawn yet.
            var dialog = settings.OpenAccount(address, rows.Take(index).Count(row => row.Name == address));
            Assert.Null(app.Agent.Find(new Filter { Id = "AccountSyncMode", Under = dialog }));
            Assert.DoesNotContain(heading, Texts(app, dialog));
            rows = settings.AccountRows();
        }
    });

    [OfflineFact]
    public void SignedOutTheAccountsListSaysNothingForOtherDevices() => Case(nameof(SignedOutTheAccountsListSaysNothingForOtherDevices), (app, settings) =>
    {
        Assert.True(settings.AccountRows().Count > 0, "the showcase lists accounts in Settings → Accounts");
        var texts = Texts(app, settings.Settings());
        // The list is drawn by now: AccountRows waits for its rows.
        Assert.DoesNotContain(Catalog.English("settings_allodia_sync_heading"), texts);
        Assert.DoesNotContain(Catalog.English("settings_allodia_sync_checking"), texts);
    });
}
