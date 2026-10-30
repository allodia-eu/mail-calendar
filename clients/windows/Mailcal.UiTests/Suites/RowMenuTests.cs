// A row's menu, a conversation's flag and how a conversation expands (docs/row-actions.md), and a
// draft opening its composer from outside Drafts (docs/drafts.md), on the harness's own
// conversations: "Venue for the spring meeting", whose newest message was received, and
// "Contribution for next year", whose newest is a draft filed in Drafts.
//
// WHY HERE. Mailcal.Tests pins how the client reads the core's answer (RowMenuPresentationTests),
// but the menu is built in the flyout's Opening handler and run by each item's Click, neither of
// which is linked into that assembly: an unwired handler leaves the menu empty, or full and inert,
// with every unit test green. Which message an item reaches is the core's, proven live in
// live_row_menu.rs; this proves the client draws it and that a pick lands on the server.
//
// WHY THE HARNESS. A flag is a mail write, and the showcase engine performs none. The flag case
// clears what it set, so the suite leaves the mailbox as it found it.

using Allodia.Mailcal.UiTests.Driver;
using Xunit;

namespace Allodia.Mailcal.UiTests.Suites;

[Collection(HarnessCollection.Name)]
[TestCaseOrderer(DeclarationOrderer.Name, DeclarationOrderer.Assembly)]
[Trait("Dataset", "harness")]
public sealed class RowMenuTests(HarnessApp harness)
{
    private const string Received = "Re: Venue for the spring meeting";
    private const string Drafted = "Re: Contribution for next year";
    private const string Elsewhere = "Harness baseline message";

    private static readonly string Inbox = Catalog.English("folder_inbox");
    private static readonly string Flagged = Catalog.English("a11y_flagged");

    private void Case(string name, Action<App> body) => harness.Case(name, app =>
    {
        app.OpenFolder(Inbox);
        body(app);
    }, CloseMenu);

    private static Filter MenuItems(App app) => new() { Type = "MenuItem", Window = app.Mailbox.Hwnd };

    private static void CloseMenu(App app)
    {
        if (app.Agent.Find(MenuItems(app)) is not null)
        {
            Input.Press(Input.Escape);
            app.Agent.WaitGone(MenuItems(app), TimeSpan.FromSeconds(5));
        }
    }

    /// <summary>The conversation's subject as its header draws it. Pressed rather than the row's
    /// centre, because an expanded conversation's centre is one of its messages.</summary>
    private static Element Header(App app, string subject) =>
        app.Agent.Find(new Filter { Type = "Text", Under = app.RowTitled(subject), Name = subject })
            ?? throw new InvalidOperationException($"the '{subject}' row draws no subject");

    /// <summary>Right-clicks <paramref name="target"/> and returns the menu it raised, by label, left open.</summary>
    private static Dictionary<string, Element> Menu(App app, Element target)
    {
        CloseMenu(app);
        app.ShowMailbox();
        Input.Click(target, right: true);
        if (!App.Eventually(() => app.Agent.FindAll(MenuItems(app)).Count > 0, TimeSpan.FromSeconds(10)))
        {
            throw new InvalidOperationException("the right-click raised no menu");
        }
        app.Agent.Settle();
        return app.Agent.FindAll(MenuItems(app)).ToDictionary(item => item.Name);
    }

    private static string Label(string key) => Catalog.English(key);

    /// <summary>The messages an expanded conversation lays out under its header, as drawn.</summary>
    private static int DrawnMessages(App app, string subject) =>
        app.Agent.FindAll(new Filter { Type = "Button", Under = app.RowTitled(subject) }).Count(b => b.Rendered);

    private static bool RowSaysFlagged(App app, string subject) =>
        app.Agent.Read(app.RowTitled(subject)).Element.Name.Contains(Flagged, StringComparison.Ordinal);

    [HarnessFact]
    public void AConversationOffersWhatAMessageDoes() => Case(nameof(AConversationOffersWhatAMessageDoes), app =>
    {
        var menu = Menu(app, Header(app, Received));
        var labels = string.Join(", ", menu.Keys);
        foreach (var key in new[] { "action_open", "action_open_in_window", "action_reply", "action_reply_all", "action_forward", "action_flag", "action_mark_as_spam", "action_move_to_folder", "action_archive", "action_move_to_trash", "action_delete_permanently" })
        {
            Assert.True(menu.ContainsKey(Label(key)), $"the conversation's menu has no '{Label(key)}', got: {labels}");
        }
        Assert.True(menu[Label("action_reply")].Enabled, "the newest message was received, so it can be answered");
        Assert.True(menu[Label("action_mark_as_spam")].Enabled, "and reported");
        Assert.False(menu.ContainsKey(Label("thread_archive")), "the conversation's archive is the message menu's Archive");
    });

    /// <summary>Rule 6: a draft is not yet a message anyone sent, so it cannot be answered, and the
    /// item stays where it is, disabled, so the menu keeps its shape.</summary>
    [HarnessFact]
    public void AConversationWhoseNewestIsADraftDrawsReplyDisabled() => Case(nameof(AConversationWhoseNewestIsADraftDrawsReplyDisabled), app =>
    {
        var menu = Menu(app, Header(app, Drafted));
        Assert.True(menu.TryGetValue(Label("action_reply"), out var reply), $"Reply is left out rather than disabled, got: {string.Join(", ", menu.Keys)}");
        Assert.False(reply!.Enabled, "Reply over a conversation whose newest message is a draft responds");
    });

