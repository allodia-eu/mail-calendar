// The message list's multi-selection half of MailboxModel (split out to keep each file under the
// 500-line limit): what is selected, what the bar over the list says, and the one batched intent
// every one of its buttons dispatches. The selection itself lives in the ListView, which is what
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

    /// <summary>Whether anything is selected, so the bar is shown.</summary>
    public bool HasSelection => _selectedRows.Length > 0;

    /// <summary>The bar's visibility, so the XAML needs no converter.</summary>
    public Visibility SelectionVisibility => HasSelection ? Visibility.Visible : Visibility.Collapsed;

    /// <summary>"N selected", the bar's own label.</summary>
    public string SelectionCountText => L10n.SelectionCount(SelectionCount);

    /// <summary>
    /// The label on the bar's read button. One button for the pair, never both: the useful one is
    /// whichever changes something, so any unread row makes it "Mark as read"
    /// (docs/list-selection.md, rule 5).
    /// </summary>
    public string SelectionReadText =>
        SelectionReadAction == BulkAction.MarkRead ? L10n.ActionMarkRead() : L10n.ActionMarkUnread();

    /// <summary>The label on the bar's flag button, on the same terms as the read one.</summary>
    public string SelectionFlagText =>
        SelectionFlagAction == BulkAction.Flag ? L10n.ActionFlag() : L10n.ActionUnflag();

    /// <summary>What the bar's read button dispatches, which is what its label says.</summary>
    internal BulkAction SelectionReadAction => SelectionActions.Read(_selectedRows);

    /// <summary>What the bar's flag button dispatches, which is what its label says.</summary>
    internal BulkAction SelectionFlagAction => SelectionActions.Flag(_selectedRows);

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
        Raise(nameof(SelectionVisibility));
        Raise(nameof(SelectionCountText));
        Raise(nameof(SelectionReadText));
        Raise(nameof(SelectionFlagText));
        Raise(nameof(SelectionReadAction));
        Raise(nameof(SelectionFlagAction));
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
    internal void ActOnSelection(BulkAction action)
    {
        if (_selectedRows.IsEmpty)
        {
            return;
        }
        var rows = SelectionActions.Rows(_selectedRows);
        var closesReading = SelectionActions.Removes(action) && SelectionHoldsOpenMessage();
        _app?.Dispatch(new Intent.ActOnSelection(rows, action));
        if (closesReading)
        {
            CloseReading();
        }
    }

    private bool SelectionHoldsOpenMessage() =>
        OpenedMessage is { } opened
        && _selectedRows.Any(row => row.IsThread
            ? row.Messages.Any(m => m.Account == opened.Account && m.Key == opened.Key)
            : row.Account == opened.Account && row.Key == opened.Key);
}
