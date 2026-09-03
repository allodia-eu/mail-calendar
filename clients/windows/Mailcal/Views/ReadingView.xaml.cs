// The message reading view: renders the open message's body that the Rust core fetched and
// (for HTML) sanitised. The Windows twin of macOS's ReadingView.swift and Android's
// ReadingScreen.kt; it is the third pane of the sidebar | list | reading layout, resting on a
// placeholder until a message is selected.
//
// This file owns the reading STATE, which of loading / html / plain / empty / error is showing,
// the header and recipient rows, the per-message remote-image opt-in, and the message actions.
// Its heavier responsibilities are partials:
//   ReadingView.WebView.cs      the hardened WebView2 host and the rendering-security gates
//   ReadingView.Attachments.cs  the attachment strip (save / open via the OS handler)
//   ReadingView.Invitation.cs   the meeting-invitation card and its Accept / Maybe / Decline

using Allodia.Mailcal.Calendar;
using Allodia.Mailcal.Services;
using Allodia.Mailcal.ViewModels;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Controls;
using Microsoft.UI.Xaml.Media;
using uniffi.mailcal_bindings;

namespace Allodia.Mailcal.Views;

/// <summary>The reading pane: a header plus the open message's fetched body, or a placeholder
/// when no message is selected.</summary>
public sealed partial class ReadingView : UserControl
{
    private MailboxModel? _model;

    /// <summary>Whether the user opted to load this message's remote images (reset per message).</summary>
    private bool _loadRemoteImages;

    /// <summary>Which message the pane has actually rendered, and whether it may stand in for the
    /// one being opened while that one's body is fetched (Services/ReadingHandover.cs).</summary>
    private readonly ReadingHandover _handover = new();

    /// <summary>Bounds that stand-in; created on the first handover and reused.</summary>
    private DispatcherTimer? _handoverTimer;

    /// <summary>Initialises the control.</summary>
    public ReadingView()
    {
        this.InitializeComponent();
        // The page every state of the body area is drawn on. Taken from the core so the sheet
        // this pane paints and the document the WebView paints inside it are one colour rather
        // than two whites that drift apart: the reason it is not a literal here any more, and
        // not a ThemeResource either (see the BodyArea comment in the XAML).
        BodyArea.Background = new SolidColorBrush(
            CalendarColors.Parse(MailcalBindingsMethods.MessageCanvas().Background));
    }

    /// <summary>Binds the view to the shared model and re-renders on reading-state changes.</summary>
    public void Init(MailboxModel model)
    {
        _model = model;
        model.PropertyChanged += (_, e) =>
        {
            if (e.PropertyName == nameof(MailboxModel.OpenedMessage))
            {
                OnOpenedChanged();
            }
            else if (e.PropertyName == nameof(MailboxModel.Reading))
            {
                Render();
            }
            else if (e.PropertyName == nameof(MailboxModel.CalendarWrite))
            {
                // An invitation answer settles on the calendar's own write surface. Only the card's
                // respond row moves, see OnCalendarWriteChanged for why this is not a Render().
                OnCalendarWriteChanged();
            }
        };
        // Set the initial resting state (no message selected yet → the placeholder).
        Render();
    }

    private void OnRetry(object sender, RoutedEventArgs e) => _model?.RetryOpen();

    /// <summary>The toolbar's natural width with the labels shown, static L10n strings, so it
    /// never changes for the session; measured once (see below).</summary>
    private double _labeledActionWidth;

    /// <summary>Whether the action toolbar is currently collapsed to icon-only.</summary>
    private bool _actionsIconOnly;

