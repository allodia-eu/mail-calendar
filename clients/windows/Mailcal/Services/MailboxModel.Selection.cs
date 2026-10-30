// The message list's multi-selection half of MailboxModel (split out to keep each file under the
// 500-line limit): what is selected, what the bar over the list says (the core's row menu over the
// selection, docs/row-actions.md), and the one batched intent every one of its buttons dispatches. The selection itself lives in the ListView, which is what
// gives Ctrl/Shift-click, Ctrl+A and the narrator's own selected state (docs/list-selection.md);
// this holds the projection the bar binds to and the rules the write follows.

using System.Collections.Generic;
using System.Collections.Immutable;
using System.Linq;
using Allodia.Mailcal.ViewModels;
using Microsoft.UI.Xaml;
using uniffi.mailcal_bindings;

namespace Allodia.Mailcal.Services;

public sealed partial class MailboxModel
{
    private ImmutableArray<MailRow> _selectedRows = [];

    /// <summary>The rows the list currently has selected, in list order.</summary>
    public IReadOnlyList<MailRow> SelectedRows => _selectedRows;

    /// <summary>How many rows are selected; 0 hides the bar over the list.</summary>
    public int SelectionCount => _selectedRows.Length;

    /// <summary>
    /// Whether anything is selected, which is what every action on the bar is enabled by. The bar
    /// itself stands whether or not it is true (docs/list-selection.md, rule 5).
    /// </summary>
    public bool HasSelection => _selectedRows.Length > 0;

    /// <summary>
    /// The reading pane's own visibility for the count, so the XAML needs no converter. The bar
    /// carries no count: the pane is where the desktop states it (docs/list-selection.md,
    /// rule 10), and it says nothing at one row, which is the row the pane is already showing.
    /// </summary>
    public Visibility SelectionPaneVisibility =>
        SelectionActions.PaneLabel(SelectionCount, IsThreaded) == SelectionPaneLabel.None
            ? Visibility.Collapsed
            : Visibility.Visible;

    /// <summary>"N conversations selected", over whatever the reading pane was holding.</summary>
    public string SelectionPaneText =>
        SelectionActions.PaneLabel(SelectionCount, IsThreaded) == SelectionPaneLabel.Conversations
            ? L10n.SelectionSelectedConversations(SelectionCount)
            : L10n.SelectionSelectedMessages(SelectionCount);

    private SelectionBarActions _bar = SelectionBarActions.None;

    /// <summary>
    /// The bar's buttons as the core's menu over the selection decides them: which half of each
    /// toggle is offered and which respond (docs/list-selection.md, rule 5).
    /// </summary>
    internal SelectionBarActions SelectionBar => _bar;

    /// <summary>
    /// The label on the bar's read button. One button for the pair, never both: the useful one is
    /// whichever changes something, which the core answers.
    /// </summary>
    public string SelectionReadText =>
        _bar.ReadAction == BulkAction.MarkRead ? L10n.ActionMarkRead() : L10n.ActionMarkUnread();

    /// <summary>The label on the bar's flag button, on the same terms as the read one.</summary>
    public string SelectionFlagText =>
        _bar.FlagAction == BulkAction.Flag ? L10n.ActionFlag() : L10n.ActionUnflag();

    /// <summary>Whether the bar's read button responds.</summary>
    public bool SelectionReadEnabled => _bar.Read.Enabled;

    /// <summary>Whether the bar's flag button responds.</summary>
    public bool SelectionFlagEnabled => _bar.Flag.Enabled;

    /// <summary>Whether the bar's Archive responds: disabled when it would move nothing.</summary>
    public bool SelectionArchiveEnabled => _bar.Archive.Enabled;

    /// <summary>Whether the bar's Move to Trash responds.</summary>
    public bool SelectionDeleteEnabled => _bar.Delete.Enabled;

    /// <summary>Whether the bar's Delete permanently responds.</summary>
    public bool SelectionPermanentlyDeleteEnabled => _bar.PermanentlyDelete.Enabled;

