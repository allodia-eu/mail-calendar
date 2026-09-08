// The reading-view half of MailboxModel (split out to keep each file under the 500-line
// limit): the open message's header + fetched body, the OpenMessage/CloseReading actions,
// and the shared-Rust HTML document wrapper the WebView2 loads. The Windows counterpart of
// macOS's MailboxModel.openMessage + ReadingView wiring. State stays in Rust; this only
// holds the projected snapshot the reading view binds to.

using System.Collections.Generic;
using System.Linq;
using Allodia.Mailcal.ViewModels;
using uniffi.mailcal_bindings;

namespace Allodia.Mailcal.Services;

public sealed partial class MailboxModel
{
    private OpenedMessage? _openedMessage;
    /// <summary>
    /// The message currently open in the reading pane (the row the user tapped), or <c>null</c>
    /// when nothing is selected (the pane shows its placeholder). The reading view re-renders on
    /// this change.
    /// </summary>
    public OpenedMessage? OpenedMessage
    {
        get => _openedMessage;
        private set => Set(ref _openedMessage, value);
    }

    private ReadingBody? _reading;
    /// <summary>
    /// The open message's fetched, sanitised body, pulled on a <c>Surface.Reading</c> signal.
    /// <c>null</c> until a snapshot for the open message arrives, until then, and while
    /// <see cref="ReadingBody.Key"/> still names the previous message, the reading view draws
    /// the body area empty. Only <see cref="ReadingBody.Pending"/> raises the loading ring.
    /// </summary>
    public ReadingBody? Reading
    {
        get => _reading;
        private set => Set(ref _reading, value);
    }

    /// <summary>
    /// Open a row for reading: record its header for the reading view, then fetch + sanitize
    /// the body. A flat row opens its own message; a conversation row opens its latest message
    /// (a thread has no key of its own, <see cref="MailRow.LatestKey"/> stands in), so reading
    /// works in threaded mode too. The header's key is the opened message key, so the reading
    /// view can match the body snapshot back to it.
    /// </summary>
    public void OpenMessage(MailRow row)
    {
        OpenedMessage = new OpenedMessage
        {
            Account = row.Account,
            Key = row.LatestKey,
            Subject = row.Title,
            RawSubject = row.RawSubject,
            From = row.Subtitle,
            Avatar = row.Avatar,
            // The reading header shows the full absolute date, not the row's compact relative label.
            DateText = row.FullDateText,
        };
        OpenKey(row.Account, row.LatestKey);
    }

    // The conversation rows expanded inline, by their list id ("t:account:thread"). Kept here so
    // a snapshot refresh restores expansion (the projection reads it) rather than collapsing every
    // open thread; a set, so re-adding an already-expanded thread is idempotent.
    private readonly HashSet<string> _expandedThreads = new();

    /// <summary>
    /// Toggles a conversation's inline expansion (revealing its whole thread as sub-rows).
    /// Expanding also opens its latest message in the reading pane, the desktop 3-pane behaviour,
    /// like macOS; collapsing leaves the pane as-is.
    /// </summary>
    public void ToggleThread(MailRow row)
    {
        row.IsExpanded = !row.IsExpanded;
        if (row.IsExpanded)
        {
            _expandedThreads.Add(row.Id);
            OpenMessage(row);
        }
        else
        {
            _expandedThreads.Remove(row.Id);
        }
    }

    /// <summary>Opens one message of an expanded conversation (a sub-row) in the reading pane.</summary>
    public void OpenThreadMessage(ThreadMessageItem message)
    {
        OpenedMessage = new OpenedMessage
        {
            Account = message.Account,
            Key = message.Key,
            Subject = message.Subject,
            RawSubject = message.RawSubject,
            From = message.FromText,
            Avatar = message.Avatar,
            // The reading header shows the full absolute date, not the sub-row's relative label.
            DateText = message.FullDateText,
        };
        OpenKey(message.Account, message.Key);
    }

