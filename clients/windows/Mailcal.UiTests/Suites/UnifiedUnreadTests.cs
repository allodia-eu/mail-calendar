// All Accounts › Inbox says what the account Inboxes under it say, as the count moves
// (docs/folder-pane.md rule 7). The app-icon badge is written from the same number
// (docs/app-badge.md), so a row left behind is an icon and a pane that disagree.
//
// WHY A MOVE. The showcase pins the sum as a launch draws it (FolderPaneTests); what can go stale
// is the redraw. The unified row is redrawn with the account rows, so a redraw that runs before the
// snapshot's sum has been read leaves it one snapshot behind until something unrelated redraws the
// pane.
//
// WHY THE HARNESS. The showcase cannot change a message's read state. Every message the harness
// seeds is read, so the case marks one unread and read again, and leaves the mailbox as it found it.

using Allodia.Mailcal.UiTests.Driver;
using Xunit;

namespace Allodia.Mailcal.UiTests.Suites;

[Collection(ImapHarnessCollection.Name)]
[Trait("Dataset", "harness")]
public sealed class UnifiedUnreadTests(ImapHarnessApp harness)
{
    private const string Subject = "Weekend walk";
    private static readonly string Inbox = Catalog.English("folder_inbox");

    private static Element Under(App app, string parent) =>
        app.Agent.Find(Filter.Named(Inbox, "ListItem") with { Under = app.Agent.Find(Filter.Named(parent, "ListItem") with { Window = app.Mailbox.Hwnd }) })
        ?? throw new InvalidOperationException($"the pane shows no Inbox under '{parent}'");

    private static string AccountInbox(App app) => app.PaneUnread(Under(app, Harness.Alice));

    private static string UnifiedInbox(App app) => app.PaneUnread(Under(app, Catalog.English("sidebar_all_accounts")));

    /// <summary>Presses the toolbar's read toggle and waits for alice's Inbox to read
    /// <paramref name="expect"/>, which is the snapshot the unified row has to agree with.</summary>
    private static void SwitchReadState(App app, string expect)
    {
        app.Agent.Invoke(app.Agent.Find(Filter.ById("SelectionMarkRead", "Button")) ?? throw new InvalidOperationException("the toolbar has no read toggle"));
        if (!App.Eventually(() => AccountInbox(app) == expect, TimeSpan.FromSeconds(15)))
        {
            throw new InvalidOperationException($"alice's Inbox never read '{expect}'; it reads '{AccountInbox(app)}'");
        }
    }

    [HarnessFact]
    public void TheUnifiedInboxCountFollowsTheAccountInboxBothWays()
    {
        var oneUnread = Catalog.English("a11y_unread_count_one").Replace("{count}", "1");
        harness.Case(nameof(TheUnifiedInboxCountFollowsTheAccountInboxBothWays), app =>
        {
            app.Agent.Invoke(app.RowTitled($"*{Subject}*"));
            Assert.Equal("", UnifiedInbox(app));

            SwitchReadState(app, oneUnread);
            App.Eventually(() => UnifiedInbox(app) == oneUnread, TimeSpan.FromSeconds(3));
            Assert.Equal(oneUnread, UnifiedInbox(app));

            SwitchReadState(app, "");
            App.Eventually(() => UnifiedInbox(app) == "", TimeSpan.FromSeconds(3));
            Assert.Equal("", UnifiedInbox(app));
        }, app =>
        {
            // A case that stopped between the two presses leaves the message unread.
            if (AccountInbox(app) == oneUnread)
            {
                SwitchReadState(app, "");
            }
        });
    }
}
