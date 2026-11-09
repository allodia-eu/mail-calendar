// What the core's row menu looks like on Windows, and the rules about expanding a conversation
// (docs/row-actions.md). The core decides which items a menu holds, which respond and which message
// each is about; this file gives each action its glyph and its batch, draws the command bar off
// the same answer (docs/command-bar.md), and keeps no rule of its own about either.
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
/// How the core's command bar is drawn and run on Windows (docs/command-bar.md). The core decides
/// which items stand on it, in which order, which respond and which message each is about; this
/// gives each its slot and its glyph, and runs a pick through the row menu's own path.
/// </summary>
internal static class CommandBarPresentation
{
    /// <summary>
    /// The automation id of the slot an item stands in: one per slot, so a toggle keeps its id
    /// whichever half it offers and Delete keeps it in Trash.
    /// </summary>
    public static string AutomationId(Command command) => command switch
    {
        Command.Sync => "CommandSync",
        Command.Act { Action: RowAction.Delete or RowAction.PermanentlyDelete } => "CommandDelete",
        Command.Act { Action: RowAction.Archive } => "CommandArchive",
        Command.Act { Action: RowAction.MoveToFolder } => "CommandMove",
        Command.Act { Action: RowAction.Flag or RowAction.Unflag } => "CommandFlag",
        Command.Act { Action: RowAction.MarkRead or RowAction.MarkUnread } => "CommandRead",
        Command.Act { Action: RowAction.MarkAsSpam or RowAction.MarkAsNotSpam } => "CommandReport",
        Command.Act act => $"Command{act.Action}",
        _ => "Command",
    };

    /// <summary>The Segoe Fluent Icons glyph an item is drawn with: its action's, as in the row
    /// menu, and Sync's own.</summary>
    public static string Glyph(Command command) =>
        command is Command.Act act ? RowMenuPresentation.Glyph(act.Action) : "";

    /// <summary>
    /// The item as the row menu names it, so a pick from the bar runs exactly as the same pick in
    /// a row menu over the selection does; <c>null</c> for Sync, which acts on the mailbox.
    /// </summary>
    public static RowMenuItem? MenuItem(CommandItem item) =>
        item.Command is Command.Act act ? new RowMenuItem(act.Action, item.Enabled, item.Target, item.Confirm) : null;

    /// <summary>
    /// The item standing in Delete's place: Delete, or in Trash Delete permanently
    /// (docs/command-bar.md, rule 6). What the Delete key and the reading pane's Delete run.
    /// </summary>
    public static CommandItem? Delete(CommandBar bar) =>
        bar.Sections.SelectMany(section => section.Items).FirstOrDefault(item =>
            item.Command is Command.Act { Action: RowAction.Delete or RowAction.PermanentlyDelete });

    /// <summary>Whether two answers draw the same bar. The generated records hold arrays, which a
    /// record compares by reference.</summary>
    public static bool Same(CommandBar a, CommandBar b) =>
        a.Sections.Length == b.Sections.Length
        && a.Sections.Zip(b.Sections).All(pair => pair.First.Items.SequenceEqual(pair.Second.Items))
        && a.Menu.SequenceEqual(b.Menu);
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
