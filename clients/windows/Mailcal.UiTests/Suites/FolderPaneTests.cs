// The folder pane as the user gets it (docs/folder-pane.md). The shaping rules are pinned in
// Mailcal.Tests against SidebarTree; whether the shape reaches a drawn NavigationView, with a name a
// screen reader can read and a badge really on screen, only a running window answers. A row given a
// Grid for its content once lost its automation name: on screen perfect, to a screen reader and to
// automation every folder row nameless.
//
// THE FIXTURE is the en showcase seed: two accounts, which is what makes the independence rules
// testable, and unread counts derived from its own seeded messages. The harness cannot stand in:
// every message it seeds is read, so every badge there is legitimately absent. The cases run in the
// order written and put the pane back as they found it.

using Allodia.Mailcal.UiTests.Driver;
using Xunit;

namespace Allodia.Mailcal.UiTests.Suites;

[Collection(ShowcaseCollection.Name)]
[Trait("Dataset", "showcase")]
[TestCaseOrderer(DeclarationOrderer.Name, DeclarationOrderer.Assembly)]
public sealed class FolderPaneTests(ShowcaseApp showcase)
{
    private const string First = "eva.jansen@example.com"; // Inbox (4 unread), Drafts, Sent, Archive
    private const string Second = "eva@northwind.example"; // Inbox (1 unread), Sent, seeded empty
    private static readonly string AllAccounts = Catalog.English("sidebar_all_accounts");
    private static readonly string Inbox = Catalog.English("folder_inbox");

    /// <summary>Every pane row with that exact name, in pane order: three Inboxes, the group's
    /// unified one and then one per account.</summary>
    private static List<Element> PaneRows(App app, string name) =>
        app.Agent.FindAll(Filter.Named(name, "ListItem") with { Window = app.Mailbox.Hwnd }).ToList();

    private static Element PaneRow(App app, string name) =>
        PaneRows(app, name).FirstOrDefault() ?? throw new InvalidOperationException($"the pane has no row named '{name}'");

    /// <summary>Whether a row is on screen. UI Automation keeps a shut tree's rows, findable by name
    /// and offscreen; the agent's tree drops them. Either way presence alone reads a shut tree as open.</summary>
    private static bool Shown(Element row) => !row.Offscreen && row.Width > 0;

    private static int OnScreen(App app, string name) => PaneRows(app, name).Count(Shown);

    /// <summary>The unified Inbox, looked up inside the group's own row: every account's inbox is
    /// called the same, and a shut group takes its child out of view.</summary>
    private static Element? UnifiedInbox(App app) =>
        app.Agent.Find(Filter.Named(Inbox, "ListItem") with { Under = PaneRow(app, AllAccounts) });

    /// <summary>
    /// The unread sentence on a row, or null when it carries no badge. Off the row's own subtree:
    /// an account row's holds its folder rows, and reading through them reports the Inbox's count
    /// as a roll-up that is not on screen.
    /// </summary>
    private static string? Unread(App app, Element row)
    {
        var nested = app.Agent.FindAll(new Filter { Type = "ListItem", Under = row })
            .SelectMany(item => app.Agent.FindAll(new Filter { Under = item }).Prepend(item))
            .Select(e => e.Handle).ToHashSet();
        return app.Agent.FindAll(new Filter { Under = row })
            .Where(e => !nested.Contains(e.Handle) && e.Name.EndsWith("unread", StringComparison.Ordinal))
            .Select(e => e.Name).FirstOrDefault();
    }

    private static string Header(App app) => app.Agent.Find(Filter.ById("MailboxHeader") with { Window = app.Mailbox.Hwnd })?.Name ?? "";

    private static string Expanded(App app, Element row) => app.Agent.Read(row).Expanded ?? "";

    /// <summary>Opens a pane row and waits for the list's header to name it: the snapshot arrives
    /// after the press.</summary>
    private static void Open(App app, Element row, string expect)
    {
        app.Agent.Invoke(row);
        App.Eventually(() => Header(app) == expect, TimeSpan.FromSeconds(10));
        app.Agent.Settle(quiet: TimeSpan.FromMilliseconds(300), floor: TimeSpan.FromMilliseconds(400));
    }

