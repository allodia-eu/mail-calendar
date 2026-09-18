// The Outbox half of MailboxModel: every account's unsent messages, and the three things a person
// can do about one (docs/sending.md). Its own partial to keep MailboxModel.cs under the 500-line
// limit.
//
// Nothing here decides whether a send waits or fails; that is the engine's, and the core asks the
// queue rather than re-reading the error. This file only renders what the snapshot carries and
// sends the user's answer back.

using System.Collections.ObjectModel;
using Allodia.Mailcal.ViewModels;
using uniffi.mailcal_bindings;

namespace Allodia.Mailcal.Services;

public sealed partial class MailboxModel
{
    /// <summary>
    /// Every account's unsent messages, oldest first: the order they will go out in.
    /// </summary>
    /// <remarks>
    /// Populated in every view, like <see cref="Accounts"/>, because the pane row it counts is on
    /// screen whatever the list is showing. <b>Empty means no Outbox row at all</b>
    /// (docs/folder-pane.md, rule 18): a row that only ever says zero trains people to stop
    /// reading it, which is the opposite of what an unsent message needs.
    /// </remarks>
    public ObservableCollection<QueuedRowItem> Outbox { get; } = new();

    /// <summary>Whether the pane draws an Outbox row at all.</summary>
    public bool HasOutbox => Outbox.Count > 0;

    private bool _showingOutbox;

    /// <summary>
    /// Whether the list is showing the Outbox rather than mail.
    /// </summary>
    /// <remarks>
    /// The core's own answer, and not derivable from anything beside it:
    /// <see cref="SelectedAccount"/> and <see cref="SelectedFolder"/> are both <c>null</c> here
    /// <i>and</i> on the unified inbox, so a client that guessed would answer a click on the
    /// Outbox with everyone's inbox.
    /// </remarks>
    public bool ShowingOutbox
    {
        get => _showingOutbox;
        private set
        {
            if (Set(ref _showingOutbox, value))
            {
                Raise(nameof(OutboxListVisible));
                Raise(nameof(MailRowsVisible));
                Raise(nameof(OutboxEmptyVisible));
                RaiseListTitle();
                Raise(nameof(MailCountText));
            }
        }
    }

    /// <summary>Show the Outbox: every account's unsent messages, in one list.</summary>
    /// <remarks>
    /// Closes the reading pane first, for the reason <see cref="SelectAccount"/> does: the message
    /// it holds is not in the list the user is moving to, and a pane left open beside the Outbox
    /// reads as one of the things waiting.
    /// </remarks>
    public void ShowOutbox()
    {
        CloseReading();
        Destination = AppDestination.Mail;
        _app?.Dispatch(new Intent.Outbox(new OutboxIntent.Show()));
    }

    /// <summary>
    /// Sends a queued message now instead of waiting out its backoff.
    /// </summary>
    /// <remarks>
    /// Offered only for a waiting message (<see cref="QueuedRowItem.IsActionable"/>). The core
    /// clears the backoff and drains in the same gesture, so the attempt happens while the user is
    /// still looking at the row rather than at some later pass.
    /// </remarks>
    public void SendQueuedNow(QueuedRowItem row)
    {
        Log.Info("outbox: sending a queued message now at the user's request");
        _app?.Dispatch(new Intent.Outbox(new OutboxIntent.SendNow(row.Account, row.Op)));
    }

    /// <summary>Withdraws a queued message so it is never delivered.</summary>
    public void CancelQueued(QueuedRowItem row)
    {
        Log.Info("outbox: withdrawing a queued message at the user's request");
        _app?.Dispatch(new Intent.Outbox(new OutboxIntent.Cancel(row.Account, row.Op)));
    }

    /// <summary>Withdraws a queued message and hands it back to the composer.</summary>
    /// <remarks>
    /// The core withdraws it <b>first</b> and then offers it back through
    /// <c>Surface.ComposeRequest</c>, which <see cref="PullComposeRequest"/> answers; this only
    /// asks. The other order leaves a window in which a drain delivers the message being edited,
    /// and no part of this app can take that back (docs/sending.md).
    /// </remarks>
    public void EditQueued(QueuedRowItem row)
    {
        Log.Info("outbox: withdrawing a queued message to edit it");
        _app?.Dispatch(new Intent.Outbox(new OutboxIntent.Edit(row.Account, row.Op)));
    }