    /// <summary>
    /// Re-reads the bar from the core. Run when the selection changes and when a snapshot lands,
    /// since a flag or a read state the selection holds moves without the selection moving.
    /// </summary>
    private void RefreshSelectionBar()
    {
        var next = _selectedRows.IsEmpty
            ? SelectionBarActions.None
            : SelectionBarActions.Of(RowMenu(SelectionActions.Rows(_selectedRows)));
        if (next == _bar)
        {
            return;
        }
        _bar = next;
        Raise(nameof(SelectionReadText));
        Raise(nameof(SelectionFlagText));
        Raise(nameof(SelectionReadEnabled));
        Raise(nameof(SelectionFlagEnabled));
        Raise(nameof(SelectionArchiveEnabled));
        Raise(nameof(SelectionDeleteEnabled));
        Raise(nameof(SelectionPermanentlyDeleteEnabled));
    }

    /// <summary>
    /// The menu over <paramref name="rows"/>, as the core decides it (docs/row-actions.md). Read
    /// from the published snapshot without waiting, so it is asked for as a menu opens.
    /// </summary>
    internal RowMenu RowMenu(SelectedRow[] rows)
    {
        try
        {
            return _app?.RowMenu(rows) ?? new RowMenu([]);
        }
        catch (Exception ex)
        {
            Log.Warn($"row menu failed: {ex.GetType().Name}");
            return new RowMenu([]);
        }
    }

    /// <summary>
    /// Whether <paramref name="row"/> is one of the selected rows, which is what decides whether a
    /// row menu opened on it acts on the whole selection (docs/list-selection.md, rule 12).
    /// </summary>
    internal bool SelectionCovers(MailRow row) => SelectionActions.Covers(_selectedRows, row);

    /// <summary>
    /// Records what the list has selected and re-labels the bar. Called from the view's
    /// SelectionChanged; the model keeps no selection the ListView does not have.
    /// </summary>
    public void SetSelection(IEnumerable<MailRow> rows)
    {
        var next = rows.ToImmutableArray();
        if (next.SequenceEqual(_selectedRows))
        {
            return;
        }
        _selectedRows = next;
        Raise(nameof(SelectedRows));
        Raise(nameof(SelectionCount));
        Raise(nameof(HasSelection));
        Raise(nameof(SelectionPaneVisibility));
        Raise(nameof(SelectionPaneText));
        RefreshSelectionBar();
        FoldConversationsLeft();
    }

    /// <summary>
    /// Folds every conversation the selection has left: one row picked keeps only that row
    /// expanded (docs/row-actions.md, rule 8), even a conversation opened by its chevron.
    /// </summary>
    private void FoldConversationsLeft()
    {
        var kept = ConversationExpansion.Kept(_expandedThreads, _selectedRows.Select(row => row.Id).ToList());
        if (kept.Count == _expandedThreads.Count)
        {
            return;
        }
        _expandedThreads.IntersectWith(kept);
        foreach (var row in Rows.Where(row => row.IsExpanded && !kept.Contains(row.Id)))
        {
            row.IsExpanded = false;
        }
    }

    /// <summary>
    /// Runs one action over every selected row as a single batch in the core: one hide, one sync
    /// per account, rather than one of each per row.
    /// </summary>
    /// <remarks>
    /// The reading pane is cleared when the message it holds is in the batch, rather than advanced
    /// the way a single-row archive is: the row it would advance to may be leaving in the same
    /// batch. Read/unread and flag take nothing out of a folder, so they leave the pane alone.
    /// </remarks>
    internal void ActOnSelection(BulkAction action) => ActOnRows(_selectedRows, action);

    /// <summary>
    /// The same batch over <paramref name="rows"/>, which a row menu opened outside the selection
    /// is about (docs/list-selection.md, rule 12).
    /// </summary>
    internal void ActOnRows(IReadOnlyList<MailRow> rows, BulkAction action)
    {
        if (rows.Count == 0)
        {
            return;
        }
        var closesReading = SelectionActions.Removes(action) && HoldsOpenMessage(rows);
        _app?.Dispatch(new Intent.ActOnSelection(SelectionActions.Rows(rows), action));
        if (closesReading)
        {
            CloseReading();
        }
    }

    private bool HoldsOpenMessage(IReadOnlyList<MailRow> rows) =>
        OpenedMessage is { } opened
        && rows.Any(row => row.IsThread
            ? row.Messages.Any(m => m.Account == opened.Account && m.Key == opened.Key)
            : row.Account == opened.Account && row.Key == opened.Key);
}
