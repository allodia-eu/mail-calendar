// Draft a reply in the composer (docs/ai.md rules 17 to 24), on the showcase offering the writing
// assistant (MAILCAL_OFFER_AI), whose first account drafts in its seeded style and whose canned draft
// answers a reply at once: the record reaches Drafted with a summary and a checklist of all three
// kinds once the editor reports the draft in. What the control does, every tick and when to ask are
// the core's; what is pinned here is that the composer draws each record and sends each press, and
// that the draft travels through the editor and back. Every composer is discarded.

using Allodia.Mailcal.UiTests.Driver;
using Xunit;

namespace Allodia.Mailcal.UiTests.Suites;

[Collection(OfferAiShowcaseCollection.Name)]
[Trait("Dataset", "showcase")]
public sealed class ReplyDraftShowcaseTests(OfferAiShowcaseApp showcase)
{
    // A message to the first account, which drafts in "Work", and one to the second, which drafts in
    // no style.
    private const string ToFirst = "Welcome to Allodia Mail & Calendar";
    private const string ToSecond = "Welcome to your first week";
    private const ushort Home = 0x24;

    private void Case(string name, Action<App> body) =>
        showcase.Case(name, body, app =>
        {
            app.CloseDialogs();
            app.CloseComposer();
        });

    /// <summary>Opens a reply to <paramref name="title"/>, a row of the list on screen, in the pane
    /// and waits for its editor.</summary>
    internal static void Reply(App app, string title)
    {
        app.Agent.Invoke(app.RowTitled(title));
        app.Agent.Invoke(app.Agent.Wait(Filter.ById("ReadingReply") with { OnScreen = true }, App.Patience)
            ?? throw new InvalidOperationException("the reading pane never offered Reply"));
        new DraftsFlow(app).WaitEditorSeeded();
    }

    internal static Element Control(App app) =>
        app.Agent.Wait(Filter.ById("ComposerDraftReply", "Button"), TimeSpan.FromSeconds(10))
            ?? throw new InvalidOperationException("a reply's composer draws no Draft a reply while the writing assistant is offered");

    /// <summary>The button <paramref name="filter"/> names, once it is drawn: a question's buttons
    /// and the input's Draft arrive a layout after the title or the line a case waited for.</summary>
    internal static Element Pressable(App app, Filter filter, string why) =>
        app.Agent.Wait(filter, TimeSpan.FromSeconds(5)) ?? throw new InvalidOperationException(why);

    /// <summary>Opens the instruction's input from the control.</summary>
    internal static void OpenInput(App app)
    {
        app.Agent.Invoke(Control(app));
        Assert.True(app.Agent.Wait(new Filter { Id = "ComposerDraftInstruction" }, TimeSpan.FromSeconds(5)) is not null
            || app.Agent.Wait(new Filter { Id = "ComposerDraftHeld" }, TimeSpan.FromSeconds(1)) is not null, "the control opens the instruction's input");
    }

    /// <summary>Drafts with <paramref name="instruction"/> on a reply nobody has typed in, and waits
    /// for the card: with nothing of the person's above the quote, nothing is asked first.</summary>
    internal static void Draft(App app, string instruction = "")
    {
        OpenInput(app);
        app.Agent.SetText(app.Agent.Find(new Filter { Id = "ComposerDraftInstruction" })!, instruction);
        app.Agent.Invoke(Pressable(app, Filter.ById("ComposerDraftCreate", "Button"), "the input offers no Draft"));
        Assert.True(app.Agent.Wait(new Filter { Id = "ComposerDraftSummary" }, TimeSpan.FromSeconds(30)) is not null,
            "the draft went in and its card came: the editor reported it to the core");
        Assert.True(app.Agent.Find(Filter.Named(Catalog.English("composer_draft_replace_title"))) is null,
            "a fresh reply's Draft asks nothing about replacing");
    }

    private static Element? Item(App app, int index) => app.Agent.Find(Filter.ById($"ComposerDraftItem_{index}", "CheckBox"));

    [OfflineFact]
    public void TheControlSitsBesideSignatureAndOpensTheInstructionWithItsSuggestions() =>
        Case(nameof(TheControlSitsBesideSignatureAndOpensTheInstructionWithItsSuggestions), app =>
        {
            Reply(app, ToFirst);
            var control = Control(app);
            Assert.Equal(Catalog.English("composer_draft_reply"), control.Name);
            Assert.True(App.Eventually(() =>
                    app.Agent.Find(Filter.ById("SignatureButton", "Button")) is { Rendered: true } signature
                    && app.Agent.Find(Filter.ById("ComposerDraftReply", "Button")) is { Rendered: true } drawn
                    && drawn.X > signature.X, TimeSpan.FromSeconds(5)),
                "Draft a reply sits after Signature in the action row");
            OpenInput(app);
            var line = app.Agent.Find(new Filter { Id = "ComposerDraftInstruction" })!;
            foreach (var (suggestion, key) in new[] { ("Yes", "composer_draft_chip_yes"), ("No", "composer_draft_chip_no"),
                         ("MoreInformation", "composer_draft_chip_more_info"), ("Holding", "composer_draft_chip_later") })
            {
                Assert.Equal(Catalog.English(key), app.Agent.Find(Filter.ById($"ComposerDraftSuggestion_{suggestion}", "Button"))?.Name);
            }
            app.Agent.Invoke(app.Agent.Find(Filter.ById("ComposerDraftSuggestion_No", "Button"))!);
            Assert.True(App.Eventually(() => app.Agent.Read(line).Value == Catalog.English("composer_draft_chip_no"), TimeSpan.FromSeconds(5)),
                "a suggestion fills the line");
        });

