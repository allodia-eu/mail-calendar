// A first run is a launch with no mail account stored, and the core keeps entries of its own in the
// same store: the own AI endpoint's key among them (docs/ai.md, "Storage"). The client asks the
// core which entries those are (is_reserved_config); counting the key as a mail account would open
// an empty mailbox with no way back to setup. The key is planted in the first-run namespace, never
// the developer's own, and nothing is dialled.

using Allodia.Mailcal.UiTests.Driver;
using Xunit;

namespace Allodia.Mailcal.UiTests.Suites;

/// <summary>A first run whose store holds the own AI endpoint's key and nothing else.</summary>
public sealed class EndpointKeyFirstRunApp : UiApp
{
    /// <summary>The id the core stores the key under, and the stored shape it writes there.</summary>
    internal const string StoredAs = "ai-endpoint";
    private const string KeyEntry = "[ai_endpoint]\nkey = \"sk-planted-for-the-first-run-suite\"\n";

    private protected override App Launch() =>
        App.LaunchOnFirstRunStaged(space => StoredAccounts.Plant(space, StoredAs, KeyEntry), Env);
}

[Trait("Dataset", "first-run")]
public sealed class ReservedEntryFirstRunTests(EndpointKeyFirstRunApp firstRun) : IClassFixture<EndpointKeyFirstRunApp>
{
    // The launch itself waits for the setup form, so a key counted as a mailbox fails it there.
    [OfflineFact]
    public void AStoredEndpointKeyIsNoMailAccountAndTheSetupFormOpens() =>
        firstRun.Case(nameof(AStoredEndpointKeyIsNoMailAccountAndTheSetupFormOpens), app =>
        {
            Assert.True(app.Agent.Find(Filter.ById("DetectEmail", "Edit") with { OnScreen = true }) is not null,
                "with only the endpoint's key stored, the app opens on the account setup form");
            Assert.True(app.Agent.Find(Filter.ById("SettingsItem") with { OnScreen = true }) is null,
                "and not on a mailbox without an account");
        }, reset: false);
}
