// Learning a writing style end to end (docs/ai.md rules 8 to 16), on the harness, whose Sent Items
// holds alice's own replies in English and Dutch, against the canned endpoint on loopback
// (scripts/dev/ai-mock.py): nothing leaves the machine. Which stage is next, what a run came to and
// what the reveal holds are the core's; what is pinned here is that the Windows client draws each,
// sends each press as its intent, and keeps what is typed.
//
// The cases run in the order written: the first learns the style the second opens.

using Allodia.Mailcal.UiTests.Driver;
using Xunit;

namespace Allodia.Mailcal.UiTests.Suites;

/// <summary>The harness launch beside the canned endpoint, which holds each answer a second so a
/// run's progress can be seen.</summary>
public sealed class WritingStyleHarnessApp : HarnessApp
{
    internal AiMock Mock { get; } = new();

    private protected override App Launch()
    {
        Mock.Start("--delay", "1");
        return base.Launch();
    }

    protected override void Stopped() => Mock.Dispose();
}

[Trait("Dataset", "harness")]
public sealed class WritingStyleHarnessTests(WritingStyleHarnessApp harness) : IClassFixture<WritingStyleHarnessApp>
{
    private const string Learned = "Work";
    private const string Renamed = "Work replies";

    private static Element? Find(App app, Element under, string id) => WritingStyleShowcaseTests.Find(app, under, id);

    private static Element Wait(App app, Element under, string id, string why) => WritingStyleShowcaseTests.Wait(app, under, id, why);

    private static void Press(App app, Element dialog, string id) => WritingStyleShowcaseTests.Press(app, dialog, id);

    private static void WaitStage(App app, Element dialog, string key) => WritingStyleShowcaseTests.WaitStage(app, dialog, key);

    private static Element? Row(App app, Element dialog, string name) =>
        app.Agent.FindAll(new Filter { Type = "Button", Under = dialog })
            .FirstOrDefault(e => e.Id.StartsWith("WritingStyleRow_", StringComparison.Ordinal) && e.Name.StartsWith(name + ",", StringComparison.Ordinal));

    /// <summary>Sets the canned endpoint up, opens the flow on the page and walks it to the review
    /// of the range it starts on, the last twelve months, ready to learn.</summary>
    private static Element ToReview(App app)
    {
        AiEndpointFormTests.SaveEndpoint(app, AiMock.Address);
        var dialog = WritingStyleShowcaseTests.Page(app);
        Assert.True(App.Eventually(() => Find(app, dialog, "WritingStyleLearn") is { Enabled: true }, TimeSpan.FromSeconds(10)),
            "with the canned endpoint set up, Learn can be pressed");
        Press(app, dialog, "WritingStyleLearn");
        // One account used for mail: the flow opens on the range, with no account to choose.
        WaitStage(app, dialog, "learn_range_title");
        Assert.True(App.Eventually(() => Find(app, dialog, "WritingStyleFlowPrimary") is { Enabled: true }, TimeSpan.FromSeconds(5)), "Next can be pressed on a range");
        Press(app, dialog, "WritingStyleFlowPrimary");
        WaitStage(app, dialog, "learn_consent_title");
        Wait(app, dialog, "WritingStyleFlowUsable", "the review never said what it read");
        return dialog;
    }

    /// <summary>Forgets every style in the library, so a case starts from an empty one whatever an
    /// earlier run left: a row whose reveal was never shown is closed first, which counts it as
    /// shown, and then opens its detail.</summary>
    private static void ForgetAll(App app)
    {
        for (var left = 0; left < 10; left++)
        {
            var dialog = WritingStyleShowcaseTests.Page(app);
            var row = app.Agent.FindAll(new Filter { Type = "Button", Under = dialog })
                .FirstOrDefault(e => e.Id.StartsWith("WritingStyleRow_", StringComparison.Ordinal));
            if (row is null)
            {
                return;
            }
            app.Agent.Invoke(row);
            Assert.True(App.Eventually(() => Find(app, dialog, "WritingStyleRevealClose") is not null || Find(app, dialog, "WritingStyleDetailPath") is not null,
                TimeSpan.FromSeconds(10)), $"{row.Name} opens neither its reveal nor its detail");
            if (Find(app, dialog, "WritingStyleRevealClose") is { } close)
            {
                app.Agent.Invoke(close);
                continue;
            }
            Press(app, dialog, "WritingStyleForget");
            Press(app, dialog, "WritingStyleForgetConfirm");
            App.Eventually(() => Find(app, WritingStyleShowcaseTests.Page(app), "WritingStyleDetailPath") is null, TimeSpan.FromSeconds(10));
        }
    }

