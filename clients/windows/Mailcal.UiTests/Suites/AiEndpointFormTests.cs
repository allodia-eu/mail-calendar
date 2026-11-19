// Settings → Advanced ▸ Own AI endpoint, as the core's ai_endpoint_form() draws it (docs/ai.md
// rules 6 and 7). What a save refuses and what it came to are the core's, pinned in its own tests;
// what is pinned here is the client wiring: the form drawn, Save held with the core's words beside
// each field, and a save shown in the fields. A keystore that is slow or refuses is
// AiEndpointKeystoreTests.
//
// On the harness launch, because the showcase core is not built as a development build and so does
// not offer the writing assistant; the harness server itself is not asked anything. Saving dials
// nothing, and a key is only ever stored in the launch's own namespace and removed again.

using Allodia.Mailcal.UiTests.Driver;
using Xunit;

namespace Allodia.Mailcal.UiTests.Suites;

[Collection(HarnessCollection.Name)]
[Trait("Dataset", "harness")]
public sealed class AiEndpointFormTests(HarnessApp harness)
{
    internal const string Address = "https://ai.example.eu/v1";

    /// <summary>Opens Settings → Advanced and returns the dialog, with the form drawn in it.</summary>
    internal static Element Form(App app)
    {
        var dialog = new SetupFlow(app).OpenCategory("advanced");
        return app.Agent.Wait(new Filter { Id = "AiEndpointAddress", Under = dialog }, TimeSpan.FromSeconds(10)) is null
            ? throw new InvalidOperationException("Settings → Advanced draws no own AI endpoint form, and a debug build offers the writing assistant to everyone")
            : dialog;
    }

    internal static Element Field(App app, Element form, string id) =>
        app.Agent.Find(new Filter { Id = id, Under = form }) ?? throw new InvalidOperationException($"the form has no #{id}");

    internal static Element? Line(App app, Element form, string id) => app.Agent.Find(new Filter { Id = id, Under = form });

    internal static Element Save(App app, Element form) => Field(app, form, "AiEndpointSave");

    /// <summary>Fills the form with an endpoint a save takes, declared to run in the EU by a
    /// European company.</summary>
    internal static void Fill(App app, Element form, string key)
    {
        app.Agent.SetText(Field(app, form, "AiEndpointAddress"), Address);
        app.Agent.SetText(Field(app, form, "AiEndpointModel"), "mistral-small");
        app.Agent.SetText(Field(app, form, "AiEndpointKey"), key);
        var label = Catalog.English("ai_endpoint_where_eu_native");
        app.Agent.Select(app.Agent.Find(new Filter { Name = label, Type = "RadioButton", Under = form })
            ?? throw new InvalidOperationException($"the where-it-runs choice offers no '{label}'"));
    }

    /// <summary>Removes what a case saved, so the next finds nothing set up.</summary>
    internal static void RemoveSaved(App app)
    {
        var form = Form(app);
        if (Field(app, form, "AiEndpointRemove") is { Enabled: true } remove)
        {
            app.Agent.Invoke(remove);
            App.Eventually(() => !Field(app, Form(app), "AiEndpointRemove").Enabled, TimeSpan.FromSeconds(10));
        }
        app.CloseDialogs();
    }

    [HarnessFact]
    public void TheFormIsDrawnWithNothingSetUpAndSaveHeldWithoutAWord() =>
        harness.Case(nameof(TheFormIsDrawnWithNothingSetUpAndSaveHeldWithoutAWord), app =>
        {
            var form = Form(app);
            foreach (var id in new[] { "AiEndpointAddress", "AiEndpointKey", "AiEndpointModel", "AiEndpointWhere" })
            {
                Assert.True(Field(app, form, id).Enabled, $"#{id} can be filled in");
            }
            Assert.Equal(Catalog.English("ai_endpoint_key_hint"), Field(app, form, "AiEndpointKey_Note").Name);
            Assert.False(Save(app, form).Enabled, "Save is held while nothing is filled in");
            Assert.False(Field(app, form, "AiEndpointRemove").Enabled, "Remove is held while nothing is stored");
            Assert.True(Line(app, form, "AiEndpointAddress_Gap") is null, "a form nobody has touched says nothing beside its fields");
        }, app => app.CloseDialogs());

    [HarnessFact]
    public void SaveIsHeldWhileTheFieldsLeaveSomethingOutAndEachFieldSaysWhat() =>
        harness.Case(nameof(SaveIsHeldWhileTheFieldsLeaveSomethingOutAndEachFieldSaysWhat), app =>
        {
            var form = Form(app);
            app.Agent.SetText(Field(app, form, "AiEndpointAddress"), "http://ai.example.eu/v1");
            // WinUI raises TextChanged after the text has changed, not with it.
            var needsHttps = Catalog.English("ai_endpoint_gap_needs_https");
            Assert.True(App.Eventually(() => Line(app, form, "AiEndpointAddress_Gap")?.Name == needsHttps, TimeSpan.FromSeconds(5)),
                $"the address field says '{needsHttps}'; it says '{Line(app, form, "AiEndpointAddress_Gap")?.Name}'");
            Assert.Equal(Catalog.English("ai_endpoint_gap_model_missing"), Line(app, form, "AiEndpointModel_Gap")?.Name);
            Assert.Equal(Catalog.English("ai_endpoint_gap_where_missing"), Line(app, form, "AiEndpointWhere_Gap")?.Name);
            Assert.False(Save(app, form).Enabled, "Save is held while the core says a save would refuse");

            Fill(app, form, key: string.Empty);
            Assert.True(App.Eventually(() => Save(app, form).Enabled, TimeSpan.FromSeconds(5)),
                "Save can be pressed once the core finds nothing missing");
            foreach (var id in new[] { "AiEndpointAddress_Gap", "AiEndpointModel_Gap", "AiEndpointWhere_Gap" })
            {
                Assert.True(Line(app, form, id) is null, $"#{id} says nothing once its field is complete");
            }
        }, app => app.CloseDialogs());

    [HarnessFact]
    public void ASaveWithoutAKeyIsStoredAndRemoveIsOffered() =>
        harness.Case(nameof(ASaveWithoutAKeyIsStoredAndRemoveIsOffered), app =>
        {
            var form = Form(app);
            Fill(app, form, key: string.Empty);
            app.Agent.Invoke(Save(app, form));
            Assert.True(App.Eventually(() => Field(app, Form(app), "AiEndpointRemove").Enabled, TimeSpan.FromSeconds(10)),
                "an endpoint saved is one Remove is offered for");
            form = Form(app);
            Assert.Equal(Address, app.Agent.Read(Field(app, form, "AiEndpointAddress")).Value);
            Assert.Equal(Catalog.English("ai_endpoint_key_hint"), Field(app, form, "AiEndpointKey_Note").Name);
            Assert.True(Line(app, form, "AiEndpointOutcome") is null, "a save that worked says nothing: the fields show it");
        }, RemoveSaved);
}