    /// <summary>Flagging a conversation marks its newest received message on the server, and the row
    /// draws the flag while any message on it is flagged, which is when its menu offers Clear flag.</summary>
    [HarnessFact]
    public void FlaggingAConversationDrawsItsFlagAndClearingItTakesItAway() => Case(nameof(FlaggingAConversationDrawsItsFlagAndClearingItTakesItAway), app =>
    {
        if (RowSaysFlagged(app, Received))
        {
            throw new InvalidOperationException($"'{Received}' is flagged before the case; was a run interrupted?");
        }
        var menu = Menu(app, Header(app, Received));
        app.Agent.Invoke(menu[Label("action_flag")]);
        Assert.True(App.Eventually(() => RowSaysFlagged(app, Received), TimeSpan.FromSeconds(15)), "flagging the conversation drew no flag on its row");
        // A sync puts the list back to what the server holds, so a flag only this client believed in
        // is gone after it.
        app.Agent.Invoke(app.Agent.Find(Filter.ById("SelectionSync", "Button"))!);
        app.Agent.Settle(floor: TimeSpan.FromSeconds(2));
        Assert.True(RowSaysFlagged(app, Received), "the flag did not survive a sync: it never reached the server");

        menu = Menu(app, Header(app, Received));
        Assert.True(menu.TryGetValue(Label("action_clear_flag"), out var clear), $"a flagged conversation offers no '{Label("action_clear_flag")}', got: {string.Join(", ", menu.Keys)}");
        app.Agent.Invoke(clear!);
        Assert.True(App.Eventually(() => !RowSaysFlagged(app, Received), TimeSpan.FromSeconds(15)), "clearing the flag left it drawn on the row");
    });

    /// <summary>Rule 5: a message inside an expanded conversation is a message the user named, with
    /// a message's own full menu.</summary>
    [HarnessFact]
    public void AMessageInsideAConversationOffersItsOwnMenu() => Case(nameof(AMessageInsideAConversationOffersItsOwnMenu), app =>
    {
        app.ShowMailbox();
        Input.Click(Header(app, Received));
        if (!App.Eventually(() => DrawnMessages(app, Received) > 1, TimeSpan.FromSeconds(10)))
        {
            throw new InvalidOperationException($"selecting '{Received}' laid out none of its messages");
        }
        var message = app.Agent.FindAll(new Filter { Type = "Button", Under = app.RowTitled(Received) }).Last(b => b.Rendered);
        var menu = Menu(app, message);
        var labels = string.Join(", ", menu.Keys);
        foreach (var key in new[] { "action_open_in_window", "action_reply", "action_forward", "action_archive", "action_move_to_trash" })
        {
            Assert.True(menu.ContainsKey(Label(key)), $"a message inside the conversation has no '{Label(key)}', got: {labels}");
        }
    });

    /// <summary>Rule 8: one conversation is expanded, the one holding the selection, so opening any
    /// other row folds it away.</summary>
    [HarnessFact]
    public void LeavingAConversationFoldsItAway() => Case(nameof(LeavingAConversationFoldsItAway), app =>
    {
        app.ShowMailbox();
        if (DrawnMessages(app, Received) == 0)
        {
            Input.Click(Header(app, Received));
        }
        if (!App.Eventually(() => DrawnMessages(app, Received) > 1, TimeSpan.FromSeconds(10)))
        {
            throw new InvalidOperationException($"selecting '{Received}' laid out none of its messages");
        }
        app.RowClicks(Elsewhere);
        Assert.True(App.Eventually(() => DrawnMessages(app, Received) == 0, TimeSpan.FromSeconds(5)),
            "the conversation stayed open after the user opened another row");
    });

    /// <summary>A draft opens into a composer wherever it is listed: this one is the newest message
    /// of a conversation in the Inbox, and selecting the conversation opens it (docs/drafts.md).</summary>
    [HarnessFact]
    public void ADraftInAConversationOpensItsComposerFromTheInbox() => Case(nameof(ADraftInAConversationOpensItsComposerFromTheInbox), app =>
    {
        app.ShowMailbox();
        Input.Click(Header(app, Drafted));
        Assert.True(app.Agent.Wait(Filter.ById("SendButton"), App.Patience) is not null,
            "selecting a conversation whose newest message is a draft opened no composer");
        var subject = app.Agent.Find(Filter.ById("SubjectBox"));
        Assert.True(subject is not null && app.Agent.Read(subject).Value == Drafted,
            "the composer does not hold the draft's own subject");
        // Left by opening another message, which keeps the draft where it is and asks nothing; a
        // Discard would take the harness's draft off the server.
        app.Agent.Invoke(app.RowTitled(Elsewhere));
        Assert.True(app.Agent.WaitGone(Filter.ById("SendButton"), TimeSpan.FromSeconds(15)),
            "the composer did not close when another message was opened");
    });
}