    [OfflineFact]
    public void ADraftBringsTheCardWithItsSummaryAndAChecklistOfEachKind() =>
        Case(nameof(ADraftBringsTheCardWithItsSummaryAndAChecklistOfEachKind), app =>
        {
            Reply(app, ToFirst);
            Draft(app);
            Assert.Equal(Catalog.English("composer_draft_summary_heading"), app.Agent.Find(Filter.Named(Catalog.English("composer_draft_summary_heading"), "Text"))?.Name);
            // The gap is a mark the reply decides, read with its state and nothing to press; never a
            // checkbox held, which would read as unavailable.
            var gap = app.Agent.Find(Filter.ById("ComposerDraftItem_0", "Text")) ?? throw new InvalidOperationException("the card draws no mark for the gap");
            Assert.StartsWith(Catalog.English("composer_task_fill_in").Split('{')[0], gap.Name, StringComparison.Ordinal);
            Assert.True(app.Agent.Read(gap).Active == false, "the gap is open while the reply still carries it");
            Assert.True(Item(app, 0) is null, "the gap is no checkbox");
            var ticks = Enumerable.Range(1, 5).Select(index => Item(app, index)).OfType<Element>().ToList();
            Assert.True(ticks.Count == 2, $"the canned draft's checklist has a file and a task to tick; the card draws {ticks.Count}");
            Assert.True(ticks.All(tick => tick.Enabled), "a file and a task are the person's to tick");
        });

    [OfflineFact]
    public void ATickGoesToTheCoreAndComesBackDrawn() => Case(nameof(ATickGoesToTheCoreAndComesBackDrawn), app =>
    {
        Reply(app, ToFirst);
        Draft(app);
        app.Agent.Toggle(Item(app, 2) ?? throw new InvalidOperationException("the card has no task"));
        Assert.True(App.Eventually(() => Item(app, 2) is { } task && app.Agent.Read(task).Active == true, TimeSpan.FromSeconds(5)),
            "the task ticked is drawn ticked from the record");
    });

    [OfflineFact]
    public void SendAsksOnceWhileItemsAreOpenAndKeepEditingKeepsTheComposerForTheNextSend() =>
        Case(nameof(SendAsksOnceWhileItemsAreOpenAndKeepEditingKeepsTheComposerForTheNextSend), app =>
        {
            Reply(app, ToFirst);
            Draft(app);
            app.Agent.Invoke(app.Agent.Find(Filter.ById("SendButton", "Button"))!);
            var title = Catalog.English("composer_send_open_title");
            var question = app.Agent.Wait(Filter.Named(title), TimeSpan.FromSeconds(10)) ?? throw new InvalidOperationException("Send with open items did not ask");
            Assert.NotNull(question);
            app.Agent.Invoke(Pressable(app, Filter.Named(Catalog.English("composer_keep_editing"), "Button"), "the question offers no Keep editing"));
            Assert.True(app.Agent.WaitGone(Filter.Named(title), TimeSpan.FromSeconds(5)), "Keep editing closes the question");
            Assert.True(app.Agent.Find(Filter.ById("SendButton", "Button")) is not null, "and the composer stays");

            app.Agent.Invoke(app.Agent.Find(Filter.ById("SendButton", "Button"))!);
            Assert.True(app.Agent.WaitGone(Filter.ById("SendButton", "Button"), TimeSpan.FromSeconds(15)),
                "the question was answered, so the next Send sends without asking again");
            Assert.True(app.Agent.Find(Filter.Named(title)) is null, "and asked nothing on the way");
        });

