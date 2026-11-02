// What the core's row menu looks like on Windows, and the rules about expanding a conversation
// (docs/row-actions.md). The core decides which items a menu holds, which respond and which message
// each is about; this file gives each action its glyph and its batch, reads the selection bar off
// the same answer, and keeps no rule of its own about either.
//
// WinUI-free, so Mailcal.Tests links it. The labels are the view's (MailListView.RowMenu.cs),
// because L10n needs a Windows TFM that assembly does not have.

using System.Collections.Generic;
using System.Linq;
using uniffi.mailcal_bindings;

namespace Allodia.Mailcal.Services;

/// <summary>How each row menu action is drawn and run.</summary>
internal static class RowMenuPresentation
{
    /// <summary>The Segoe Fluent Icons glyph an item is drawn with (docs/icons.md).</summary>
    /// <remarks>The trailing arm draws an action this client does not know yet with no glyph,
    /// rather than refusing the menu: a C# switch over an enum is never exhaustive.</remarks>
    public static string Glyph(RowAction action) => action switch
    {
        RowAction.Open => "",
        RowAction.OpenInWindow => "",
        RowAction.Reply => "",
        RowAction.ReplyAll => "",
        RowAction.Forward => "",
        RowAction.MarkRead or RowAction.MarkUnread => "",
        RowAction.Flag or RowAction.Unflag => "",
        RowAction.MarkAsSpam => "",
        RowAction.MarkAsNotSpam => "",
        RowAction.MoveToFolder => "",
        RowAction.Archive => "",
        RowAction.Delete or RowAction.PermanentlyDelete => "",
        _ => string.Empty,
    };

    /// <summary>Whether the item is drawn in the critical colour: the one irreversible action.</summary>
    public static bool Critical(RowAction action) => action == RowAction.PermanentlyDelete;

    /// <summary>
    /// The batch an item without a target runs over its rows, or <c>null</c> for the actions that
    /// always name one message (open, answer, report) or ask for a folder first.
    /// </summary>
    public static BulkAction? Batch(RowAction action) => action switch
    {
        RowAction.MarkRead => BulkAction.MarkRead,
        RowAction.MarkUnread => BulkAction.MarkUnread,
        RowAction.Flag => BulkAction.Flag,
        RowAction.Unflag => BulkAction.Unflag,
        RowAction.Archive => BulkAction.Archive,
        RowAction.Delete => BulkAction.Delete,
        RowAction.PermanentlyDelete => BulkAction.PermanentlyDelete,
        _ => null,
    };
}

/// <summary>
/// The selection bar's buttons, read off the core's menu over the selection
/// (docs/list-selection.md, rule 5): each toggle names the half that changes something, and a
/// button the menu would draw disabled is disabled on the bar too.
/// </summary>
/// <remarks>
/// With nothing selected the core answers no menu, and the bar names the affirmative halves, all
/// disabled: "Mark as unread" over nothing describes an action nobody asked for.
/// </remarks>
internal sealed record SelectionBarActions(
    RowMenuItem Read,
    RowMenuItem Flag,
    RowMenuItem Archive,
    RowMenuItem Delete,
    RowMenuItem PermanentlyDelete)
{
    /// <summary>The bar over nothing.</summary>
    public static SelectionBarActions None { get; } = Of(new RowMenu([]));

    /// <summary>The bar <paramref name="menu"/> describes.</summary>
    public static SelectionBarActions Of(RowMenu menu)
    {
        var items = menu.Sections.SelectMany(section => section.Items).ToList();
        // The first item doing one of `actions`, which for a toggle is the half on offer.
        RowMenuItem Find(params RowAction[] actions) =>
            items.FirstOrDefault(item => actions.Contains(item.Action))
            ?? new RowMenuItem(actions[0], false, null);
        return new SelectionBarActions(
            Find(RowAction.MarkRead, RowAction.MarkUnread),
            Find(RowAction.Flag, RowAction.Unflag),
            Find(RowAction.Archive),
            Find(RowAction.Delete),
            Find(RowAction.PermanentlyDelete));
    }

    /// <summary>What the read button runs, which is what its label says.</summary>
    public BulkAction ReadAction =>
        Read.Action == RowAction.MarkUnread ? BulkAction.MarkUnread : BulkAction.MarkRead;

    /// <summary>What the flag button runs, which is what its label says.</summary>
    public BulkAction FlagAction =>
        Flag.Action == RowAction.Unflag ? BulkAction.Unflag : BulkAction.Flag;
}

/// <summary>Which conversations are expanded, and what selecting one opens (docs/row-actions.md,
/// rules 2 and 8).</summary>
internal static class ConversationExpansion
{
    /// <summary>
    /// The conversations still expanded, by row id, once the list's selection is
    /// <paramref name="selected"/>, out of <paramref name="expanded"/>.
    /// </summary>
    /// <remarks>
    /// One row picked keeps open only that row, so leaving a conversation folds it away, even one
    /// opened by its chevron. A message inside a conversation is not a row of the list here: opening
    /// one selects its conversation's row, so it keeps the conversation open. Several rows picked,
    /// or none, change nothing: adding a conversation to a set expands nothing, and a selection
    /// emptied by an archive is not the user leaving anything.
    /// </remarks>
    public static IReadOnlySet<string> Kept(IReadOnlySet<string> expanded, IReadOnlyList<string> selected) =>
        selected.Count == 1
            ? expanded.Where(id => id == selected[0]).ToHashSet()
            : expanded;

    /// <summary>
    /// Which of a conversation's messages, newest first by <paramref name="keys"/>, selecting it
    /// opens: the newest, whatever its kind, or in Drafts the newest draft, which is the message its
    /// row summarises (<paramref name="latestKey"/>). <c>null</c> for a conversation the list carries
    /// no messages of.
    /// </summary>
    public static int? Representative(IReadOnlyList<string> keys, string latestKey, bool showingDrafts)
    {
        if (keys.Count == 0)
        {
            return null;
        }
        if (!showingDrafts)
        {
            return 0;
        }
        var at = keys.ToList().IndexOf(latestKey);
        return at < 0 ? 0 : at;
    }
}
