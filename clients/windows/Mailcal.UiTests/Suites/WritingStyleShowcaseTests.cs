// Settings → Writing style on the showcase offering the writing assistant (MAILCAL_OFFER_AI,
// docs/ai.md rules 8 to 16), which seeds two styles on the first account ("Work", already revealed
// and the account's drafting style; "Newly learned", whose reveal has not been shown, in English and
// Dutch with German on greetings only) and stages an endpoint at localhost whose every request is
// unreachable; its sent mail says too little to learn from. Which stage is next, which control is held and why are the core's, pinned in its own
// tests; what is pinned here is that the client draws each record and sends each press as its
// intent. The cases run in the order written, in one launch: the reveal's case leaves it shown.
//
// Learning for real, against a canned endpoint, is WritingStyleHarnessTests.

using Allodia.Mailcal.UiTests.Driver;
using Xunit;

namespace Allodia.Mailcal.UiTests.Suites;

[Collection(OfferAiShowcaseCollection.Name)]
[Trait("Dataset", "showcase")]
public sealed class WritingStyleShowcaseTests(OfferAiShowcaseApp showcase)
{
    private const string Work = "Work";
    private const string NewlyLearned = "Newly learned";

    /// <summary>Opens Settings → Writing style and returns the dialog.</summary>
    internal static Element Page(App app) => new SetupFlow(app).OpenCategory("writing_style");

    internal static Element? Find(App app, Element under, string id) => app.Agent.Find(new Filter { Id = id, Under = under });

    internal static Element Wait(App app, Element under, string id, string why) =>
        app.Agent.Wait(new Filter { Id = id, Under = under }, TimeSpan.FromSeconds(15)) ?? throw new InvalidOperationException(why);

    /// <summary>The title of the host <paramref name="host"/> once it reads <paramref name="key"/>'s words.</summary>
    internal static void WaitStage(App app, Element dialog, string key, string host = "WritingStyleFlow")
    {
        var title = Catalog.English(key);
        Assert.True(App.Eventually(() => Find(app, dialog, host + "Title")?.Name == title, TimeSpan.FromSeconds(15)),
            $"the host reached '{title}'; its title reads '{Find(app, dialog, host + "Title")?.Name}'");
    }

    /// <summary>Presses <paramref name="id"/> once it can be pressed.</summary>
    internal static void Press(App app, Element dialog, string id)
    {
        Assert.True(App.Eventually(() => Find(app, dialog, id) is { Enabled: true }, TimeSpan.FromSeconds(15)), $"#{id} can be pressed");
        app.Agent.Invoke(Find(app, dialog, id)!);
    }

    internal static Element? Row(App app, Element dialog, string name) =>
        app.Agent.FindAll(new Filter { Type = "Button", Under = dialog })
            .FirstOrDefault(e => e.Id.StartsWith("WritingStyleRow_", StringComparison.Ordinal) && e.Name.StartsWith(name + ",", StringComparison.Ordinal));

    private static Element NamedRadio(App app, Element under, string key) =>
        app.Agent.Find(new Filter { Name = Catalog.English(key), Type = "RadioButton", Under = under })
            ?? throw new InvalidOperationException($"no '{Catalog.English(key)}' is offered");

