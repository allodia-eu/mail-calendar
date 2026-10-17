// Keeping the message being composed on the server (docs/drafts.md): Save as draft, the idle save,
// leaving a composer, a Drafts row opening back into its composer, and the two ways a stored copy
// leaves, Discard and an accepted send.
//
// WHY HERE AND NOT IN `Mailcal.Tests`. Every rule below is wiring in ComposerView.Drafts.cs,
// MainWindow.Drafts.cs and the list's click handlers, all of which link WinUI. The idle save is the
// sharpest edge: the body is a WebView2 with no channel back, so the host samples the editor's own
// change count, and a sampler that never ran would leave every headless suite green.
//
// WHY THE HARNESS. A draft is a mail write, and the showcase engine performs none. Every case
// asserts on the Drafts folder as the server lists it after a sync, and each takes its draft off the
// server again, so the suite leaves Drafts as it found it. The send case leaves one message in Sent.

using Allodia.Mailcal.UiTests.Driver;
using Xunit;

namespace Allodia.Mailcal.UiTests.Suites;

[Collection(HarnessCollection.Name)]
public sealed class DraftsTests(HarnessApp harness)
{
    private static string Subject(string what) => $"Draft suite {DraftsFlow.Run} {what}";

    /// <summary>A case, then its draft taken off the server again however the case ended, so the
    /// next run starts from the same Drafts.</summary>
    private void Case(string name, Action<App, DraftsFlow> body, string? subject = null) => harness.Case(
        name,
        app => body(app, new DraftsFlow(app)),
        app =>
        {
            if (subject is null)
            {
                app.CloseComposer();
            }
            else
            {
                new DraftsFlow(app).ClearStoredDraft(subject);
            }
        });

    [HarnessFact]
    public void SaveAsDraftStoresTheMessageInDraftsAndLeavingKeepsItThere() => Case(nameof(SaveAsDraftStoresTheMessageInDraftsAndLeavingKeepsItThere), (_, flow) =>
    {
        var subject = Subject("save");
        flow.NewSavedDraft(subject);
        flow.CloseDraftComposer();
        Assert.True(flow.WaitDraftRow(subject),
            "the draft Save as draft reported saved is not in Drafts after leaving the composer: either the save never reached the server, or leaving took it away");
    }, Subject("save"));

    [HarnessFact]
    public void LeavingAComposerThatWasWrittenInSavesItAndAsksNothing() => Case(nameof(LeavingAComposerThatWasWrittenInSavesItAndAsksNothing), (_, flow) =>
    {
        // Never saved by hand and left at once, well inside the idle interval, so only the leave can
        // have put it on the server.
        var subject = Subject("leave");
        flow.OpenNewComposer();
        flow.SetText("ToField", Harness.Bob);
        flow.SetText("SubjectBox", subject);
        flow.CloseDraftComposer();
        Assert.True(flow.WaitDraftRow(subject),
            "a composer left by opening another message is not in Drafts: leaving must save what was written");
    }, Subject("leave"));

    [HarnessFact]
    public void DiscardOverAComposerNothingWasWrittenInAsksNothing() => Case(nameof(DiscardOverAComposerNothingWasWrittenInAsksNothing), (app, flow) =>
    {
        flow.OpenNewComposer();
        Assert.False(app.CloseComposer(),
            "Discard over an untouched composer asked \"Discard draft?\"; with nothing written and nothing stored there is nothing to lose");
        Assert.Null(app.Agent.Find(Filter.ById("SendButton")));
    });

    [HarnessFact]
    public void ADraftsRowOpensBackIntoItsComposerHoldingWhatWasSaved() => Case(nameof(ADraftsRowOpensBackIntoItsComposerHoldingWhatWasSaved), (app, flow) =>
    {
        var subject = Subject("resume");
        flow.NewSavedDraft(subject);
        flow.CloseDraftComposer();
        flow.OpenStoredDraft(subject);
        var box = app.Agent.Find(Filter.ById("SubjectBox")) ?? throw new InvalidOperationException("the resumed composer has no subject field");
        Assert.True(subject == app.Agent.Read(box).Value, "the resumed composer does not hold the subject the draft was saved with");
        // A pill's accessible name is "<address>, Remove recipient", in the app's language.
        var pills = app.Agent.FindAll(new Filter { Type = "Button", NameLike = $"{Harness.Bob}, *" });
        Assert.True(pills.Count == 1, $"the resumed composer does not hold the recipient the draft was saved with ({Harness.Bob})");
        Assert.True(flow.DraftHint() == "", "a composer that has saved nothing yet says nothing about saving (docs/drafts.md)");
    }, Subject("resume"));

    [HarnessFact]
    public void ADraftAnotherClientStoredReopensWithItsPicture() => Case(nameof(ADraftAnotherClientStoredReopensWithItsPicture), (app, flow) =>
    {
        // The picture is a cid: part, so it is on screen only if the composer was seeded with the
        // message's HTML: a draft seeded with its text alone opens with no picture at all.
        var subject = Subject("picture");
        DraftsFlow.AddForeignDraft(subject).GetAwaiter().GetResult();
        flow.OpenStoredDraft(subject);
        // 40 DIPs square, sampled every second pixel each way; half of it is plenty to tell it from none.
        var side = 40 * Input.Scale(app.Mailbox.Hwnd);
        var floor = (int)(side * side / 4 / 2);
        var red = 0;
        Assert.True(App.Eventually(() => (red = flow.EditorRedPixels()) > floor, TimeSpan.FromSeconds(5)),
            $"the resumed composer does not draw the picture the stored draft carries ({red} red pixels, wanted more than {floor}): it was not seeded with the draft's HTML (docs/drafts.md)");
    }, Subject("picture"));

