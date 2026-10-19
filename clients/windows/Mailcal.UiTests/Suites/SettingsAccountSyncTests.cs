// An account's page in Settings → Accounts: how one mail account is shared with the person's other
// devices, and what a refusal from the core reads like on screen.
//
// WHY HERE. MailboxModel is a WinUI type, so the failure text it assigns, and the order the page is
// built in, are invisible to Mailcal.Tests.
//
// WHY THE HARNESS. A harness run signs in to no Allodia account, so asking the core to change an
// account's sync position is refused every time. That makes the error path deterministic, and it is
// the path that once put a generated field name in front of the user. Only a build with the Allodia
// sign-in draws the control, so every case needs one.

using System.Text.RegularExpressions;
using Allodia.Mailcal.UiTests.Driver;
using Xunit;

namespace Allodia.Mailcal.UiTests.Suites;

[Collection(HarnessCollection.Name)]
[Trait("Dataset", "harness")]
public sealed class SettingsAccountSyncTests(HarnessApp harness)
{
    private void Case(string name, Action<App, Element> body) =>
        harness.Case(name, app => body(app, new SetupFlow(app).OpenAccount(Harness.Alice)), app => app.CloseDialogs());

    private static Element Picker(App app, Element dialog) =>
        app.Agent.Wait(new Filter { Id = "AccountSyncMode", Under = dialog, OnScreen = true }, TimeSpan.FromSeconds(10))
        ?? throw new InvalidOperationException("the account page draws no sync-position control (#AccountSyncMode)");

    /// <summary>Every text drawn in the dialog, in tree order.</summary>
    private static List<string> Texts(App app, Element dialog) =>
        app.Agent.FindAll(new Filter { Type = "Text", Under = dialog, OnScreen = true }).Select(text => text.Name).ToList();

    /// <summary>Asks for Paused, which the core refuses with nobody signed in, and returns the line
    /// that says so: whichever state the core reports decides which of the catalog's sentences.</summary>
    private static string Refuse(App app, Element dialog)
    {
        var paused = app.Agent.Find(new Filter { Type = "RadioButton", Under = Picker(app, dialog), Name = Catalog.English("settings_account_sync_paused") })
            ?? throw new InvalidOperationException("the sync-position control offers no Paused position");
        app.Agent.Select(paused);
        var refusals = new[] { "settings_allodia_sync_unavailable", "settings_allodia_signed_out_elsewhere", "settings_allodia_reauth" }
            .SelectMany(Catalog.Everywhere).ToHashSet();
        string? failure = null;
        App.Eventually(() => (failure = Texts(app, dialog).FirstOrDefault(refusals.Contains)) is not null, TimeSpan.FromSeconds(10));
        return failure
            ?? throw new InvalidOperationException($"a refused change says nothing, which reads as the change having been made. On screen: {string.Join(" | ", Texts(app, dialog))}");
    }

    [HarnessFact(NeedsAllodia = true)]
    public void AnAccountsAddressComesBeforeTheControlThatSharesIt() => Case(nameof(AnAccountsAddressComesBeforeTheControlThatSharesIt), (app, dialog) =>
    {
        var address = app.Agent.Find(new Filter { Type = "Text", Name = Harness.Alice, Under = dialog, OnScreen = true })
            ?? throw new InvalidOperationException("the account page does not name its account");
        var picker = Picker(app, dialog);
        // The page has no box around its sections, so the address is what says which account the
        // rows under it are about.
        Assert.True(address.Y < picker.Y, $"the address names the page, so it comes first (address at {address.Y}, sync control at {picker.Y})");
    });

    [HarnessFact(NeedsAllodia = true)]
    public void ARefusalFromTheCoreReadsAsASentenceNotAsAGeneratedFieldName() => Case(nameof(ARefusalFromTheCoreReadsAsASentenceNotAsAGeneratedFieldName), (app, dialog) =>
    {
        Refuse(app, dialog);
        // UniFFI's C# codegen builds each error's message as "@v1=" + the field, so a client that
        // shows ex.Message puts a generated field name in product copy. Nothing in this repo writes
        // "@v1", so it shows only when an error path runs.
        Assert.DoesNotMatch(@"@v\d+=", string.Join(" | ", Texts(app, dialog)));
    });

    [HarnessFact(NeedsAllodia = true)]
    public void NoFailurePutsAnOAuthErrorCodeOrEndpointJargonInFrontOfAPerson() => Case(nameof(NoFailurePutsAnOAuthErrorCodeOrEndpointJargonInFrontOfAPerson), (app, dialog) =>
    {
        Refuse(app, dialog);
        // The core answers with a typed health and the client draws from that, so no path runs from
        // an exception's text to the screen. This asserts the absence of the whole class: an OAuth
        // error code, an endpoint's own words or a scope name is jargon whichever error produced it.
        var shown = Texts(app, dialog);
        foreach (var pattern in new[] { "oauth", "invalid_scope", "invalid_grant", "mailcal:", @"@v\d+=", "endpoint", @"http \d\d\d" })
        {
            var offending = shown.Where(text => Regex.IsMatch(text, pattern, RegexOptions.IgnoreCase)).ToList();
            Assert.True(offending.Count == 0,
                $"a person is never shown '{pattern}': the core's typed health decides the words, and the detail belongs in the log. On screen: {string.Join(" | ", offending)}");
        }
    });
}
