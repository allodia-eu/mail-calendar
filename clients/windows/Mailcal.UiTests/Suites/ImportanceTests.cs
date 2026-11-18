// A message's importance on Windows (docs/importance.md): the mark a row announces, the banner over
// an open message, and the composer's choice surviving a save and a reopen.
//
// WHY HERE AND NOT IN `Mailcal.Tests`. Which importance each surface shows is the core's, pinned in
// mailcal-viewmodel and mailcal-app; what only a running app shows is the wiring: the row's
// accessible name, the conversation's inner message icons named for a screen reader, the InfoBar
// opened from the reading snapshot, and the composer's menu writing through the core.
//
// WHY THE HARNESS. The seed carries one high and one low message (`docker/stalwart/seed/mail`, 20 and 21), and
// a draft's importance is a mail write. The conversation case imports its own two messages, and
// every case takes what it stored off the server again.

using Allodia.Mailcal.UiTests.Driver;
using Xunit;

namespace Allodia.Mailcal.UiTests.Suites;

[Collection(HarnessCollection.Name)]
[Trait("Dataset", "harness")]
public sealed class ImportanceTests(HarnessApp harness)
{
    private const string Raised = "Contract needs your signature today";
    private const string Lowered = "Office plants, whenever you have a moment";
    private const string Plain = "Harness baseline message";

    private static readonly string Inbox = Catalog.English("folder_inbox");
    private static readonly string HighMark = Catalog.English("a11y_importance_high");
    private static readonly string LowMark = Catalog.English("a11y_importance_low");

    private static string Subject(string what) => $"Importance suite {DraftsFlow.Run} {what}";

    [HarnessFact]
    public void ARaisedAndALoweredMessageAnnounceTheirImportanceOnTheirRows() => harness.Case(nameof(ARaisedAndALoweredMessageAnnounceTheirImportanceOnTheirRows), app =>
    {
        app.OpenFolder(Inbox);
        var raised = app.RowTitled(Raised).Name;
        Assert.True(raised.Contains($", {HighMark}", StringComparison.Ordinal),
            $"the high-importance row announces '{raised}', without '{HighMark}' (docs/importance.md, rule 2)");
        var lowered = app.RowTitled(Lowered).Name;
        Assert.True(lowered.Contains($", {LowMark}", StringComparison.Ordinal),
            $"the low-importance row announces '{lowered}', without '{LowMark}': it carries X-Priority alone, so the engine's fallback or the row's wiring is missing");
        var plain = app.RowTitled(Plain).Name;
        Assert.False(plain.Contains(HighMark, StringComparison.Ordinal) || plain.Contains(LowMark, StringComparison.Ordinal),
            $"a normal row announces an importance: '{plain}'");
    });

    [HarnessFact]
    public void AnOpenMessageSaysInABannerWhenItWasSentHighOrLow() => harness.Case(nameof(AnOpenMessageSaysInABannerWhenItWasSentHighOrLow), app =>
    {
        app.OpenFolder(Inbox);
        foreach (var (subject, key) in new[] { (Raised, "reading_importance_high"), (Lowered, "reading_importance_low") })
        {
            app.OpenMessage(subject);
            var said = Catalog.English(key);
            Assert.True(app.Agent.Wait(new Filter { Name = said, Window = app.Mailbox.Hwnd, OnScreen = true }, TimeSpan.FromSeconds(10)) is not null,
                $"'{subject}' opened without the banner '{said}' (docs/importance.md, rule 4)");
        }
        app.OpenMessage(Plain);
        Assert.True(app.Agent.WaitGone(Filter.ById("ReadingImportance") with { Window = app.Mailbox.Hwnd, OnScreen = true }, TimeSpan.FromSeconds(10)),
            "a normal message opened with the importance banner still up: the banner of the message before it was left open");
    });

    [HarnessFact]
    public void AConversationIsHighWhileAnyMessageInItIsAndEachMessageCarriesItsOwn()
    {
        var conversation = Subject("conversation");
        harness.Case(nameof(AConversationIsHighWhileAnyMessageInItIsAndEachMessageCarriesItsOwn), app =>
        {
            // Bob raises it, Alice answers at normal from Sent.
            Import(Message($"Bob Fixture <{Harness.Bob}>", Harness.Alice, conversation, "high", "Importance: High"), "inbox");
            Import(Message($"Alice Fixture <{Harness.Alice}>", Harness.Bob, $"Re: {conversation}", "reply", $"In-Reply-To: <importance-high-{DraftsFlow.Run}@test.local>\r\nReferences: <importance-high-{DraftsFlow.Run}@test.local>"), "sent");

            app.OpenFolder(Inbox);
            app.Agent.Invoke(app.Agent.Find(Filter.ById("CommandSync", "Button"))!);
            Element? row = null;
            if (!App.Eventually(() => (row = Listed(app, conversation)) is not null, TimeSpan.FromSeconds(30)))
            {
                throw new InvalidOperationException("the imported conversation never reached the list");
            }
            Assert.True(app.Agent.FindAll(new Filter { Type = "Text", Under = row }).Any(t => t.Name == "2"),
                $"'{conversation}' is a conversation of two, the reply in Sent included; did the harness thread it?");
            Assert.True(row!.Name.Contains($", {HighMark}", StringComparison.Ordinal),
                $"the conversation announces '{row.Name}': it holds a high-importance message, and a normal reply must not hide it (docs/importance.md, rule 3)");

            // Opening it expands it, and each message inside is a button of its own.
            app.OpenMessage(conversation);
            var messages = app.Agent.FindAll(new Filter { Type = "Button", Under = app.RowTitled(conversation) }).Where(b => b.Rendered).ToList();
            Assert.True(messages.Count >= 2, $"the opened conversation shows {messages.Count} message button(s), not its two messages");
            var marked = messages.Count(m => app.Agent.Find(new Filter { Name = HighMark, Under = m }) is not null);
            Assert.True(marked == 1,
                $"{marked} of the conversation's messages announce '{HighMark}'; only Bob's was sent high, and a screen reader must hear it on that one alone");
        },
        _ => Harness.RemoveMail(Harness.Alice, conversation));
    }