    private static void Settle(App app) => app.Agent.Settle(quiet: TimeSpan.FromMilliseconds(300), floor: TimeSpan.FromMilliseconds(600));

    [ShowcaseFact]
    public void EveryRowInThePaneAnnouncesItsOwnName() => showcase.Case(nameof(EveryRowInThePaneAnnouncesItsOwnName), app =>
    {
        foreach (var name in new[] { AllAccounts, First, Second, Catalog.English("folder_drafts"), "Archive" })
        {
            PaneRow(app, name);
        }
        Assert.Equal(3, PaneRows(app, Inbox).Count);
    });

    [ShowcaseFact]
    public void BothAccountsShowTheirFoldersWhileNeitherIsSelected() => showcase.Case(nameof(BothAccountsShowTheirFoldersWhileNeitherIsSelected), app =>
    {
        // Under the old rule, expansion was selection, and at most one account showed its folders.
        Assert.Equal(2, PaneRows(app, Catalog.English("folder_sent")).Count);
        Assert.True(Shown(PaneRow(app, Catalog.English("folder_drafts"))), "the first account, which nothing selected, still shows its folders");
    });

    [ShowcaseFact]
    public void TheUnreadCountReadsAsASentenceOnTheFolderAndOnTheUnifiedInbox() => showcase.Case(nameof(TheUnreadCountReadsAsASentenceOnTheFolderAndOnTheUnifiedInbox), app =>
    {
        // "Inbox, 4" read aloud is a position in a list; the badge carries the words. The one after
        // the group's is the first account's.
        Assert.Equal("4 unread", Unread(app, PaneRows(app, Inbox)[1]));
        // The roll-up sums both accounts' inboxes, on the Inbox under the group, never the heading (rule 8).
        Assert.Equal("5 unread", Unread(app, UnifiedInbox(app)!));
        Assert.Null(Unread(app, PaneRow(app, AllAccounts)));
    });

    [ShowcaseFact]
    public void AFolderWithNothingUnreadCarriesNoBadgeAtAll() => showcase.Case(nameof(AFolderWithNothingUnreadCarriesNoBadgeAtAll), app =>
    {
        // Zero is not drawn, which also covers a provider reporting no count (rule 6), and the
        // account row stays bare however much is unread beneath it (rule 8).
        Assert.Null(Unread(app, PaneRow(app, Catalog.English("folder_drafts"))));
        Assert.Null(Unread(app, PaneRow(app, First)));
    });

    [ShowcaseFact]
    public void ShuttingOneAccountLeavesTheOtherOneOpen() => showcase.Case(nameof(ShuttingOneAccountLeavesTheOtherOneOpen), app =>
    {
        var account = PaneRow(app, First);
        Assert.Equal("Expanded", Expanded(app, account));
        app.Agent.Collapse(account);
        Settle(app);
        Assert.True(OnScreen(app, Catalog.English("folder_drafts")) == 0, "the shut account takes its folders with it");
        // Expansion is per account and shares nothing with the selection.
        Assert.True(Shown(PaneRow(app, Second)), "the other account is still there");
        Assert.True(Shown(PaneRow(app, AllAccounts)), "and the group above them is untouched");
        app.Agent.Expand(PaneRow(app, First));
        Settle(app);
        Assert.True(Shown(PaneRow(app, Catalog.English("folder_drafts"))), "reopening restores the tree");
    }, app => app.Agent.Expand(PaneRow(app, First)));

    [ShowcaseFact]
    public void TheUnifiedListIsAGroupOverAnInboxAndTheHeaderNamesTheInbox() => showcase.Case(nameof(TheUnifiedListIsAGroupOverAnInboxAndTheHeaderNamesTheInbox), app =>
    {
        // Rule 16: the user selects the Inbox under the heading, and rule 13 names the list after it.
        Assert.Equal("Expanded", Expanded(app, PaneRow(app, AllAccounts)));
        var unified = UnifiedInbox(app);
        Assert.True(unified is not null && Shown(unified), "its Inbox is on screen");
        Open(app, unified!, Inbox);
        Assert.Equal(Inbox, Header(app));
    });

