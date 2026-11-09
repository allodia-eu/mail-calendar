// The message list's multi-selection half of MailboxModel (split out to keep each file under the
// 500-line limit): what is selected, the command bar over it (the core's, docs/command-bar.md), and
// the one batched intent a batch over it dispatches. The selection itself lives in the ListView,
// which is what gives Ctrl/Shift-click, Ctrl+A and the narrator's own selected state
// (docs/list-selection.md); this holds the projection the bar is drawn from and the rules the
// write follows.

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
    /// Whether anything is selected. The bar itself stands whether or not it is true, and which of
    /// its actions respond is the core's (<see cref="Bar"/>; docs/command-bar.md, rule 5).
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

    private static readonly CommandBar NoBar = new([], []);

    private CommandBar _bar = NoBar;

    /// <summary>
    /// The mail command bar over the selection, as the core decides it (docs/command-bar.md): which
    /// items stand on it, in which order, which half of each toggle is offered and which respond.
    /// </summary>
    internal CommandBar Bar => _bar;

    /// <summary>Raised when <see cref="Bar"/> changes, for the window that draws it.</summary>
    internal event Action? BarChanged;

    // What the bar was last read over: each selected row with the state the core's answer turns
    // on. A conversation's sub-rows are immutable and replaced only when one of them changes
    // (MailRow.CopyFrom), so the list itself stands for them.
    private ImmutableArray<(MailRow Row, bool Unread, bool Flagged, IReadOnlyList<ThreadMessageItem> Messages)> _barReadOver = [];

    // Whether the bar has been asked for at all: before the first answer it is drawn over nothing.
    private bool _barRead;

    /// <summary>
    /// Re-reads the bar from the core. Run when the selection changes and when a snapshot lands,
    /// since a flag or a read state the selection holds moves without the selection moving, and the
    /// folder decides whether Delete is Delete permanently. A snapshot that changed none of the
    /// selected rows asks nothing, because a Ctrl+A selection is the whole list and a sync lands many
    /// snapshots.
    /// </summary>
    private void RefreshSelectionBar()
    {
        var over = _selectedRows.Select(row => (row, row.Unread, row.Flagged, row.Messages)).ToImmutableArray();
        if (_barRead
            && over.Length == _barReadOver.Length
            && over.Zip(_barReadOver).All(pair => ReferenceEquals(pair.First.row, pair.Second.Row)
                && pair.First.Unread == pair.Second.Unread
                && pair.First.Flagged == pair.Second.Flagged
                && ReferenceEquals(pair.First.Messages, pair.Second.Messages)))
        {
            return;
        }
        _barRead = true;
        _barReadOver = over;
        var next = CommandBarOver(SelectionActions.Rows(_selectedRows));
        if (CommandBarPresentation.Same(next, _bar))
        {
            return;
        }
        _bar = next;
        BarChanged?.Invoke();
    }

    /// <summary>
    /// The bar over <paramref name="rows"/>, as the core decides it. Read from the published
    /// snapshot without waiting; the reading pane asks it over the message it shows, so its Delete
    /// is the bar's (docs/command-bar.md, rule 8).
    /// </summary>
    internal CommandBar CommandBarOver(SelectedRow[] rows)
    {
        try
        {
            return _app?.CommandBar(rows) ?? NoBar;
        }
        catch (Exception ex)
        {
            Log.Warn($"command bar failed: {ex.GetType().Name}");
            return NoBar;
        }
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
        var selected = SelectionActions.Rows(rows);
        var removes = SelectionActions.Removes(action);
        var closesReading = removes && HoldsOpenMessage(selected);
        _app?.Dispatch(new Intent.ActOnSelection(selected, action));
        if (closesReading)
        {
            CloseReading();
        }
        if (removes)
        {
            // A conversation leaving the folder takes its expansion with it, so it does not come
            // back open when it is listed again.
            foreach (var row in rows.Where(row => row.IsThread))
            {
                _expandedThreads.Remove(row.Id);
                row.IsExpanded = false;
            }
        }
    }
}
