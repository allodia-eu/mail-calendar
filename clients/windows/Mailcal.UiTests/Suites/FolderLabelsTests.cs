// A known folder is called what we call it, not what the server calls it (docs/folder-pane.md
// rule 12).
//
// THE FIXTURE is the harness account, and it has to be: the showcase seed already names its folders
// "Inbox" and "Sent", so it cannot tell a working rename from no rename at all. A real IMAP server
// names them `INBOX` (the one name the protocol mandates), `Sent Items`, `Junk Mail` and `Deleted
// Items`, which is what the user saw, and what this asserts is gone.
//
// The other half matters as much: a custom folder keeps its own name. Mapping by name instead of by
// role would either miss these renames or rename somebody's own folder.

using Allodia.Mailcal.UiTests.Driver;
using Xunit;

namespace Allodia.Mailcal.UiTests.Suites;

[Collection(HarnessCollection.Name)]
public sealed class FolderLabelsTests(HarnessApp harness)
{
    /// <summary>Every row the mailbox draws, by its name exactly: `INBOX` against `Inbox` is the
    /// fault, so nothing here may compare without case.</summary>
    private static HashSet<string> RowNames(App app)
    {
        // A launch lists the account before its folders arrive from the server.
        if (app.Agent.Wait(Filter.Named("Projects", "ListItem") with { Window = app.Mailbox.Hwnd }, App.Patience) is null)
        {
            throw new InvalidOperationException("the pane never listed alice's folders");
        }
        return app.Agent.FindAll(new Filter { Type = "ListItem", Window = app.Mailbox.Hwnd }).Select(row => row.Name).ToHashSet(StringComparer.Ordinal);
    }

    [HarnessFact]
    public void TheServerNamesOfKnownFoldersAreReplacedByOurOwn() => harness.Case(nameof(TheServerNamesOfKnownFoldersAreReplacedByOurOwn), app =>
    {
        var names = RowNames(app);
        foreach (var ours in new[] { "Inbox", "Drafts", "Sent", "Junk", "Trash" })
        {
            Assert.True(names.Contains(ours), $"the pane calls it '{ours}'");
        }
        foreach (var theirs in new[] { "INBOX", "Sent Items", "Junk Mail", "Deleted Items" })
        {
            Assert.False(names.Contains(theirs), $"the server's '{theirs}' is not what the user reads");
        }
    });

    [HarnessFact]
    public void AFolderTheUserMadeKeepsTheNameTheUserGaveIt() => harness.Case(nameof(AFolderTheUserMadeKeepsTheNameTheUserGaveIt), app =>
    {
        // The seeded custom folders. If these start reading "Folder" or an app word, the mapping has
        // stopped keying on the role, which is the failure that renames real mail.
        var names = RowNames(app);
        foreach (var theirs in new[] { "Projects", "QResync", "Idle" })
        {
            Assert.True(names.Contains(theirs), $"'{theirs}' is the user's folder and keeps its name");
        }
    });
}