    /// <summary>Puts the endpoint and the library back as the suite found them.</summary>
    private static void PutBack(App app)
    {
        var dialog = WritingStyleShowcaseTests.Page(app);
        if (Find(app, dialog, "WritingStyleFlowClose") is { Enabled: true } close)
        {
            app.Agent.Invoke(close);
        }
        if (Find(app, dialog, "WritingStyleStop") is { Enabled: true } stop)
        {
            app.Agent.Invoke(stop);
        }
        AiEndpointFormTests.RemoveSaved(app);
    }

    [HarnessFact]
    public void LearningRunsToTheRevealAndItsSavePutsTheStyleInTheLibrary() =>
        harness.Case(nameof(LearningRunsToTheRevealAndItsSavePutsTheStyleInTheLibrary), app =>
        {
            ForgetAll(app);
            var dialog = ToReview(app);
            var consent = Wait(app, dialog, "WritingStyleFlowConsent", "the review does not say where Learn sends");
            Assert.Contains("127.0.0.1", consent.Name, StringComparison.Ordinal);
            Press(app, dialog, "WritingStyleFlowPrimary");

            // The run, with its Stop, until the reveal follows in the same host.
            Assert.True(App.Eventually(() => Find(app, dialog, "WritingStyleFlowProgress") is not null, TimeSpan.FromSeconds(10)), "the run says how far it is");
            Assert.Equal(Catalog.English("learn_stop"), Find(app, dialog, "WritingStyleFlowPrimary")?.Name);
            WaitStage(app, dialog, "reveal_title");
            Wait(app, dialog, "WritingStyleFlowSteps", "the reveal draws no pips");
            Assert.True(App.Eventually(() => Find(app, dialog, "WritingStyleFlowSteps")?.Name.StartsWith("Page 1 of ", StringComparison.Ordinal) == true,
                TimeSpan.FromSeconds(5)), $"the pips say which page is on screen; they say '{Find(app, dialog, "WritingStyleFlowSteps")?.Name}'");

            // Page by page to the name, which is Save's page.
            for (var page = 0; page < 8 && Find(app, dialog, "WritingStyleRevealName") is null; page++)
            {
                Press(app, dialog, "WritingStyleFlowPrimary");
                app.Agent.Settle(quiet: TimeSpan.FromMilliseconds(300));
            }
            var name = Wait(app, dialog, "WritingStyleRevealName", "the reveal never reached its name");
            app.Agent.SetText(name, " ");
            Assert.True(App.Eventually(() => Find(app, dialog, "WritingStyleFlowPrimary") is { Enabled: false }, TimeSpan.FromSeconds(5)), "Save is held while the name is blank");
            app.Agent.SetText(name, Learned);
            Assert.True(App.Eventually(() => Find(app, dialog, "WritingStyleFlowPrimary") is { Enabled: true }, TimeSpan.FromSeconds(5)), "Save goes with a name");
            Press(app, dialog, "WritingStyleFlowPrimary");

            Assert.True(App.Eventually(() => Row(app, dialog, Learned) is not null, TimeSpan.FromSeconds(15)), "the saved style is a row in the library");
            var slot = app.Agent.FindAll(new Filter { Type = "ComboBox", Under = dialog }).FirstOrDefault(e => e.Id.StartsWith("WritingStyleSlot_", StringComparison.Ordinal))
                ?? throw new InvalidOperationException("the page draws no slot for alice's account");
            Assert.Equal(Learned, app.Agent.Read(slot).Selection);
        }, app => app.CloseDialogs());

