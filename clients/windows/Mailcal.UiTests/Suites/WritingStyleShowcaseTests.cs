// Settings → Writing style on the showcase offering the writing assistant (docs/ai.md rules 8 to
// 16). Which stage is next, which control is held and why are the core's, pinned in its own tests;
// what is pinned here is the client wiring: the category where it belongs, Learn saying why it
// cannot be pressed with the remedy opening what it names, and the flow drawing each stage's
// controls and nothing else.
//
// The showcase's host answers every request as unreachable and its sent mail is too short to learn
// from, so the flow goes as far as a review that finds nothing usable; learning itself, the reveal
// and the detail are WritingStyleHarnessTests. The seeded styles' rows are drawn here once the core
// seeds them, and skip, saying so, until it does.

using Allodia.Mailcal.UiTests.Driver;
using Xunit;

namespace Allodia.Mailcal.UiTests.Suites;

[Collection(OfferAiShowcaseCollection.Name)]
[Trait("Dataset", "showcase")]
public sealed class WritingStyleShowcaseTests(OfferAiShowcaseApp showcase)
{
    /// <summary>Opens Settings → Writing style and returns the dialog.</summary>
    internal static Element Page(App app) => new SetupFlow(app).OpenCategory("writing_style");

    internal static Element? Find(App app, Element under, string id) => app.Agent.Find(new Filter { Id = id, Under = under });

    internal static Element Wait(App app, Element under, string id, string why) =>
        app.Agent.Wait(new Filter { Id = id, Under = under }, TimeSpan.FromSeconds(15)) ?? throw new InvalidOperationException(why);

    /// <summary>The flow's title once it reads <paramref name="key"/>'s words.</summary>
    internal static void WaitStage(App app, Element dialog, string key)
    {
        var title = Catalog.English(key);
        Assert.True(App.Eventually(() => Find(app, dialog, "WritingStyleFlowTitle")?.Name == title, TimeSpan.FromSeconds(15)),
            $"the flow reached '{title}'; its title reads '{Find(app, dialog, "WritingStyleFlowTitle")?.Name}'");
    }

    internal static void Press(App app, Element dialog, string id) =>
        app.Agent.Invoke(Wait(app, dialog, id, $"the panel draws no #{id}"));

    /// <summary>Closes a flow left open and removes the endpoint a case saved.</summary>
    private static void PutBack(App app)
    {
        var dialog = Page(app);
        if (Find(app, dialog, "WritingStyleFlowClose") is { Enabled: true } close)
        {
            app.Agent.Invoke(close);
            App.Eventually(() => Find(app, Page(app), "WritingStyleFlowClose") is null, TimeSpan.FromSeconds(5));
        }
        AiEndpointFormTests.RemoveSaved(app);
    }

    [OfflineFact]
    public void TheCategoryIsOfferedRightAfterSignatures() => showcase.Case(nameof(TheCategoryIsOfferedRightAfterSignatures), app =>
    {
        var categories = new SetupFlow(app).Categories();
        var at = categories.IndexOf("writing_style");
        Assert.True(at > 0 && categories[at - 1] == "signatures",
            $"Writing style follows Signatures while the writing assistant is offered; Settings lists {string.Join(", ", categories)}");
    }, app => app.CloseDialogs());

    [OfflineFact]
    public void LearnSaysWhyItCannotBePressedAndItsRemedyOpensTheEndpointForm() =>
        showcase.Case(nameof(LearnSaysWhyItCannotBePressedAndItsRemedyOpensTheEndpointForm), app =>
        {
            var dialog = Page(app);
            Assert.False(Wait(app, dialog, "WritingStyleLearn", "the page draws no Learn").Enabled, "Learn is held with nowhere to send");
            var blocked = Wait(app, dialog, "WritingStyleLearnBlocked", "the page does not say why Learn is held");
            var nowhere = Catalog.English("ai_failure_nowhere_to_send");
            Assert.True(app.Agent.FindAll(new Filter { Under = blocked }).Any(e => e.Name == nowhere), $"the page says '{nowhere}'");
            var remedy = Wait(app, dialog, "WritingStyleLearnRemedy", "the reason offers no remedy");
            Assert.Equal(Catalog.English("ai_remedy_open_endpoint"), remedy.Name);
            app.Agent.Invoke(remedy);
            Assert.True(app.Agent.Wait(new Filter { Id = "AiEndpointAddress", Under = dialog }, TimeSpan.FromSeconds(10)) is not null,
                "the remedy opens the own endpoint's form");
        }, app => app.CloseDialogs());

