// What the reading view's action row does, which is the one thing a detached reading window
// changes about it (docs/reading-window.md).
//
// The row itself is fixed by docs/reading-actions.md and is the same in both hosts: reply,
// reply-all, forward, archive, delete, overflow, in that order. Two rules are the window's own,
// and they are the reason this is handed in rather than decided in the view:
//
//   * archive and delete CLOSE the window. The message has left the folder, so there is nothing
//     left for the window to be about. The pane moves on to the next message instead, because it
//     is a place in a list; a window is one message.
//   * reply, reply-all and forward open a COMPOSER WINDOW. A draft answering the window in front
//     of you does not belong in a different window behind it.

using System;
using System.Threading.Tasks;
using Allodia.Mailcal.Dialogs;
using Allodia.Mailcal.Services;
using Microsoft.UI.Xaml;
using uniffi.mailcal_bindings;

namespace Allodia.Mailcal.ViewModels;

/// <summary>What the reading view's row and its load-error panel perform, as its host performs
/// them.</summary>
/// <param name="Reply">Reply, or reply-all when the flag is set.</param>
/// <param name="Forward">Forward the message.</param>
/// <param name="Archive">Archive it, and settle whatever was showing it.</param>
/// <param name="Delete">Delete it as the command bar would (<see cref="DeleteAsync"/>), and settle whatever was showing it.</param>
/// <param name="Retry">Re-run the open behind the load-error panel, into this host's own slot.</param>
internal sealed record ReadingActions(
    Action<OpenedMessage, bool> Reply,
    Action<OpenedMessage> Forward,
    Action<OpenedMessage> Archive,
    Action<OpenedMessage> Delete,
    Action Retry)
{
    /// <summary>
    /// The reading pane's own actions: the composer opens in this pane's slot, and archive/delete
    /// move the pane on to the message the core names (docs/reading-actions.md, "Where the pane goes
    /// next").
    /// </summary>
    /// <remarks>
    /// Nothing here guards a draft. Any composer that was open is this pane's own, and replying to
    /// the message you are reading while already writing about it is not reachable: the action row
    /// is gone the moment the composer takes the column.
    /// <para>
    /// Where the pane lands is asked BEFORE the dispatch, while the message is still listed, and is
    /// opened through the list's own click path (<c>OpenOrResumeAsync</c>), so it lands where a click
    /// on that row would have taken the reader.
    /// </para>
    /// </remarks>
    public static ReadingActions ForPane(MailboxModel model) => new(
        (opened, all) => App.Shell?.ComposeReply(opened.Account, opened.Key, all),
        opened => App.Shell?.ComposeForward(opened.Account, opened.Key),
        opened => _ = MoveOnAsync(model, opened, () => model.Archive(opened.Account, opened.Key)),
        opened => _ = DeleteAsync(model, opened, App.Shell?.Content?.XamlRoot, remove => MoveOnAsync(model, opened, remove)),
        model.RetryOpen);

    /// <summary>
    /// A reading view's Delete, which is the command bar's Delete over that message's row
    /// (docs/command-bar.md, rule 8): in Trash, or on an account with no Trash, it asks first and
    /// deletes for good; anywhere else it moves the message to Trash. <paramref name="settle"/> runs
    /// the removal and leaves the view as its host leaves it.
    /// </summary>
    /// <remarks>Anything but the bar's Delete permanently is the move to Trash, so a message the
    /// snapshot no longer lists, in a window left open on it, still goes to Trash.</remarks>
    internal static async Task DeleteAsync(MailboxModel model, OpenedMessage opened, XamlRoot? root, Func<Action, Task> settle)
    {
        var item = CommandBarPresentation.Delete(
            model.CommandBarOver([new SelectedRow.Message(opened.Account, opened.Key)]));
        if (item is { Command: Command.Act { Action: RowAction.PermanentlyDelete } })
        {
            if (!item.Enabled
                || (item.Confirm && (root is null || !await PurgeQuestion.AskAsync(root, PermanentDeleteWording.Message, 1))))
            {
                return;
            }
            await settle(() => model.PermanentlyDelete(opened.Account, opened.Key));
            return;
        }
        await settle(() => model.Delete(opened.Account, opened.Key));
    }

    /// <summary>
    /// Runs <paramref name="dispatch"/>, which takes <paramref name="opened"/> out of the list, and
    /// moves the pane on to the message the core names, opened as clicking its row opens it: a
    /// draft resumes in a composer. The pane empties when nothing is left.
    /// </summary>
    /// <remarks>
    /// Where the pane lands is asked BEFORE the dispatch, while the message is still listed. The
    /// pane's own Archive and Delete come here, and so does a row menu's move of the open message.
    /// </remarks>
    internal static async Task MoveOnAsync(MailboxModel model, OpenedMessage opened, Action dispatch)
    {
        var next = model.StopAfterRemoving(opened);
        dispatch();
        if (next is null || model.ReadStop(next) is not { } read)
        {
            model.CloseReading();
            return;
        }
        if (App.Shell is { } shell)
        {
            await shell.OpenOrResumeAsync(next.Account, next.Key, next.Kind == MessageKind.Draft, read);
        }
        else
        {
            read();
        }
    }
}