    [ShowcaseFact]
    public void TheGroupsRowOpensAndShutsItsTreeRatherThanNavigating() => showcase.Case(nameof(TheGroupsRowOpensAndShutsItsTreeRatherThanNavigating), app =>
    {
        // Rule 17: a heading that navigated would claim an "every account" scope the core has none for.
        var before = Header(app);
        Assert.Equal(3, OnScreen(app, Inbox));
        app.Agent.Invoke(PaneRow(app, AllAccounts));
        Settle(app);
        Assert.Equal("Collapsed", Expanded(app, PaneRow(app, AllAccounts)));
        Assert.Equal(before, Header(app));
        // Counted, since the accounts' inboxes carry the same name and both must stay.
        Assert.Equal(2, OnScreen(app, Inbox));
        Assert.True(Shown(PaneRow(app, First)), "the accounts below are untouched (rule 2)");
        app.Agent.Invoke(PaneRow(app, AllAccounts));
        Settle(app);
        Assert.Equal("Expanded", Expanded(app, PaneRow(app, AllAccounts)));
        Assert.Equal(3, OnScreen(app, Inbox));
    }, app => app.Agent.Expand(PaneRow(app, AllAccounts)));

    [ShowcaseFact]
    public void CollapsingThePaneIsNotTheUserShuttingATree() => showcase.Case(nameof(CollapsingThePaneIsNotTheUserShuttingATree), app =>
    {
        // A pane on its way to the icon strip takes every tree down with it through the same binding
        // a chevron uses; told, the core persisted it, and the pane came back with every account
        // shut for good (rules 2 and 3).
        Assert.Equal("Expanded", Expanded(app, PaneRow(app, First)));
        var toggle = app.Agent.Find(Filter.ById("PART_PaneToggleButton", "Button")) ?? throw new InvalidOperationException("no pane toggle in the caption");
        app.Agent.Invoke(toggle);
        Settle(app);
        app.Agent.Invoke(toggle);
        Settle(app);
        Assert.Equal("Expanded", Expanded(app, PaneRow(app, First)));
        Assert.True(Shown(PaneRow(app, Catalog.English("folder_drafts"))), "with its folders on screen");
        Assert.Equal(3, OnScreen(app, Inbox));
    });

    [ShowcaseFact]
    public void AFolderOpensEvenThoughNoAccountIsSelected() => showcase.Case(nameof(AFolderOpensEvenThoughNoAccountIsSelected), app =>
    {
        // From All Inboxes, a folder press once did nothing at all: the unified scope ignores a
        // folder, so the shell has to name its account alongside it.
        Open(app, UnifiedInbox(app)!, Inbox);
        var drafts = Catalog.English("folder_drafts");
        Open(app, PaneRow(app, drafts), drafts);
        Assert.Equal(drafts, Header(app));
        Assert.Single(app.Rows());
    }, app => Open(app, UnifiedInbox(app)!, Inbox));

    [ShowcaseFact]
    public void AnEmptyOutboxIsNotARowSayingZero() => showcase.Case(nameof(AnEmptyOutboxIsNotARowSayingZero), app =>
        // Rule 18: the Outbox is on the pane only while something is in it, and the showcase
        // queues nothing. The other half, its appearing with a queued message, needs a send that
        // cannot go out, which needs a server taken away: proven by hand and by the core's tests.
        Assert.Empty(PaneRows(app, Catalog.English("folder_outbox"))));

    [ShowcaseFact]
    public void AFolderKeyOpensTheMailboxOfTheAccountWhoseTreeItIsIn() => showcase.Case(nameof(AFolderKeyOpensTheMailboxOfTheAccountWhoseTreeItIsIn), app =>
    {
        // Both accounts key a folder `sent`, and only the account tells them apart (rule 14); the
        // second account's is empty and the first's is not, which makes the difference visible.
        var sentName = Catalog.English("folder_sent");
        var sent = PaneRows(app, sentName);
        Assert.Equal(2, sent.Count);
        Open(app, sent[1], sentName);
        Assert.True(App.Eventually(() => app.Rows().Count == 0, TimeSpan.FromSeconds(5)), "the second account's Sent is empty, as its seed is");
        Open(app, PaneRows(app, sentName)[0], sentName);
        Assert.True(App.Eventually(() => app.Rows().Count > 0, TimeSpan.FromSeconds(5)), "the first account's Sent, under the same key, holds its own mail");
    }, app => Open(app, UnifiedInbox(app)!, Inbox));
}