    // The reading pane is user-resizable (the draggable list|reading divider) down to a narrow
    // width where the action labels, long in Dutch and other locales, overflow. The buttons sit
    // in a horizontally-scrolling ScrollViewer, so the inner panel is always arranged at its full
    // NATURAL width (never clamped to the viewport), a reliable read even when it overflows,
    // unlike an in-place Measure. Cache that natural width while the labels are shown (their
    // strings are static, so it's constant), then collapse to icon-only whenever the pane is
    // narrower than that and restore the labels once it's comfortably wider again. The Windows
    // twin of the Apple client's compact-width toolbar (labels ⇢ icons at a narrow size class).
    private void OnActionToolbarSizeChanged(object sender, SizeChangedEventArgs e)
    {
        var available = ActionScroller.ActualWidth;
        if (available <= 0)
        {
            return;
        }
        // Learn (and keep the largest) natural labelled width while the labels are shown, the
        // ScrollViewer arranges ActionButtons unclamped, so this is the true content width. When
        // icon-only we can't observe it, so the cached value drives the decision to restore.
        if (!_actionsIconOnly && ActionButtons.ActualWidth > _labeledActionWidth)
        {
            _labeledActionWidth = ActionButtons.ActualWidth;
        }
        if (_labeledActionWidth <= 0)
        {
            return;
        }
        // Hysteresis: drop to icon-only once the labels no longer fit, but only restore them once
        // there's a comfortable margin again, so a divider parked at the edge doesn't oscillate.
        if (_actionsIconOnly)
        {
            if (available >= _labeledActionWidth + 24)
            {
                SetActionsIconOnly(false);
            }
        }
        else if (available < _labeledActionWidth)
        {
            SetActionsIconOnly(true);
        }
    }

    // Show or hide the action-button text labels together (the icons and tooltips stay), so the
    // toolbar shrinks to fit a narrow pane without clipping the buttons off the edge.
    private void SetActionsIconOnly(bool iconOnly)
    {
        if (_actionsIconOnly == iconOnly)
        {
            return;
        }
        _actionsIconOnly = iconOnly;
        var visibility = iconOnly ? Visibility.Collapsed : Visibility.Visible;
        ReplyLabel.Visibility = visibility;
        ReplyAllLabel.Visibility = visibility;
        ForwardLabel.Visibility = visibility;
        ArchiveLabel.Visibility = visibility;
        DeleteLabel.Visibility = visibility;
    }

    // Reply / reply-all / forward hand off to the shell, which opens the composer in this pane's
    // slot (MainWindow.Compose.cs), the To/Cc pre-fill, the From account, and the quoted original
    // are all derived there, so the list's context menu and this toolbar open the same composer by
    // the same route. Nothing needs guarding here: any draft that was open is this pane's own, and
    // replying to the message you are reading while already writing about it isn't reachable, the
    // toolbar is gone the moment the composer takes the column.
    private void OnReply(object sender, RoutedEventArgs e) => BeginCompose(replyAll: false);

    private void OnReplyAll(object sender, RoutedEventArgs e) => BeginCompose(replyAll: true);

    private void BeginCompose(bool replyAll)
    {
        if (_model?.OpenedMessage is not { } opened)
        {
            return;
        }
        App.Shell?.ComposeReply(opened.Account, opened.Key, replyAll);
    }

    private void OnForward(object sender, RoutedEventArgs e)
    {
        if (_model?.OpenedMessage is not { } opened)
        {
            return;
        }
        App.Shell?.ComposeForward(opened.Account, opened.Key);
    }

    // Archive/delete move the open message out of the folder, so the pane cannot keep showing it,
    // it advances to the next message down (or, at the end of the list, the one above) so a mailbox
    // can be worked through without going back to the list for each message. The destination is
    // chosen BEFORE the dispatch, while the row it is relative to is still on screen.
    private void OnArchive(object sender, RoutedEventArgs e)
    {
        if (_model is null || _model.OpenedMessage is not { } opened)
        {
            return;
        }
        var next = _model.StopAfterRemoving(opened);
        _model.Archive(opened.Account, opened.Key);
        _model.SettleReadingPane(next);
    }

    private void OnDelete(object sender, RoutedEventArgs e)
    {
        if (_model is null || _model.OpenedMessage is not { } opened)
        {
            return;
        }
        var next = _model.StopAfterRemoving(opened);
        _model.Delete(opened.Account, opened.Key);
        _model.SettleReadingPane(next);
    }