    /// <summary>
    /// Archives a conversation, the core moves its received side to Archive and leaves any Sent
    /// copies in Sent, then tidies the UI: collapse it and, if the open message was part of it,
    /// clear the reading pane (its row is leaving the folder).
    /// </summary>
    public void ArchiveConversation(MailRow row)
    {
        ArchiveThread(row.Account, row.Key);
        _expandedThreads.Remove(row.Id);
        row.IsExpanded = false;
        if (OpenedMessage is { } opened
            && row.Messages.Any(m => m.Account == opened.Account && m.Key == opened.Key))
        {
            CloseReading();
        }
    }

    /// <summary>
    /// Re-open the message currently in the reading view, the "retry" affordance after a
    /// load error. Reuses the existing header; just re-fetches the body.
    /// </summary>
    public void RetryOpen()
    {
        if (OpenedMessage is { } opened)
        {
            OpenKey(opened.Account, opened.Key);
        }
    }

    /// <summary>
    /// Clear any stale body so the view shows its loading state until this key's snapshot
    /// arrives, including on a retry of the same key after a load error, then ask the core
    /// to fetch + sanitise the message body.
    /// </summary>
    private void OpenKey(string account, string key)
    {
        Reading = null;
        _app?.Dispatch(new Intent.OpenMessage(account, key));
    }

    /// <summary>Clear the open message (the reading pane falls back to its placeholder); called
    /// when the scope changes, switching account/folder or to the calendar.</summary>
    public void CloseReading()
    {
        OpenedMessage = null;
        Reading = null;
    }

    /// <summary>
    /// The messages the list is showing, in the order they appear, a flat row is itself, an
    /// expanded conversation contributes its own messages in place of its header, and a collapsed
    /// one stands for the single message its row summarises (the same one clicking it would open).
    /// </summary>
    /// <remarks>
    /// The view-facing half of the auto-advance: it reads the row view models, so it lives here
    /// rather than in <see cref="ReadingAdvance"/>, which Mailcal.Tests links and must keep
    /// WinUI-free. Mirrors Apple's <c>readableStops</c>.
    /// </remarks>
    private List<MessageStop> ReadableStops()
    {
        var stops = new List<MessageStop>();
        foreach (var row in Rows)
        {
            if (row.IsThread && row.IsExpanded)
            {
                foreach (var message in row.Messages)
                {
                    stops.Add(new MessageStop(
                        message.Account,
                        message.Key,
                        message.Subject,
                        message.FromText,
                        message.Avatar,
                        message.FullDateText,
                        message.RawSubject));
                }
                continue;
            }

            // A conversation row has no key of its own; LatestKey is what a click opens.
            stops.Add(new MessageStop(
                row.Account, row.LatestKey, row.Title, row.Subtitle, row.Avatar, row.FullDateText,
                row.RawSubject));
        }

        return stops;
    }

    /// <summary>
    /// Where the reading pane should land after <paramref name="opened"/> leaves the folder: the
    /// next message down, or the one above it at the end of the list (<see cref="ReadingAdvance"/>).
    /// </summary>
    /// <remarks>
    /// Read <em>before</em> the archive/delete is dispatched, so the answer doesn't depend on
    /// whether the core has re-projected the list yet.
    /// </remarks>
    public MessageStop? StopAfterRemoving(OpenedMessage opened) =>
        ReadingAdvance.Next(
            new MessageStop(
                opened.Account, opened.Key, opened.Subject, opened.From, opened.Avatar,
                opened.DateText, opened.RawSubject),
            ReadableStops());

    /// <summary>
    /// Opens what <see cref="StopAfterRemoving"/> chose, or empties the pane when the folder had
    /// nothing left to read.
    /// </summary>
    public void SettleReadingPane(MessageStop? next)
    {
        if (next is not { } stop)
        {
            CloseReading();
            return;
        }

        OpenedMessage = new OpenedMessage
        {
            Account = stop.Account,
            Key = stop.Key,
            Subject = stop.Subject,
            RawSubject = stop.RawSubject,
            From = stop.From,
            Avatar = stop.Avatar,
            DateText = stop.DateText,
        };
        OpenKey(stop.Account, stop.Key);
    }

