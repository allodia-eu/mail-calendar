// Draft a reply for real (docs/ai.md rules 17 to 25), on the harness against the canned endpoint on
// loopback (scripts/dev/ai-mock.py): a style learned from alice's own sent replies, a draft asked
// of the endpoint and put in by the editor, the card's file item ticking itself once a file is
// added, and a send to alice's own address whose Sent copy carries the drafted-reply keyword. The
// reply goes to @test.local, which the harness delivers locally; nothing leaves the machine.

using Allodia.Mailcal.UiTests.Driver;
using Xunit;

namespace Allodia.Mailcal.UiTests.Suites;

[Trait("Dataset", "harness")]
public sealed class ReplyDraftHarnessTests(WritingStyleHarnessApp harness) : IClassFixture<WritingStyleHarnessApp>
{
    private const string Answered = "Message with flags and a custom keyword";
    private static readonly string Subject = $"Drafted reply {DraftsFlow.Run}";

    // Whether this fixture's app has learned its style; both cases draft in it, in either order.
    private static bool _learned;

    /// <summary>Learns a style from alice's sent mail once and leaves its reveal, so her account
    /// drafts in it: a learned style takes the empty slot of the account it was learned from.</summary>
    private static void LearnAStyle(App app)
    {
        if (_learned)
        {
            return;
        }
        AiEndpointFormTests.SaveEndpoint(app, AiMock.Address);
        var dialog = WritingStyleShowcaseTests.Page(app);
        WritingStyleShowcaseTests.Press(app, dialog, "WritingStyleLearn");
        WritingStyleShowcaseTests.WaitStage(app, dialog, "learn_range_title");
        WritingStyleShowcaseTests.Press(app, dialog, "WritingStyleFlowPrimary");
        WritingStyleShowcaseTests.WaitStage(app, dialog, "learn_consent_title");
        WritingStyleShowcaseTests.Wait(app, dialog, "WritingStyleFlowConsent", "the review never said where Learn sends");
        WritingStyleShowcaseTests.Press(app, dialog, "WritingStyleFlowPrimary");
        WritingStyleShowcaseTests.WaitStage(app, dialog, "reveal_title");
        WritingStyleShowcaseTests.Press(app, dialog, "WritingStyleFlowClose");
        app.CloseDialogs();
        _learned = true;
    }

    /// <summary>A reply to the seeded message to alice, under this run's subject, its draft in.</summary>
    private static void DraftAReply(App app)
    {
        LearnAStyle(app);
        app.OpenFolder("Inbox");
        ReplyDraftShowcaseTests.Reply(app, Answered);
        new DraftsFlow(app).SetText("SubjectBox", Subject);
        ReplyDraftShowcaseTests.Draft(app);
    }

    private static void PutBack(App app)
    {
        app.CloseDialogs();
        app.CloseComposer();
        Harness.RemoveMail(Harness.Alice, Subject);
    }

    [HarnessFact]
    public void ALiveDraftComesFromTheEndpointAndTheFileItemTicksOnceAFileIsAdded() =>
        harness.Case(nameof(ALiveDraftComesFromTheEndpointAndTheFileItemTicksOnceAFileIsAdded), app =>
        {
            DraftAReply(app);
            var file = Enumerable.Range(0, 6).Select(index => app.Agent.Find(Filter.ById($"ComposerDraftItem_{index}", "CheckBox")))
                .FirstOrDefault(item => item?.Name.StartsWith("Attach", StringComparison.Ordinal) == true)
                ?? throw new InvalidOperationException("the canned draft's checklist has no file to attach");
            Assert.True(app.Agent.Read(file).Active != true, "the file item is open before anything is added");

            var attachment = Path.Combine(Path.GetTempPath(), $"agenda-{DraftsFlow.Run}.txt");
            File.WriteAllText(attachment, "Agenda");
            try
            {
                app.Agent.Invoke(app.Agent.Find(Filter.Named(Catalog.English("action_attach"), "Button"))
                    ?? throw new InvalidOperationException("the composer offers no Attach"));
                SaveDialogs.Choose(app.ProcessId, attachment);
                Assert.True(App.Eventually(() => app.Agent.Find(Filter.ById(file.Id, "CheckBox")) is { } item && app.Agent.Read(item).Active == true,
                    TimeSpan.FromSeconds(10)), "a file added after the draft ticks its item");
            }
            finally
            {
                File.Delete(attachment);
            }
        }, PutBack);

    [HarnessFact]
    public void ASentDraftedReplyCarriesTheKeywordOnItsSentCopy() =>
        harness.Case(nameof(ASentDraftedReplyCarriesTheKeywordOnItsSentCopy), app =>
        {
            DraftAReply(app);
            app.Agent.Invoke(app.Agent.Find(Filter.ById("SendButton", "Button"))!);
            if (app.Agent.Wait(Filter.Named(Catalog.English("composer_send_open_title")), TimeSpan.FromSeconds(10)) is not null)
            {
                app.Agent.Invoke(app.Agent.Find(Filter.Named(Catalog.English("composer_send_anyway"), "Button"))!);
            }
            Assert.True(app.Agent.WaitGone(Filter.ById("SendButton", "Button"), TimeSpan.FromSeconds(15)), "the reply is sent");
            Assert.True(App.Eventually(() => Harness.Keywords(Harness.Alice, Subject).Any(keywords => keywords.Contains("writing-assistant")),
                TimeSpan.FromSeconds(60)), "the Sent copy of a drafted reply carries the writing-assistant keyword");
        }, PutBack);
}
