// How the Windows list reads the core's row menu, and the rules for which conversation stays
// expanded and what selecting one opens (docs/row-actions.md), without a window. All of them fail
// silently in the running app: a bar naming the half of a toggle that changes nothing does nothing
// when clicked, a conversation left open after the user moved on keeps its messages in a list they
// are no longer reading, and a conversation opened on its newest message in the folder rather than
// its newest message shows the reader something other than what Reply answers.

using Allodia.Mailcal.Services;
using uniffi.mailcal_bindings;
using Xunit;

namespace Allodia.Mailcal.Tests;

public class RowMenuPresentationTests
{
    private static RowMenuItem Item(RowAction action, bool enabled = true) => new(action, enabled, null);

    [Fact]
    public void TheBarOffersTheHalfOfEachToggleTheCoreNamed()
    {
        var menu = new RowMenu([
            new RowMenuSection([Item(RowAction.MarkUnread), Item(RowAction.Unflag)]),
            new RowMenuSection([Item(RowAction.Archive, enabled: false), Item(RowAction.Delete)]),
        ]);
        var bar = SelectionBarActions.Of(menu);
        Assert.Equal(BulkAction.MarkUnread, bar.ReadAction);
        Assert.Equal(BulkAction.Unflag, bar.FlagAction);
        Assert.False(bar.Archive.Enabled, "a move that would take nothing is disabled on the bar too");
        Assert.True(bar.Delete.Enabled);
        Assert.False(bar.PermanentlyDelete.Enabled, "an action the core did not offer is not on offer");
    }

    /// <summary>The bar stands with nothing picked, and the core answers no menu over no rows.
    /// "Mark as unread" over nothing is the wrong sentence to leave on screen.</summary>
    [Fact]
    public void AnEmptySelectionNamesTheAffirmativeActionsDisabled()
    {
        var bar = SelectionBarActions.Of(new RowMenu([]));
        Assert.Equal(BulkAction.MarkRead, bar.ReadAction);
        Assert.Equal(BulkAction.Flag, bar.FlagAction);
        Assert.All(
            new[] { bar.Read, bar.Flag, bar.Archive, bar.Delete, bar.PermanentlyDelete },
            button => Assert.False(button.Enabled));
        Assert.Equal(SelectionBarActions.None, bar);
    }

    [Fact]
    public void OnlyAnItemWithoutAMessageRunsTheBatch()
    {
        Assert.Equal(BulkAction.Archive, RowMenuPresentation.Batch(RowAction.Archive));
        Assert.Equal(BulkAction.Unflag, RowMenuPresentation.Batch(RowAction.Unflag));
        Assert.Equal(BulkAction.PermanentlyDelete, RowMenuPresentation.Batch(RowAction.PermanentlyDelete));
        foreach (var action in new[] { RowAction.Open, RowAction.Reply, RowAction.MarkAsSpam, RowAction.MoveToFolder })
        {
            Assert.Null(RowMenuPresentation.Batch(action));
        }
    }

    /// <summary>Rule 1: every action is drawn under its icon, and the one irreversible one stands
    /// out from Move to Trash beside it.</summary>
    [Fact]
    public void EveryActionHasAGlyphAndOnlyPermanentDeleteIsCritical()
    {
        foreach (var action in Enum.GetValues<RowAction>())
        {
            Assert.False(string.IsNullOrEmpty(RowMenuPresentation.Glyph(action)), $"{action} draws no glyph");
            Assert.Equal(action == RowAction.PermanentlyDelete, RowMenuPresentation.Critical(action));
        }
    }

    [Fact]
    public void LeavingAConversationFoldsItAway()
    {
        var expanded = new HashSet<string> { "t:acct-1:t1" };
        Assert.Empty(ConversationExpansion.Kept(expanded, ["m:acct-1:m1"]));
        Assert.Equal(expanded, ConversationExpansion.Kept(expanded, ["t:acct-1:t1"]));
    }

    /// <summary>Several rows picked, or none, is not the user leaving a conversation: adding one to
    /// a set expands nothing and folds nothing, and an archive that emptied the selection is not a
    /// move.</summary>
    [Fact]
    public void ASelectionOfSeveralOrNoneChangesNothing()
    {
        var expanded = new HashSet<string> { "t:acct-1:t1" };
        Assert.Equal(expanded, ConversationExpansion.Kept(expanded, ["m:acct-1:m1", "m:acct-1:m2"]));
        Assert.Equal(expanded, ConversationExpansion.Kept(expanded, []));
    }

    [Fact]
    public void OnlyTheConversationHoldingTheSelectionStaysOpen()
    {
        var both = new HashSet<string> { "t:acct-1:t1", "t:acct-1:t2" };
        Assert.Equal(["t:acct-1:t2"], ConversationExpansion.Kept(both, ["t:acct-1:t2"]));
    }

    /// <summary>Rule 2: selecting a conversation opens its newest message whatever its kind, a reply
    /// filed in Sent or a draft included, not the newest in the folder its row is summarised by.</summary>
    [Fact]
    public void AConversationOpensItsNewestMessage()
    {
        // Newest first: a reply filed in Sent, then the received message the Inbox row shows.
        Assert.Equal(0, ConversationExpansion.Representative(["sent", "received"], "received", showingDrafts: false));
    }

    /// <summary>In Drafts a conversation's row is its newest draft, and that is what it resumes
    /// (docs/drafts.md).</summary>
    [Fact]
    public void InDraftsAConversationOpensTheDraftItsRowSummarises()
    {
        Assert.Equal(1, ConversationExpansion.Representative(["received", "draft"], "draft", showingDrafts: true));
        Assert.Equal(0, ConversationExpansion.Representative(["received", "draft"], "gone", showingDrafts: true));
    }

    [Fact]
    public void AConversationWithNoMessagesListedOpensNothingOfItsOwn() =>
        Assert.Null(ConversationExpansion.Representative([], "latest", showingDrafts: false));
}