    private void OnLoadRemoteImages(object sender, RoutedEventArgs e)
    {
        _loadRemoteImages = true;
        RemoteImagesBanner.Visibility = Visibility.Collapsed;
        Render();
    }

    // A new message was opened (or the view closed): reset the per-message image opt-in and
    // force the next HTML render (a different message reuses the same WebView).
    private void OnOpenedChanged()
    {
        _loadRemoteImages = false;
        _lastFragment = null;
        Render();
    }

    // Reflect the current opened-message + body snapshot; called on every relevant change and
    // idempotent. Draws the body area empty until the core says the open is worth announcing,
    // then the spinner, a fetch-error retry, the HTML (locked-down WebView), the plain-text
    // fallback, or an empty state.
    private void Render()
    {
        if (_model?.OpenedMessage is not { } opened)
        {
            ShowNoSelection(); // no message selected, the pane rests on its placeholder.
            return;
        }

        var body = _model.Reading;
        // Ignore a stale body for a previously-opened message, wait for this one. While waiting,
        // the message already on screen stands in for a moment (ReadingHandover) instead of the
        // pane being torn down to a spinner and rebuilt: everything that moves, the header, the
        // recipient rows, the remote-images bar, the body, then moves in one step.
        if (body is null || body.Key != opened.Key)
        {
            var step = _handover.Next(opened.Key);
            if (step == HandoverStep.StartGrace)
            {
                StartHandoverGrace();
                return;
            }
            if (step == HandoverStep.Hold)
            {
                return;
            }
            // Nothing to stand in, or the grace window is spent: show this message's header over
            // an empty body area. Reset the render guard so the next HTML render runs (e.g. after
            // a retry clears the pane first).
            StopHandoverGrace();
            _handover.Cleared();
            ShowHeader(opened);
            _lastFragment = null;
            RemoteImagesBanner.Visibility = Visibility.Collapsed;
            AttachmentPanel.Visibility = Visibility.Collapsed;
            SetInvitation(null);
            SetRecipients(null);
            // Nothing yet, and too soon to say so: the core announces a wait only once one has
            // run long enough to notice, so a fast open draws no ring at all rather than
            // flashing one. The header set above already carries the row that was clicked.
            ShowState();
            return;
        }
        StopHandoverGrace();
        ShowHeader(opened);
        // The body for this message has arrived, surface its recipient headers, and upgrade the
        // sender line from the carried name-only (set above) to the body's full `Name <email>`
        // (keeping the carried name if the body supplies none, so it never blanks).
        if (!string.IsNullOrEmpty(body.From))
        {
            FromText.Text = body.From;
        }
        // The core resolved this one against the same map the list read, so it is the answer once
        // it arrives; it can only differ from the row's by having found a photo. A `pending`
        // snapshot has resolved nothing and carries the avatar for nobody, so taking it would
        // replace the row's face with an empty circle for exactly as long as the wait lasts,
        // the flash ShowHeader sets the row's avatar to prevent (docs/avatars.md).
        if (!body.Pending)
        {
            SenderAvatar.Avatar = body.Avatar;
        }
        SetRecipients(body);
        SetAttachments(body);
        SetInvitation(body);
        if (body.Pending)
        {
            // The core publishes this only once the open has outlasted its threshold. It carries
            // no body, so it must be read before the branches that look for one, otherwise a
            // wait draws the "no content" text.
            _handover.Cleared();
            _lastFragment = null;
            RemoteImagesBanner.Visibility = Visibility.Collapsed;
            ShowState(loading: true);
        }
        else if (body.LoadError)
        {
            // A failed fetch (provider/network), not a body-less message, offer a retry. Nothing
            // here is worth holding the pane on: pressing Retry has to answer at once, and an
            // error panel is not a message for the next one to be handed over from.
            _handover.Cleared();
            _lastFragment = null;
            RemoteImagesBanner.Visibility = Visibility.Collapsed;
            ShowState(error: true);
        }
        else if (!string.IsNullOrEmpty(body.Html))
        {
            _handover.Rendered(opened.Key);
            RemoteImagesBanner.Visibility =
                body.HasRemoteImages && !_loadRemoteImages ? Visibility.Visible : Visibility.Collapsed;
            ShowState(html: true);
            RenderHtml(body.Html!);
        }
        else if (!string.IsNullOrEmpty(body.Plain))
        {
            _handover.Rendered(opened.Key);
            RemoteImagesBanner.Visibility = Visibility.Collapsed;
            PlainText.Text = body.Plain;
            ShowState(plain: true);
        }
        else
        {
            _handover.Rendered(opened.Key);
            RemoteImagesBanner.Visibility = Visibility.Collapsed;
            ShowState(empty: true);
        }
    }