    /// <summary>Closes a flow left open.</summary>
    private static void CloseFlow(App app)
    {
        var dialog = Page(app);
        if (Find(app, dialog, "WritingStyleFlowClose") is { Enabled: true } close)
        {
            app.Agent.Invoke(close);
        }
        app.CloseDialogs();
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
    public void ThePageListsTheLibraryAndEachAccountsStyle() => showcase.Case(nameof(ThePageListsTheLibraryAndEachAccountsStyle), app =>
    {
        var dialog = Page(app);
        Assert.True(Row(app, dialog, Work) is not null, "the library lists Work");
        Assert.True(Row(app, dialog, NewlyLearned) is not null, "the library lists Newly learned");
        var slots = app.Agent.FindAll(new Filter { Type = "ComboBox", Under = dialog }).Where(e => e.Id.StartsWith("WritingStyleSlot_", StringComparison.Ordinal)).ToList();
        Assert.True(slots.Count == 2, $"a slot picker per account used for mail; the page draws {slots.Count}");
        Assert.Equal(Work, app.Agent.Read(slots[0]).Selection);
        Assert.True(Find(app, dialog, "WritingStyleLearn") is { Enabled: true }, "with the staged endpoint, Learn can be pressed");
    }, app => app.CloseDialogs());

    [OfflineFact]
    public void ARevealedStylesRowOpensItsDetailWhichAsksBeforeForgetting() =>
        showcase.Case(nameof(ARevealedStylesRowOpensItsDetailWhichAsksBeforeForgetting), app =>
        {
            var dialog = Page(app);
            app.Agent.Invoke(Row(app, dialog, Work) ?? throw new InvalidOperationException("the library lists no Work"));
            Wait(app, dialog, "WritingStyleDetailPath", "Work, already revealed, opens its detail");

            var name = Wait(app, dialog, "WritingStyleDetailName", "the detail draws no name");
            Assert.Equal(Work, app.Agent.Read(name).Value);
            app.Agent.SetText(name, " ");
            Assert.True(App.Eventually(() => Find(app, dialog, "WritingStyleRename") is { Enabled: false }, TimeSpan.FromSeconds(5)), "Rename is held while the name is blank");
            app.Agent.SetText(name, Work);

            Press(app, dialog, "WritingStyleForget");
            var question = Wait(app, dialog, "WritingStyleForgetMessage", "Forget does not ask first");
            Assert.Contains("eva.jansen@example.com", question.Name, StringComparison.Ordinal);
            Press(app, dialog, "WritingStyleForgetCancel");
            Assert.True(App.Eventually(() => Find(app, dialog, "WritingStyleForgetMessage") is null, TimeSpan.FromSeconds(5)), "Cancel closes the question");
            Assert.True(Find(app, dialog, "WritingStyleDetailPath") is not null, "and keeps the style, its detail still on screen");
        }, app => app.CloseDialogs());

    [OfflineFact]
    public void ANewStylesRowOpensItsRevealWhichPagesThroughItsLanguagesToItsName() =>
        showcase.Case(nameof(ANewStylesRowOpensItsRevealWhichPagesThroughItsLanguagesToItsName), app =>
        {
            var dialog = Page(app);
            app.Agent.Invoke(Row(app, dialog, NewlyLearned) ?? throw new InvalidOperationException("the library lists no Newly learned"));
            WaitStage(app, dialog, "reveal_title", "WritingStyleReveal");
            Assert.True(App.Eventually(() => Find(app, dialog, "WritingStyleRevealSteps")?.Name.StartsWith("Page 1 of ", StringComparison.Ordinal) == true,
                TimeSpan.FromSeconds(5)), "the pips say which page is on screen");
            Assert.True(Find(app, dialog, "WritingStyleRevealBack") is null, "the first page has no Back");

            // A page per language, its control naming each. German was learned for its greetings
            // alone, so on tone and approach it says nothing stood out.
            var voice = Catalog.English("reveal_step_voice");
            for (var page = 0; page < 6 && Find(app, dialog, "WritingStyleRevealTitle")?.Name != voice; page++)
            {
                Press(app, dialog, "WritingStyleRevealPrimary");
                app.Agent.Settle(quiet: TimeSpan.FromMilliseconds(300));
            }
            WaitStage(app, dialog, "reveal_step_voice", "WritingStyleReveal");
            // The control is the platform's SelectorBar, whose items are list items named by language.
            var german = app.Agent.Wait(new Filter { Name = "Deutsch", Type = "ListItem", Under = dialog }, TimeSpan.FromSeconds(5))
                ?? throw new InvalidOperationException("a style in three languages draws no language control offering Deutsch");
            app.Agent.Select(german);
            var nothing = Catalog.English("reveal_nothing_in_language");
            Assert.True(App.Eventually(() => app.Agent.Find(new Filter { Name = nothing, Under = dialog }) is not null, TimeSpan.FromSeconds(5)),
                "a language with nothing on a page says so");

            for (var page = 0; page < 8 && Find(app, dialog, "WritingStyleRevealName") is null; page++)
            {
                Press(app, dialog, "WritingStyleRevealPrimary");
                app.Agent.Settle(quiet: TimeSpan.FromMilliseconds(300));
            }
            var name = Wait(app, dialog, "WritingStyleRevealName", "the reveal never reached its name");
            app.Agent.SetText(name, " ");
            Assert.True(App.Eventually(() => Find(app, dialog, "WritingStyleRevealPrimary") is { Enabled: false }, TimeSpan.FromSeconds(5)), "Save is held while the name is blank");
            app.Agent.SetText(name, NewlyLearned);
            Press(app, dialog, "WritingStyleRevealPrimary");

            // Left by Save, it has been shown: the row opens the detail from now on.
            Assert.True(App.Eventually(() => Row(app, Page(app), NewlyLearned) is not null, TimeSpan.FromSeconds(10)), "Save goes back to the library");
            app.Agent.Invoke(Row(app, Page(app), NewlyLearned)!);
            Wait(app, dialog, "WritingStyleDetailPath", "a style whose reveal was shown opens its detail");
        }, app => app.CloseDialogs());

    [OfflineFact]
    public void TheFlowAsksForTheAccountThenTheRangeAndReviewsWhatTheDeviceHolds() =>
        showcase.Case(nameof(TheFlowAsksForTheAccountThenTheRangeAndReviewsWhatTheDeviceHolds), app =>
        {
            var dialog = Page(app);
            Press(app, dialog, "WritingStyleLearn");

            // Two accounts used for mail: which one, Next held until one is chosen, no Back.
            WaitStage(app, dialog, "learn_account_title");
            Assert.False(Wait(app, dialog, "WritingStyleFlowPrimary", "the stage has no Next").Enabled, "Next is held until an account is chosen");
            Assert.True(Find(app, dialog, "WritingStyleFlowBack") is null, "the first stage has no Back");
            app.Agent.Select(app.Agent.Find(new Filter { Type = "RadioButton", Under = Wait(app, dialog, "WritingStyleFlowAccounts", "no accounts are offered") })
                ?? throw new InvalidOperationException("the account stage offers no account"));
            Press(app, dialog, "WritingStyleFlowPrimary");

            // The core's presets, the last a range of one's own drawn with its two days.
            WaitStage(app, dialog, "learn_range_title");
            app.Agent.Select(NamedRadio(app, Wait(app, dialog, "WritingStyleFlowRanges", "no ranges are offered"), "learn_range_between"));
            Assert.True(app.Agent.Wait(new Filter { Id = "WritingStyleFlowSince", Under = dialog }, TimeSpan.FromSeconds(5)) is not null,
                "a range of one's own is drawn with its two days");
            app.Agent.Select(NamedRadio(app, Wait(app, dialog, "WritingStyleFlowRanges", "no ranges are offered"), "learn_range_all"));
            Press(app, dialog, "WritingStyleFlowPrimary");

            // The review reads on the device. The showcase's sent mail says too little to learn from,
            // so it says so and Learn is held; a run, and a run that fails, are the harness's.
            WaitStage(app, dialog, "learn_consent_title");
            Wait(app, dialog, "WritingStyleFlowNothing", "the review never said nothing here is long enough");
            Assert.True(Find(app, dialog, "WritingStyleFlowFound") is not null, "the review says how many messages it found");
            Assert.True(Find(app, dialog, "WritingStyleFlowPrimary") is not { Enabled: true }, "Learn is held when nothing can be learned from");

            Press(app, dialog, "WritingStyleFlowBack");
            WaitStage(app, dialog, "learn_range_title");
            Press(app, dialog, "WritingStyleFlowClose");
            Assert.True(App.Eventually(() => Find(app, dialog, "WritingStyleLearn") is not null, TimeSpan.FromSeconds(10)), "closing the flow goes back to the page");
        }, CloseFlow);

    [OfflineFact]
    public void WithNowhereToSendLearnSaysWhyAndItsRemedyOpensTheEndpointForm() =>
        showcase.Case(nameof(WithNowhereToSendLearnSaysWhyAndItsRemedyOpensTheEndpointForm), app =>
        {
            AiEndpointFormTests.RemoveSaved(app);
            var dialog = Page(app);
            Assert.True(App.Eventually(() => Find(app, dialog, "WritingStyleLearn") is { Enabled: false }, TimeSpan.FromSeconds(10)), "Learn is held with nowhere to send");
            var blocked = Wait(app, dialog, "WritingStyleLearnBlocked", "the page does not say why Learn is held");
            var nowhere = Catalog.English("ai_failure_nowhere_to_send");
            Assert.True(app.Agent.FindAll(new Filter { Under = blocked }).Any(e => e.Name == nowhere), $"the page says '{nowhere}'");
            var remedy = Wait(app, dialog, "WritingStyleLearnRemedy", "the reason offers no remedy");
            Assert.Equal(Catalog.English("ai_remedy_open_endpoint"), remedy.Name);
            app.Agent.Invoke(remedy);
            Assert.True(app.Agent.Wait(new Filter { Id = "AiEndpointAddress", Under = dialog }, TimeSpan.FromSeconds(10)) is not null,
                "the remedy opens the own endpoint's form");
        }, app => AiEndpointFormTests.SaveEndpoint(app, AiEndpointFormTests.Staged));
}
