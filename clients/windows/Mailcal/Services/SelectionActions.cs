// What the message list's multi-selection means: which of the paired actions its bar offers, and
// which rows a batch actually names (docs/list-selection.md).
//
// WinUI-free on purpose, so Mailcal.Tests can link it: both rules fail silently in the app. A bar
// offering "Mark as unread" over a selection that is already unread does nothing when clicked, and
// a conversation row sent as its latest message archives one reply and leaves the rest of the
// thread in the inbox, which reads as a working archive right up until the user scrolls.

using System.Collections.Generic;
using System.Linq;
using uniffi.mailcal_bindings;

namespace Allodia.Mailcal.Services;

/// <summary>The row facts a selection's actions are decided from. Implemented by MailRow.</summary>
public interface ISelectableRow
{
    /// <summary>The id of the account this row belongs to.</summary>
    string Account { get; }

    /// <summary>The message's provider key, or the thread id for a conversation row.</summary>
    string Key { get; }

    /// <summary>Whether this row is a conversation rather than a single message.</summary>
    bool IsThread { get; }

    /// <summary>Whether the row holds an unread message.</summary>
    bool Unread { get; }

    /// <summary>Whether the message is flagged. Always false for a conversation row.</summary>
    bool Flagged { get; }
}

/// <summary>What the reading pane says while rows are selected, as a closed set of labels.</summary>
/// <remarks>
/// An enum rather than the string itself: <c>L10n</c> needs a Windows TFM that Mailcal.Tests does
/// not have, so the decision is testable here and the wording is the model's.
/// </remarks>
internal enum SelectionPaneLabel
{
    /// <summary>Nothing to say: the pane shows whatever it holds.</summary>
    None,

    /// <summary>"N messages selected", the flat list's rows.</summary>
    Messages,

    /// <summary>"N conversations selected", the threaded list's rows.</summary>
    Conversations,
}

/// <summary>The selection bar's decisions.</summary>
/// <remarks>
/// Internal, not public: it speaks in the generated <c>BulkAction</c> and <c>SelectedRow</c>,
/// which UniFFI emits as internal types. <see cref="ISelectableRow"/> stays public because
/// MailRow implements it and carries only BCL types.
/// </remarks>
internal static class SelectionActions
{
    /// <summary>
    /// The action the bar's single read button runs: the one that changes something, so any
    /// unread row in the selection makes it "mark read".
    /// </summary>
    /// <remarks>
    /// An empty selection names the affirmative action rather than the one a test over no rows
    /// falls out to. The bar stands with nothing picked, and a disabled button reading "Mark as
    /// unread" describes an action nobody asked for.
    /// </remarks>
    public static BulkAction Read(IEnumerable<ISelectableRow> rows)
    {
        var listed = rows as IReadOnlyCollection<ISelectableRow> ?? rows.ToList();
        if (listed.Count == 0)
        {
            return BulkAction.MarkRead;
        }
        return listed.Any(row => row.Unread) ? BulkAction.MarkRead : BulkAction.MarkUnread;
    }

    /// <summary>
    /// The action the bar's single flag button runs, on the same terms. A conversation carries no
    /// flag of its own, so it counts as unflagged: flagging is what its rows can be asked for.
    /// </summary>
    public static BulkAction Flag(IEnumerable<ISelectableRow> rows)
    {
        var listed = rows as IReadOnlyCollection<ISelectableRow> ?? rows.ToList();
        if (listed.Count == 0)
        {
            return BulkAction.Flag;
        }
        return listed.Any(row => row.IsThread || !row.Flagged) ? BulkAction.Flag : BulkAction.Unflag;
    }

    /// <summary>
    /// What the reading pane states instead of showing what it holds, given how many rows are
    /// selected and which list they were picked in.
    /// </summary>
    /// <remarks>
    /// **More than one**, never one: a plain click both selects a row and opens it, so stating
    /// "1 selected" over the pane would replace the message the user just asked to read with a
    /// count of it. What the rows are called follows the list, since a threaded row stands for a
    /// whole conversation and a flat one for a single message.
    /// </remarks>
    public static SelectionPaneLabel PaneLabel(int selected, bool threaded)
    {
        if (selected <= 1)
        {
            return SelectionPaneLabel.None;
        }
        return threaded ? SelectionPaneLabel.Conversations : SelectionPaneLabel.Messages;
    }

    /// <summary>
    /// Whether a row menu opened on <paramref name="row"/> acts on the whole selection, which it
    /// does when that row is one of the selected ones (docs/list-selection.md, rule 12). A menu
    /// opened on a row outside the selection is about that row alone.
    /// </summary>
    public static bool Covers(IEnumerable<ISelectableRow> selected, ISelectableRow row) =>
        selected.Any(one =>
            one.IsThread == row.IsThread && one.Account == row.Account && one.Key == row.Key);

    /// <summary>
    /// Whether the action takes its rows out of the folder, so the reading pane must let go of a
    /// message that is in the batch. Read and flag change a message in place and do not.
    /// </summary>
    public static bool Removes(BulkAction action) =>
        action is BulkAction.Archive or BulkAction.Delete or BulkAction.PermanentlyDelete;

    /// <summary>
    /// The rows in the shape the core's batched intent takes. A conversation row travels as a
    /// thread, never as the message it happens to summarise: the core expands it from the store's
    /// thread index, which holds messages the list never showed.
    /// </summary>
    public static SelectedRow[] Rows(IEnumerable<ISelectableRow> rows) =>
        rows
            .Select(row => row.IsThread
                ? (SelectedRow)new SelectedRow.Thread(row.Account, row.Key)
                : new SelectedRow.Message(row.Account, row.Key))
            .ToArray();
}
