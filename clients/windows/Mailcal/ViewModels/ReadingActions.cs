// What the reading view's action row does, which is the one thing a detached reading window
// changes about it (docs/reading-window.md).
//
// The row itself is fixed by docs/reading-actions.md and is the same in both hosts: reply,
// reply-all, forward, archive, delete, overflow, in that order. Two rules are the window's own,
// and they are the reason this is handed in rather than decided in the view:
//
//   * archive and delete CLOSE the window. The message has left the folder, so there is nothing
//     left for the window to be about. The pane advances to the next message down instead, because
//     it is a place in a list; a window is one message.
//   * reply, reply-all and forward open a COMPOSER WINDOW. A draft answering the window in front
//     of you does not belong in a different window behind it.

using System;
using Allodia.Mailcal.Services;

namespace Allodia.Mailcal.ViewModels;

/// <summary>What the reading view's row and its load-error panel perform, as its host performs
/// them.</summary>
/// <param name="Reply">Reply, or reply-all when the flag is set.</param>
/// <param name="Forward">Forward the message.</param>
/// <param name="Archive">Archive it, and settle whatever was showing it.</param>
/// <param name="Delete">Move it to Trash, and settle whatever was showing it.</param>
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
    /// advance the pane to the next message down.
    /// </summary>
    /// <remarks>
    /// Nothing here guards a draft. Any composer that was open is this pane's own, and replying to
    /// the message you are reading while already writing about it is not reachable: the action row
    /// is gone the moment the composer takes the column.
    /// <para>
    /// Where the pane lands is chosen BEFORE the dispatch, while the row it is relative to is still
    /// on screen.
    /// </para>
    /// </remarks>
    public static ReadingActions ForPane(MailboxModel model) => new(
        (opened, all) => App.Shell?.ComposeReply(opened.Account, opened.Key, all, opened.RawSubject),
        opened => App.Shell?.ComposeForward(opened.Account, opened.Key, opened.RawSubject),
        opened => Settle(model, opened, archive: true),
        opened => Settle(model, opened, archive: false),
        model.RetryOpen);

    private static void Settle(MailboxModel model, OpenedMessage opened, bool archive)
    {
        var next = model.StopAfterRemoving(opened);
        if (archive)
        {
            model.Archive(opened.Account, opened.Key);
        }
        else
        {
            model.Delete(opened.Account, opened.Key);
        }
        model.SettleReadingPane(next);
    }
}
