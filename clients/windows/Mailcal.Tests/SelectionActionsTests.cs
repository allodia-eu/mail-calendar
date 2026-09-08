// The selection bar's two decisions (docs/list-selection.md): which of the paired actions it
// offers, and what a conversation row becomes when the batch goes to the core.
//
// Both fail silently in the running app. A button labelled "Mark as unread" over a selection that
// is already unread does nothing when clicked, and a conversation sent as its latest message
// archives one reply while the rest of the thread stays in the inbox.

using Allodia.Mailcal.Services;
using uniffi.mailcal_bindings;
using Xunit;

namespace Allodia.Mailcal.Tests;

public class SelectionActionsTests
{
    private sealed record Row(
        string Account,
        string Key,
        bool IsThread,
        bool Unread,
        bool Flagged) : ISelectableRow;

    private static Row Message(bool unread = false, bool flagged = false, string key = "m1") =>
        new("acct-1", key, IsThread: false, unread, flagged);

    private static Row Thread(bool unread = false, string key = "t1") =>
        new("acct-1", key, IsThread: true, unread, Flagged: false);

    /// <summary>
    /// A plain click selects a row *and* opens it, so a pane stating "1 selected" would replace
    /// the message the user just asked to read with a count of it.
    /// </summary>
    [Fact]
    public void TheReadingPaneStatesTheCountOnlyAboveOneRow()
    {
        Assert.Equal(SelectionPaneLabel.None, SelectionActions.PaneLabel(0, threaded: false));
        Assert.Equal(SelectionPaneLabel.None, SelectionActions.PaneLabel(1, threaded: false));
        Assert.Equal(SelectionPaneLabel.None, SelectionActions.PaneLabel(1, threaded: true));
    }

    /// <summary>The rows are called what the list they were picked in calls them.</summary>
    [Fact]
    public void TheCountNamesRowsAsTheListDoes()
    {
        Assert.Equal(SelectionPaneLabel.Messages, SelectionActions.PaneLabel(2, threaded: false));
        Assert.Equal(
            SelectionPaneLabel.Conversations,
            SelectionActions.PaneLabel(2, threaded: true));
    }

    [Fact]
    public void ARowMenuOnASelectedRowCoversTheWholeSelection()
    {
        var selected = new[] { Message(key: "m1"), Message(key: "m2") };
        Assert.True(SelectionActions.Covers(selected, Message(key: "m2")));
    }

    [Fact]
    public void ARowMenuOnAnUnselectedRowIsAboutThatRowAlone()
    {
        var selected = new[] { Message(key: "m1"), Message(key: "m2") };
        Assert.False(SelectionActions.Covers(selected, Message(key: "m3")));
        Assert.False(SelectionActions.Covers([], Message(key: "m1")));
    }

    [Fact]
    public void AThreadRowIsNotTheMessageRowThatSharesItsKey()
    {
        // The two travel to the core as different shapes, so a conversation whose id happens to
        // equal a message key must not make that message's menu act on the selection.
        Assert.False(SelectionActions.Covers([Thread(key: "x")], Message(key: "x")));
        Assert.True(SelectionActions.Covers([Thread(key: "x")], Thread(key: "x")));
    }

    [Fact]
    public void TheSameKeyInAnotherAccountIsADifferentRow()
    {
        var selected = new[] { Message(key: "m1") };
        var elsewhere = new Row("acct-2", "m1", IsThread: false, Unread: false, Flagged: false);
        Assert.False(SelectionActions.Covers(selected, elsewhere));
    }

    [Fact]
    public void ReadOffersMarkReadWhileAnythingSelectedIsUnread()
    {
        Assert.Equal(BulkAction.MarkRead, SelectionActions.Read([Message(unread: true), Message()]));
        Assert.Equal(BulkAction.MarkUnread, SelectionActions.Read([Message(), Message()]));
        Assert.Equal(BulkAction.MarkRead, SelectionActions.Read([Thread(unread: true)]));
    }

    [Fact]
    public void FlagOffersFlagWhileAnythingSelectedIsUnflagged()
    {
        Assert.Equal(BulkAction.Flag, SelectionActions.Flag([Message(flagged: true), Message()]));
        Assert.Equal(BulkAction.Unflag, SelectionActions.Flag([Message(flagged: true)]));
        // A conversation has no flag of its own, so flagging is what it can be asked for.
        Assert.Equal(BulkAction.Flag, SelectionActions.Flag([Thread()]));
    }

    [Fact]
    public void OnlyTheActionsThatEmptyTheRowRemoveIt()
    {
        Assert.True(SelectionActions.Removes(BulkAction.Archive));
        Assert.True(SelectionActions.Removes(BulkAction.Delete));
        Assert.True(SelectionActions.Removes(BulkAction.PermanentlyDelete));
        Assert.False(SelectionActions.Removes(BulkAction.MarkRead));
        Assert.False(SelectionActions.Removes(BulkAction.MarkUnread));
        Assert.False(SelectionActions.Removes(BulkAction.Flag));
        Assert.False(SelectionActions.Removes(BulkAction.Unflag));
    }

    [Fact]
    public void AConversationTravelsAsAThreadAndAMessageAsAMessage()
    {
        var rows = SelectionActions.Rows([Message(key: "m1"), Thread(key: "t1")]);

        var message = Assert.IsType<SelectedRow.Message>(rows[0]);
        Assert.Equal("acct-1", message.Account);
        Assert.Equal("m1", message.Key);

        var thread = Assert.IsType<SelectedRow.Thread>(rows[1]);
        Assert.Equal("acct-1", thread.Account);
        Assert.Equal("t1", thread.ThreadId);
    }

    [Fact]
    public void TheRowsKeepTheOrderTheListShowedThem()
    {
        var rows = SelectionActions.Rows(
            [Message(key: "m1"), Message(key: "m2"), Message(key: "m3")]);

        Assert.Equal(
            ["m1", "m2", "m3"],
            rows.Cast<SelectedRow.Message>().Select(row => row.Key));
    }
}