    /// <summary>
    /// Raised when the core has a withdrawn message for this host's composer to hold.
    /// </summary>
    /// <remarks>
    /// The shell answers it by opening the composer (MainWindow.Compose.cs). Not a snapshot field:
    /// it is a standing request, and it stands until <see cref="DismissComposeRequest"/> says the
    /// composer has the message.
    /// </remarks>
    internal event Action<ComposeRequest>? ComposeRequested;

    /// <summary>
    /// Pulls the message the core is asking this host to compose (on a
    /// <c>Surface.ComposeRequest</c> signal), and hands it to the shell.
    /// </summary>
    /// <remarks>
    /// What arrives here is the <b>only</b> copy: the message has already left the Outbox, so a
    /// host that drops it loses it. The request is therefore left standing until the composer is
    /// actually up, which is where <see cref="DismissComposeRequest"/> is called from, never here.
    /// </remarks>
    private void PullComposeRequest()
    {
        if (_app?.ComposeRequest() is not { } request)
        {
            return;
        }
        // Whether there is a message to open, never its recipients, subject or body, which are the
        // user's own mail (docs/logging.md).
        Log.Info("outbox: opening a withdrawn message in the composer");
        ComposeRequested?.Invoke(request);
    }

    /// <summary>
    /// Tells the core the composer holds the message, so the request is answered and must not be
    /// offered again.
    /// </summary>
    /// <remarks>
    /// Not a cancellation: the message left the Outbox before the request existed, so there is
    /// nothing to put back. Left unanswered it would reappear at every relaunch, offering to
    /// compose a message the user already has open.
    /// </remarks>
    public void DismissComposeRequest() => _app?.Dispatch(new Intent.DismissComposeRequest());

    /// <summary>The state words and glyphs an Outbox row is drawn with.</summary>
    /// <remarks>
    /// Segoe Fluent codepoints, written as <c>\u</c> escapes for the reason
    /// <c>MainWindow.Sidebar.cs</c> gives: they sit in the Unicode private use area, where a
    /// literal renders as nothing outside the font and reads as an empty string to a reviewer.
    /// </remarks>
    private static OutboxLabels Labels => new(
        L10n.OutboxWaiting(),
        L10n.OutboxSending(),
        L10n.OutboxUnconfirmed(),
        WaitingGlyph: "\uE823",      // Clock
        SendingGlyph: "\uE724",      // Send (a paper plane), as the Sent folder's row carries
        UnconfirmedGlyph: "\uE7BA", // Warning
        // The same words the message list and the reading header use for the same absence.
        NoSubject: L10n.MailNoSubject());

    /// <summary>Projects the snapshot's queued sends into <see cref="Outbox"/>.</summary>
    /// <remarks>
    /// Reconciled in place by <see cref="QueuedRowItem.Id"/> like every other list here, so a
    /// refresh arriving while a row's context menu is open does not take the row out from under
    /// it. A drain runs on every sync pass, so those refreshes are frequent.
    /// </remarks>
    private void SyncOutbox(IReadOnlyList<QueuedRow> queued)
    {
        var had = Outbox.Count > 0;
        Reconcile(Outbox, OutboxRows.Build(queued, Labels, AccountEmail), row => row.Id, SameQueued);
        if (had != (Outbox.Count > 0))
        {
            Raise(nameof(HasOutbox));
            Raise(nameof(OutboxEmptyVisible));
        }
    }

    // Every field a row draws, so a state change (waiting -> sending) replaces the row rather than
    // leaving the old word beside a message that has moved on.
    private static bool SameQueued(QueuedRowItem left, QueuedRowItem right) =>
        left.ToText == right.ToText
        && left.SubjectText == right.SubjectText
        && left.AccountText == right.AccountText
        && left.State == right.State;
}
