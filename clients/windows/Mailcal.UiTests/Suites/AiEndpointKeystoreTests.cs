// The own AI endpoint's Save against a keystore that takes its time or refuses (docs/ai.md rule 7,
// docs/waiting.md rules 1 and 5), staged with MAILCAL_FAKE_KEYSTORE on the harness launch, whose
// keystore is a namespace of its own (debug builds only): the input is faked, everything the form
// draws from it is real. Saving dials nothing, and what a case stores it removes again.

using Allodia.Mailcal.UiTests.Driver;
using Xunit;

namespace Allodia.Mailcal.UiTests.Suites;

/// <summary>The harness launch with a keystore whose every write takes a second and a half.</summary>
public sealed class SlowKeystoreHarnessApp : HarnessApp
{
    protected override IReadOnlyDictionary<string, string> Env { get; } =
        new Dictionary<string, string> { ["MAILCAL_FAKE_KEYSTORE"] = "slow" };
}

/// <summary>The harness launch with a keystore that refuses every write.</summary>
public sealed class RefusingKeystoreHarnessApp : HarnessApp
{
    protected override IReadOnlyDictionary<string, string> Env { get; } =
        new Dictionary<string, string> { ["MAILCAL_FAKE_KEYSTORE"] = "refuses" };
}

[Trait("Dataset", "harness")]
public sealed class AiEndpointSlowKeystoreTests(SlowKeystoreHarnessApp harness) : IClassFixture<SlowKeystoreHarnessApp>
{
    [HarnessFact]
    public void SaveRunsWithItsFieldsHeldUntilTheKeyIsStored() =>
        harness.Case(nameof(SaveRunsWithItsFieldsHeldUntilTheKeyIsStored), app =>
        {
            var form = AiEndpointFormTests.Form(app);
            AiEndpointFormTests.Fill(app, form, key: "sk-harness-only");
            app.Agent.Invoke(AiEndpointFormTests.Save(app, form));

            // Read while the write is held up: the ring beside Save, Save and the fields held.
            Assert.True(App.Eventually(
                () => app.Agent.Find(new Filter { Id = "AiEndpointSave_Running", Under = form }) is not null,
                TimeSpan.FromSeconds(5)), "Save shows its ring from the press");
            Assert.False(AiEndpointFormTests.Save(app, form).Enabled, "a running Save cannot be pressed again");
            Assert.False(AiEndpointFormTests.Field(app, form, "AiEndpointAddress").Enabled, "the fields are held with Save");
            Assert.True(AiEndpointFormTests.Line(app, form, "AiEndpointRemove") is not { Enabled: true }, "Remove cannot be pressed while Save runs");

            Assert.True(App.Eventually(
                () => AiEndpointFormTests.Line(app, AiEndpointFormTests.Form(app), "AiEndpointRemove") is { Enabled: true },
                TimeSpan.FromSeconds(15)), "the save ends, and an endpoint stored is one Remove is offered for");
            form = AiEndpointFormTests.Form(app);
            Assert.True(app.Agent.Find(new Filter { Id = "AiEndpointSave_Running", Under = form }) is null, "the ring goes with the work");
            Assert.Equal(Catalog.English("ai_endpoint_key_stored"), AiEndpointFormTests.Field(app, form, "AiEndpointKey_Note").Name);
            Assert.True(string.IsNullOrEmpty(app.Agent.Read(AiEndpointFormTests.Field(app, form, "AiEndpointKey")).Value),
                "the stored key is never drawn back");
        }, AiEndpointFormTests.RemoveSaved);
}

[Trait("Dataset", "harness")]
public sealed class AiEndpointRefusingKeystoreTests(RefusingKeystoreHarnessApp harness) : IClassFixture<RefusingKeystoreHarnessApp>
{
    [HarnessFact]
    public void AKeyTheKeystoreRefusedIsSaidOnceAndTheFieldsKeepTheEdit() =>
        harness.Case(nameof(AKeyTheKeystoreRefusedIsSaidOnceAndTheFieldsKeepTheEdit), app =>
        {
            var form = AiEndpointFormTests.Form(app);
            AiEndpointFormTests.Fill(app, form, key: "sk-not-stored");
            app.Agent.Invoke(AiEndpointFormTests.Save(app, form));
            var refused = Catalog.English("ai_endpoint_key_not_stored");
            Element? said = null;
            Assert.True(App.Eventually(() => (said = AiEndpointFormTests.Line(app, form, "AiEndpointOutcome")) is not null
                && app.Agent.FindAll(new Filter { Under = said }).Any(e => e.Name == refused), TimeSpan.FromSeconds(10)),
                "a key the keystore refused is said on the form");
            Assert.Equal(AiEndpointFormTests.Address, app.Agent.Read(AiEndpointFormTests.Field(app, form, "AiEndpointAddress")).Value);
            Assert.True(AiEndpointFormTests.Line(app, form, "AiEndpointRemove") is null, "nothing was stored, so Remove is not drawn");

            app.Agent.Invoke(app.Agent.Find(new Filter { Id = "CloseButton", Under = said })
                ?? throw new InvalidOperationException("what the save came to cannot be closed"));
            Assert.True(App.Eventually(() => AiEndpointFormTests.Line(app, form, "AiEndpointOutcome") is null, TimeSpan.FromSeconds(5)),
                "closed, it goes");

            // The page drawn again from the core's record, which still carries that outcome.
            new SetupFlow(app).OpenCategory("general");
            Assert.True(AiEndpointFormTests.Line(app, AiEndpointFormTests.Form(app), "AiEndpointOutcome") is null,
                "an outcome already said is not said again");
        }, app => app.CloseDialogs());
}