    [HarnessFact]
    public void AnImportancePickedInTheComposerIsSavedWithTheDraftAndReopensWithIt()
    {
        var subject = Subject("draft");
        harness.Case(nameof(AnImportancePickedInTheComposerIsSavedWithTheDraftAndReopensWithIt), app =>
        {
            var flow = new DraftsFlow(app);
            flow.OpenNewComposer();
            flow.SetText("ToField", Harness.Bob);
            flow.SetText("SubjectBox", subject);
            Pick(app, Catalog.English("compose_importance_high"));
            Save(app, flow);
            flow.CloseDraftComposer();
            AssertDraftAnnounces(app, flow, subject, HighMark, "the draft was saved after High was picked");

            // Reopened, the menu marks what the draft was stored with; lowering it is a change of its own.
            flow.OpenStoredDraft(subject);
            var high = OpenMenu(app, Catalog.English("compose_importance_high"));
            Assert.True(app.Agent.Read(high).Selected == true,
                "the reopened draft's menu does not mark High: the composer opened at normal, and a save from here would lower the stored draft (docs/importance.md, rule 6)");
            app.Agent.Invoke(app.Agent.Find(MenuItem(Catalog.English("compose_importance_low")))!);
            Save(app, flow);
            flow.CloseDraftComposer();
            AssertDraftAnnounces(app, flow, subject, LowMark, "the reopened draft was saved after Low was picked");
        },
        app => new DraftsFlow(app).ClearStoredDraft(subject));
    }

    private static Filter MenuItem(string label) => new() { Type = "MenuItem", Name = label };

    /// <summary>Opens the composer's Importance menu and answers its entry <paramref name="label"/>.</summary>
    private static Element OpenMenu(App app, string label)
    {
        var button = app.Agent.Wait(Filter.ById("ComposerImportance", "Button"), App.Patience)
            ?? throw new InvalidOperationException("the composer draws no Importance button");
        app.Agent.Invoke(button);
        return app.Agent.Wait(MenuItem(label), TimeSpan.FromSeconds(10))
            ?? throw new InvalidOperationException($"the Importance menu offers no '{label}'");
    }

    private static void Pick(App app, string label)
    {
        app.Agent.Invoke(OpenMenu(app, label));
        app.Agent.WaitGone(MenuItem(label), TimeSpan.FromSeconds(5));
    }

    private static void Save(App app, DraftsFlow flow)
    {
        var save = app.Agent.Find(Filter.ById("SaveDraftButton", "Button"))
            ?? throw new InvalidOperationException("the composer draws no Save as draft button");
        app.Agent.Invoke(save);
        if (!flow.WaitDraftSaved())
        {
            throw new InvalidOperationException($"Save as draft never reported the draft saved; the hint reads '{flow.DraftHint()}'");
        }
    }

    /// <summary>Waits until the Drafts row for <paramref name="subject"/>, as the server lists it,
    /// announces <paramref name="mark"/>: the importance reached the stored copy, not only the
    /// composer.</summary>
    private static void AssertDraftAnnounces(App app, DraftsFlow flow, string subject, string mark, string when)
    {
        Assert.True(flow.WaitDraftRow(subject), $"Drafts does not list '{subject}' after {when}");
        var name = "";
        Assert.True(App.Eventually(() => (name = app.RowTitled(subject).Name).Contains($", {mark}", StringComparison.Ordinal), TimeSpan.FromSeconds(20)),
            $"the Drafts row announces '{name}' after {when}, without '{mark}': the save did not carry the composition's importance (docs/importance.md, rule 5)");
    }

    private static string Message(string from, string to, string subject, string id, string header) => string.Join("\r\n",
        $"From: {from}", $"To: {to}", $"Subject: {subject}", $"Date: {DateTime.UtcNow:r}",
        $"Message-ID: <importance-{id}-{DraftsFlow.Run}@test.local>", header,
        "MIME-Version: 1.0", "Content-Type: text/plain; charset=utf-8", "", "Importance fixture.", "");

    /// <summary>Imports one message into <paramref name="role"/>, a second after the last: a
    /// conversation is ordered by when its messages arrived.</summary>
    private static void Import(string message, string role)
    {
        Thread.Sleep(1100);
        Harness.ImportAsync(message, role).GetAwaiter().GetResult();
    }

    private static Element? Listed(App app, string subject)
    {
        try
        {
            return app.RowTitled(subject);
        }
        catch (InvalidOperationException)
        {
            return null;
        }
    }
}