    /// <summary>
    /// Wraps a sanitised HTML body fragment in the shared-Rust, strict-CSP document the
    /// locked-down WebView2 loads, built in the core so the security boundary and base
    /// styling are identical across clients. <paramref name="loadRemoteImages"/> reflects the
    /// user's per-message choice to load remote images.
    /// </summary>
    public string RenderMessageHtml(string html, bool loadRemoteImages) =>
        MailcalBindingsMethods.RenderMessageHtml(html, loadRemoteImages);

    /// <summary>
    /// Whether a link the user clicked in a rendered message should be opened in the OS
    /// default browser/handler. The launch policy (a strict scheme allowlist, mail is
    /// hostile input) is shared in Rust so every client decides identically and consistently
    /// with what the sanitiser keeps; only the actual launch is native.
    /// </summary>
    public bool ShouldOpenExternalLink(string url) =>
        MailcalBindingsMethods.ShouldOpenExternalLink(url);

    /// <summary>Save one decoded attachment to a host-selected path.</summary>
    internal bool SaveAttachment(string account, string key, uint attachmentId, string destinationPath)
    {
        if (_app is null)
        {
            return false;
        }
        try
        {
            _app.SaveAttachment(account, key, attachmentId, destinationPath);
            return true;
        }
        catch (Exception ex)
        {
            Log.Warn($"attachment save failed: {ex.GetType().Name}");
            return false;
        }
    }

    /// <summary>Pulls the reading snapshot from the core (on a <c>Surface.Reading</c> signal).</summary>
    private void PullReading()
    {
        if (_app is null)
        {
            return;
        }
        var snapshot = _app.ReadingView();
        Reading = new ReadingBody
        {
            Key = snapshot.Key,
            From = snapshot.From,
            Avatar = AvatarItem.From(snapshot.Avatar),
            To = snapshot.To,
            Cc = snapshot.Cc,
            Bcc = snapshot.Bcc,
            Html = snapshot.Html,
            Plain = snapshot.Plain,
            Attachments = snapshot.Attachments
                .Select(attachment => new MessageAttachment
                {
                    Id = attachment.Id,
                    FileName = attachment.FileName,
                    MediaType = attachment.MediaType,
                    Size = attachment.Size,
                })
                .ToList(),
            HasRemoteImages = snapshot.HasRemoteImages,
            LoadError = snapshot.LoadError,
            Pending = snapshot.Pending,
            // Passed through as the core built it, see ReadingBody.Invitation for why this one is
            // not re-projected. Null for all but a genuine iMIP invitation.
            Invitation = snapshot.Invitation,
        };
    }

    /// <summary>
    /// Answer the invitation the open message carries, then refresh the calendar and this pane.
    /// </summary>
    /// <remarks>
    /// Named by the <b>message</b>, never by the event: the answer goes out as the address the
    /// invitation matched, which on an aliased account is not the account's primary identity, and
    /// only the core knows the address set (<c>docs/invitations.md</c> §4).
    /// <para>
    /// <paramref name="comment"/> must be <c>null</c> unless the card says <c>can_comment</c>, and
    /// <paramref name="notifyOrganizer"/> <c>true</c> unless it says <c>can_choose_notify</c>, a
    /// transport that cannot honour one refuses the whole answer rather than dropping it, so an
    /// unasked-for note loses the answer, not just the note.
    /// </para>
    /// <para>
    /// Not applied optimistically: the write settles behind the same <c>CalendarWriteStatus</c>
    /// every other calendar write reports on, and both surfaces are rebuilt from what the server
    /// holds. Hiding a declined meeting immediately would buy a few hundred milliseconds and cost a
    /// rollback path exercised only when something has already gone wrong.
    /// </para>
    /// </remarks>
    internal void RespondToInvitation(
        string account, string key, InvitationResponse response, string? comment, bool notifyOrganizer,
        string? replySubject = null)
    {
        // Counts and the answer only, a meeting's title, its organiser and its attendees are
        // message content, and the diagnostic log has to stay safe to attach to a support request
        // (docs/invitations.md → Logging and privacy). That is also why the reply subject is not
        // logged: it *contains* the meeting's title.
        Log.Info($"invitation: respond {response} notify={notifyOrganizer} note={comment?.Length ?? 0}");
        _app?.Dispatch(new Intent.RespondToInvitation(
            account, key, response, comment, notifyOrganizer, replySubject));
    }
}