    [HarnessFact]
    public void TheDetailRenamesAndForgetsTheStyleAskingFirst() =>
        harness.Case(nameof(TheDetailRenamesAndForgetsTheStyleAskingFirst), app =>
        {
            var dialog = WritingStyleShowcaseTests.Page(app);
            var row = Row(app, dialog, Learned) ?? throw new InvalidOperationException("the library holds no style the case before learned");
            app.Agent.Invoke(row);
            Wait(app, dialog, "WritingStyleDetailPath", "a style already revealed opens its detail");

            var name = Wait(app, dialog, "WritingStyleDetailName", "the detail draws no name");
            app.Agent.SetText(name, "  ");
            Assert.True(App.Eventually(() => Find(app, dialog, "WritingStyleRename") is { Enabled: false }, TimeSpan.FromSeconds(5)), "Rename is held while the name is blank");
            app.Agent.SetText(name, Renamed);
            Assert.True(App.Eventually(() => Find(app, dialog, "WritingStyleRename") is { Enabled: true }, TimeSpan.FromSeconds(5)), "Rename goes with a name");
            Press(app, dialog, "WritingStyleRename");
            // The rename holds the page until the library is written.
            Assert.True(App.Eventually(() => Find(app, dialog, "WritingStyleForget") is { Enabled: true }, TimeSpan.FromSeconds(10)),
                "the page comes back free once the rename is written");

            Press(app, dialog, "WritingStyleForget");
            var question = Wait(app, dialog, "WritingStyleForgetMessage", "Forget does not ask first");
            Assert.Contains(Harness.Alice, question.Name, StringComparison.Ordinal);
            Press(app, dialog, "WritingStyleForgetConfirm");
            Assert.True(App.Eventually(() => Find(app, WritingStyleShowcaseTests.Page(app), "WritingStyleEmpty") is not null, TimeSpan.FromSeconds(15)),
                "the forgotten style leaves an empty library");
        }, app =>
        {
            ForgetAll(app);
            app.CloseDialogs();
        });

    [HarnessFact]
    public void ClosingTheFlowWhileItLearnsLeavesTheRunOnThePageWithItsStop() =>
        harness.Case(nameof(ClosingTheFlowWhileItLearnsLeavesTheRunOnThePageWithItsStop), app =>
        {
            harness.Mock.Start("--delay", "4");
            var dialog = ToReview(app);
            Press(app, dialog, "WritingStyleFlowPrimary");
            Assert.True(App.Eventually(() => Find(app, dialog, "WritingStyleFlowProgress") is not null, TimeSpan.FromSeconds(10)), "the run starts");
            Press(app, dialog, "WritingStyleFlowClose");
            Wait(app, dialog, "WritingStyleRun", "the run is drawn on the page once its host is closed");
            Press(app, dialog, "WritingStyleStop");
            Assert.True(App.Eventually(() => Find(app, dialog, "WritingStyleRun") is null && Find(app, dialog, "WritingStyleLearn") is not null,
                TimeSpan.FromSeconds(15)), "Stop ends the run and Learn is back");
        }, app =>
        {
            PutBack(app);
            harness.Mock.Start("--delay", "1");
        });

    [HarnessFact]
    public void ARunTheEndpointFailsSaysSoWithItsOneRemedy() =>
        harness.Case(nameof(ARunTheEndpointFailsSaysSoWithItsOneRemedy), app =>
        {
            harness.Mock.Start("--status", "500");
            var dialog = ToReview(app);
            Press(app, dialog, "WritingStyleFlowPrimary");
            WaitStage(app, dialog, "learn_failed_title");
            Assert.Equal(Catalog.English("ai_failure_server_error"), Wait(app, dialog, "WritingStyleFlowFailure", "the failure is not said").Name);
            Assert.Equal(Catalog.English("ai_remedy_try_again"), Find(app, dialog, "WritingStyleFlowPrimary")?.Name);
        }, app =>
        {
            PutBack(app);
            harness.Mock.Start("--delay", "1");
        });
}