    [HarnessFact]
    public void AReplyLeftAsADraftReopensAsAReply() => Case(nameof(AReplyLeftAsADraftReopensAsAReply), (app, flow) =>
    {
        // The reply's conversation is joined when its composer opens, so even a save made before it
        // was ever sent stores In-Reply-To, and the reopened composer is a reply again.
        var subject = Subject("reply");
        flow.OpenFolder("Inbox");
        app.Agent.Invoke(app.RowTitled("Harness baseline message"));
        var reply = app.Agent.Wait(Filter.ById("ReadingReply"), App.Patience)
            ?? throw new InvalidOperationException("the reading pane never offered Reply");
        app.Agent.Invoke(reply);
        if (app.Agent.Wait(Filter.ById("SendButton"), App.Patience) is null)
        {
            throw new InvalidOperationException("no composer within 30 s of pressing Reply");
        }
        flow.WaitEditorSeeded();
        flow.SetText("SubjectBox", subject);
        app.Agent.Invoke(app.Agent.Find(Filter.ById("SaveDraftButton", "Button"))!);
        if (!flow.WaitDraftSaved())
        {
            throw new InvalidOperationException($"the reply was never reported saved; the hint reads '{flow.DraftHint()}'");
        }
        flow.CloseDraftComposer();
        flow.OpenStoredDraft(subject);
        Assert.True(DraftsFlow.Reply == app.Agent.Find(Filter.ById("ComposerTitle"))?.Name,
            "the reply saved as a draft reopened as something else: its save did not carry the message it answers (docs/drafts.md)");
    }, Subject("reply"));

    [HarnessFact]
    public void ADoubleClickOnADraftsRowResumesItAndOpensNoReadingWindow() => Case(nameof(ADoubleClickOnADraftsRowResumesItAndOpensNoReadingWindow), (app, flow) =>
    {
        var subject = Subject("window");
        flow.NewSavedDraft(subject);
        flow.CloseDraftComposer();
        if (!flow.WaitDraftRow(subject))
        {
            throw new InvalidOperationException($"Drafts does not list '{subject}'");
        }
        app.ShowMailbox();
        Input.Click(app.RowTitled(subject), times: 2);
        Assert.True(app.Agent.Wait(Filter.ById("SendButton"), App.Patience) is not null,
            "a double-click on a Drafts row opened no composer, which is where a draft opens");
        // A window would open on the second press; give it the time it takes before counting.
        app.Agent.Settle(floor: TimeSpan.FromSeconds(1));
        var windows = app.ExtraWindows();
        Assert.True(windows.Count == 0,
            $"a double-click on a Drafts row also opened a window ({string.Join(" | ", windows.Select(w => w.Title))}): the same draft is then open twice, once read-only beside the composer saving over it");
    }, Subject("window"));

    [HarnessFact]
    public void DiscardOverAStoredDraftAsksFirstAndTakesTheCopyOffTheServer() => Case(nameof(DiscardOverAStoredDraftAsksFirstAndTakesTheCopyOffTheServer), (app, flow) =>
    {
        var subject = Subject("discard");
        flow.NewSavedDraft(subject);
        flow.CloseDraftComposer();
        // Untouched since it opened, so only the stored copy makes the question due.
        flow.OpenStoredDraft(subject);
        Assert.True(app.CloseComposer(), "Discard over a draft with a copy in Drafts asked nothing; the copy is something to lose");
        Assert.True(flow.WaitDraftRow(subject, absent: true),
            "Discard left the draft in Drafts; it is the one path that removes the stored copy");
    }, Subject("discard"));

    [HarnessFact]
    public void AnAcceptedSendTakesTheStoredDraftAway() => Case(nameof(AnAcceptedSendTakesTheStoredDraftAway), (app, flow) =>
    {
        var subject = Subject("send");
        flow.NewSavedDraft(subject);
        if (!flow.WaitDraftRow(subject))
        {
            throw new InvalidOperationException($"Drafts does not list '{subject}' before the send");
        }
        app.Agent.Invoke(app.Agent.Find(Filter.ById("SendButton", "Button"))!);
        app.Agent.WaitGone(Filter.ById("SendButton"), TimeSpan.FromSeconds(10));
        Assert.True(flow.WaitDraftRow(subject, absent: true),
            "the message went out and its draft is still in Drafts: the submit did not name its composition, or the composer forgot it before the send could");
    }, Subject("send"));

    [HarnessFact]
    public void APauseAfterTypingInTheBodySavesTheDraftOnItsOwn() => Case(nameof(APauseAfterTypingInTheBodySavesTheDraftOnItsOwn), (app, flow) =>
    {
        // Nothing but the body is touched, so the editor's change count is the only thing that can
        // start the idle interval: a header field would start it by itself and prove nothing here.
        flow.OpenNewComposer();
        var editor = app.Agent.Find(Filter.ById("Editor")) ?? throw new InvalidOperationException("the composer has no editor");
        Input.BringToFront(editor.Window);
        Input.ClickAt((int)(editor.X + 60), (int)(editor.Y + 30));
        Input.Type("Written in the body and nowhere else");
        // The core's interval is 30 s and the host samples at a third of it, so a save is due
        // within 40 s of the last keystroke; the rest is the round trip.
        Assert.True(flow.WaitDraftSaved(timeoutSec: 50),
            $"nothing was saved 50 s after typing in the body (the hint reads '{flow.DraftHint()}'): the host is not sampling the editor's change count");
    });
}