    // The pane's resting state: no message selected, so show the placeholder and hide the
    // message UI (the body states collapse with it).
    private void ShowNoSelection()
    {
        StopHandoverGrace();
        _handover.Cleared();
        NoSelectionPanel.Visibility = Visibility.Visible;
        ContentRoot.Visibility = Visibility.Collapsed;
        RemoteImagesBanner.Visibility = Visibility.Collapsed;
        SetRecipients(null);
        AttachmentPanel.Visibility = Visibility.Collapsed;
        SetInvitation(null);
        ShowState();
    }

    // The header the opened row carries, which is available before its body is. The body upgrades
    // the sender line to its full `Name <email>` once it lands.
    private void ShowHeader(OpenedMessage opened)
    {
        ContentRoot.Visibility = Visibility.Visible;
        NoSelectionPanel.Visibility = Visibility.Collapsed;
        SubjectText.Text = string.IsNullOrEmpty(opened.Subject) ? "(no subject)" : opened.Subject;
        FromText.Text = opened.From;
        // The face the list row already drew, so opening a message never flashes an empty circle,
        // or, worse, replaces a photograph with initials (docs/avatars.md).
        SenderAvatar.Avatar = opened.Avatar;
        DateText.Text = opened.DateText;
    }

    // Arm (or re-arm) the window the rendered message may stand in for a newly opened one. On
    // expiry the pane falls back to this message's header over an empty body area, so a slow fetch
    // is never hidden behind a message the user has already moved on from.
    private void StartHandoverGrace()
    {
        if (_handoverTimer is null)
        {
            _handoverTimer = new DispatcherTimer { Interval = ReadingHandover.Grace };
            _handoverTimer.Tick += (_, _) =>
            {
                StopHandoverGrace();
                _handover.GraceElapsed();
                Render();
            };
        }
        _handoverTimer.Stop();
        _handoverTimer.Start();
    }

    private void StopHandoverGrace() => _handoverTimer?.Stop();

    // Set the To/Cc/Bcc header rows from the body snapshot (each collapses when empty); pass
    // null while loading / when nothing is selected to collapse them all.
    private void SetRecipients(ReadingBody? body)
    {
        SetRecipientRow(ToText, L10n.ComposeTo(), body?.To);
        SetRecipientRow(CcText, L10n.ComposeCc(), body?.Cc);
        SetRecipientRow(BccText, L10n.ComposeBcc(), body?.Bcc);
    }

    private static void SetRecipientRow(TextBlock block, string label, string? value)
    {
        if (string.IsNullOrEmpty(value))
        {
            block.Visibility = Visibility.Collapsed;
        }
        else
        {
            block.Text = $"{label}: {value}";
            block.Visibility = Visibility.Visible;
        }
    }

    // Exactly one body state is visible at a time.
    private void ShowState(
        bool loading = false, bool html = false, bool plain = false,
        bool empty = false, bool error = false)
    {
        LoadingRing.IsActive = loading;
        LoadingRing.Visibility = loading ? Visibility.Visible : Visibility.Collapsed;
        Body.Visibility = html ? Visibility.Visible : Visibility.Collapsed;
        PlainScroller.Visibility = plain ? Visibility.Visible : Visibility.Collapsed;
        EmptyText.Visibility = empty ? Visibility.Visible : Visibility.Collapsed;
        ErrorPanel.Visibility = error ? Visibility.Visible : Visibility.Collapsed;
    }
}