    [OfflineFact]
    public void DraftingOverWhatThePersonWroteAsksFirst() => Case(nameof(DraftingOverWhatThePersonWroteAsksFirst), app =>
    {
        Reply(app, ToFirst);
        app.ClickToType(app.Agent.Find(Filter.ById("Editor")) ?? throw new InvalidOperationException("no editor"));
        Input.Chord(Home, Input.Control);
        foreach (var character in "Sounds good")
        {
            Input.Type(character.ToString());
            Thread.Sleep(40);
        }
        app.Agent.Settle(quiet: TimeSpan.FromMilliseconds(400), floor: TimeSpan.FromMilliseconds(600));
        var title = Catalog.English("composer_draft_replace_title");

        OpenInput(app);
        app.Agent.Invoke(Pressable(app, Filter.ById("ComposerDraftCreate", "Button"), "the input offers no Draft"));
        Assert.True(app.Agent.Wait(Filter.Named(title), TimeSpan.FromSeconds(10)) is not null, "Draft over the person's own words asks first");
        app.Agent.Invoke(Pressable(app, Filter.Named(Catalog.English("action_cancel"), "Button"), "the question offers no Cancel"));
        Assert.True(app.Agent.WaitGone(Filter.Named(title), TimeSpan.FromSeconds(5)), "Cancel closes the question");
        Assert.True(app.Agent.Find(new Filter { Id = "ComposerDraftSummary" }) is null && app.Agent.Find(new Filter { Id = "ComposerDrafting" }) is null,
            "and drafts nothing");

        OpenInput(app);
        app.Agent.Invoke(Pressable(app, Filter.ById("ComposerDraftCreate", "Button"), "the input offers no Draft"));
        Assert.True(app.Agent.Wait(Filter.Named(title), TimeSpan.FromSeconds(10)) is not null, "asked again on the next Draft");
        app.Agent.Invoke(Pressable(app, Filter.Named(Catalog.English("composer_draft_replace"), "Button"), "the question offers no Replace"));
        Assert.True(app.Agent.Wait(new Filter { Id = "ComposerDraftSummary" }, TimeSpan.FromSeconds(30)) is not null, "Replace drafts");
    });

    [OfflineFact]
    public void SendAnywayAnswersTheQuestionAndSends() => Case(nameof(SendAnywayAnswersTheQuestionAndSends), app =>
    {
        Reply(app, ToFirst);
        Draft(app);
        app.Agent.Invoke(app.Agent.Find(Filter.ById("SendButton", "Button"))!);
        var title = Catalog.English("composer_send_open_title");
        Assert.True(app.Agent.Wait(Filter.Named(title), TimeSpan.FromSeconds(10)) is not null, "Send with open items asks");
        app.Agent.Invoke(Pressable(app, Filter.Named(Catalog.English("composer_send_anyway"), "Button"), "the question offers no Send anyway"));
        Assert.True(app.Agent.WaitGone(Filter.ById("SendButton", "Button"), TimeSpan.FromSeconds(15)), "Send anyway sends, and the composer goes");
    });

    [OfflineFact]
    public void AnAccountThatDraftsInNoStyleSaysSoWithItsRemedyInPlaceOfTheLine() =>
        Case(nameof(AnAccountThatDraftsInNoStyleSaysSoWithItsRemedyInPlaceOfTheLine), app =>
        {
            Reply(app, ToSecond);
            OpenInput(app);
            var held = app.Agent.Wait(new Filter { Id = "ComposerDraftHeld" }, TimeSpan.FromSeconds(5))
                ?? throw new InvalidOperationException("the input does not say why it cannot draft");
            Assert.Equal(Catalog.English("ai_failure_no_style_for_account"), held.Name);
            Assert.True(app.Agent.Find(new Filter { Id = "ComposerDraftInstruction" }) is null, "the reason stands in place of the line");
            var remedy = app.Agent.Find(Filter.ById("ComposerDraftHeldRemedy", "Button")) ?? throw new InvalidOperationException("the reason offers no remedy");
            Assert.Equal(Catalog.English("ai_remedy_choose_style"), remedy.Name);
            app.Agent.Invoke(remedy);
            Assert.True(app.Agent.Wait(Filter.ById("SettingsCategory_writing_style"), TimeSpan.FromSeconds(10)) is not null,
                "the remedy opens Settings at Writing style, where an account's style is chosen");
        });

    [OfflineFact]
    public void UndoTakesTheDraftAndItsCardOutInOneStepAndRedoBringsBoth() =>
        Case(nameof(UndoTakesTheDraftAndItsCardOutInOneStepAndRedoBringsBoth), app =>
        {
            Reply(app, ToFirst);
            Draft(app);
            var body = app.Agent.Find(Filter.ById("Editor")) ?? throw new InvalidOperationException("no editor");
            app.ClickToType(body);
            Input.Chord(0x5A, Input.Control);
            Assert.True(app.Agent.WaitGone(new Filter { Id = "ComposerDraftSummary" }, TimeSpan.FromSeconds(10)),
                "one Ctrl+Z takes the draft out, and its card goes with it");
            Input.Chord(0x59, Input.Control);
            Assert.True(app.Agent.Wait(new Filter { Id = "ComposerDraftSummary" }, TimeSpan.FromSeconds(10)) is not null,
                "one Ctrl+Y brings the draft back, and its card with it");
        });
}