    [OfflineFact]
    public void TheFlowAsksForTheAccountThenTheRangeAndReviewsWhatTheDeviceHolds() =>
        showcase.Case(nameof(TheFlowAsksForTheAccountThenTheRangeAndReviewsWhatTheDeviceHolds), app =>
        {
            AiEndpointFormTests.SaveEndpoint(app, AiEndpointFormTests.Address);
            var dialog = Page(app);
            Assert.True(App.Eventually(() => Find(app, dialog, "WritingStyleLearn") is { Enabled: true }, TimeSpan.FromSeconds(10)),
                "with an endpoint to send to, Learn can be pressed");
            Press(app, dialog, "WritingStyleLearn");

            // Two accounts used for mail: which one, Next held until one is chosen, no Back.
            WaitStage(app, dialog, "learn_account_title");
            Assert.False(Wait(app, dialog, "WritingStyleFlowPrimary", "the stage has no Next").Enabled, "Next is held until an account is chosen");
            Assert.True(Find(app, dialog, "WritingStyleFlowBack") is null, "the first stage has no Back");
            app.Agent.Select(app.Agent.Find(new Filter { Type = "RadioButton", Under = Wait(app, dialog, "WritingStyleFlowAccounts", "no accounts are offered") })
                ?? throw new InvalidOperationException("the account stage offers no account"));
            Assert.True(App.Eventually(() => Find(app, dialog, "WritingStyleFlowPrimary") is { Enabled: true }, TimeSpan.FromSeconds(5)), "Next goes once an account is chosen");
            Press(app, dialog, "WritingStyleFlowPrimary");

            // The core's presets, then a range of the person's own with its two days.
            WaitStage(app, dialog, "learn_range_title");
            var ranges = Wait(app, dialog, "WritingStyleFlowRanges", "the range stage offers no ranges");
            app.Agent.Select(app.Agent.Find(new Filter { Name = Catalog.English("learn_range_between"), Type = "RadioButton", Under = ranges })
                ?? throw new InvalidOperationException("the range stage offers no range of one's own"));
            Assert.True(app.Agent.Wait(new Filter { Id = "WritingStyleFlowSince", Under = dialog }, TimeSpan.FromSeconds(5)) is not null,
                "a range of one's own is chosen by its two days");
            app.Agent.Select(app.Agent.Find(new Filter { Name = Catalog.English("learn_range_all"), Type = "RadioButton", Under = Wait(app, dialog, "WritingStyleFlowRanges", "no ranges") })
                ?? throw new InvalidOperationException("the range stage offers no everything"));
            Press(app, dialog, "WritingStyleFlowPrimary");

            // The review reads on the device; this mail says too little, so Learn is held.
            WaitStage(app, dialog, "learn_consent_title");
            Wait(app, dialog, "WritingStyleFlowNothing", "the review never said nothing here is long enough");
            Assert.True(Find(app, dialog, "WritingStyleFlowFound") is not null, "the review says how many messages it found");
            Assert.True(Find(app, dialog, "WritingStyleFlowPrimary") is not { Enabled: true }, "Learn is held when nothing can be learned from");

            Press(app, dialog, "WritingStyleFlowBack");
            WaitStage(app, dialog, "learn_range_title");
            Press(app, dialog, "WritingStyleFlowClose");
            Assert.True(App.Eventually(() => Find(app, dialog, "WritingStyleLearn") is not null, TimeSpan.FromSeconds(10)), "closing the flow goes back to the page");
        }, PutBack);

    // The core's seeded styles (MAILCAL_OFFER_AI): one whose reveal has not been shown, one whose has.
    [OfflineFact]
    public void ASeededStylesRowOpensWhatItsRecordSays() => showcase.Case(nameof(ASeededStylesRowOpensWhatItsRecordSays), app =>
    {
        var dialog = Page(app);
        var rows = app.Agent.FindAll(new Filter { Type = "Button", Under = dialog }).Where(e => e.Id.StartsWith("WritingStyleRow_", StringComparison.Ordinal)).ToList();
        if (rows.Count == 0)
        {
            Assert.Skip("the showcase seeds no writing style yet");
        }
        foreach (var at in Enumerable.Range(0, rows.Count))
        {
            var row = app.Agent.FindAll(new Filter { Type = "Button", Under = Page(app) }).Where(e => e.Id.StartsWith("WritingStyleRow_", StringComparison.Ordinal)).ElementAt(at);
            app.Agent.Invoke(row);
            Assert.True(App.Eventually(() => Find(app, dialog, "WritingStyleRevealSteps") is not null || Find(app, dialog, "WritingStyleDetailPath") is not null,
                TimeSpan.FromSeconds(10)), $"{row.Name} opens a reveal or a detail");
            if (Find(app, dialog, "WritingStyleRevealClose") is { } close)
            {
                app.Agent.Invoke(close);
            }
            else
            {
                app.CloseDialogs();
            }
        }
    }, app => app.CloseDialogs());
}
