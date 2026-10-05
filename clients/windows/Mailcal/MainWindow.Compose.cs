// The compose state: the one place that decides whether the detail column shows the
// reading pane or the composer.
//
// The composer used to be a ContentDialog, constructed independently at five call sites (three in
// MailListView, two in ReadingView), a modal that blacked out the mailbox behind it. It now
// REPLACES the reading pane: the sidebar and the message list stay live and clickable while you
// write. The five call sites collapse into ComposeNew / ComposeReply / ComposeForward
// here, so the quoted-original seed and the From account are derived in one place rather than five.
//
// The Windows twin of the Apple client's `@State var compose: ComposeContext?`, which the macOS
// detail column renders the same way.

using Allodia.Mailcal.Services;
using Allodia.Mailcal.ViewModels;
using Allodia.Mailcal.Views;
using Microsoft.UI.Xaml;
using uniffi.mailcal_bindings;

namespace Allodia.Mailcal;

public sealed partial class MainWindow
{
    /// <summary>The composer currently in the detail column, or <c>null</c> when the reading pane
    /// has it. Built fresh per draft and torn down on Send/Discard, never reused across messages,
    /// so no document, quote, or attachment list can leak from one draft into the next.</summary>
    private ComposerView? _composer;

    /// <summary>Whether a composer is open (a draft is in the detail column).</summary>
    internal bool IsComposing => _composer is not null;

    /// <summary>Opens the composer for a brand-new message.</summary>
    internal void ComposeNew()
    {
        var quoting = Model.QuoteSettings;
        // Composing while one mailbox is open sends from that account; in the combined inbox there
        // is no such context, so the app-level default send account decides.
        BeginCompose(new ComposeContext(
            RichComposeKind.New,
            Account: null,
            Key: null,
            InitialFrom: Model.SendAccount(Model.SelectedAccount)?.Id,
            InitialTo: string.Empty,
            InitialCc: string.Empty,
            Quote: null,
            QuoteStyle: quoting.Style,
            QuoteStylePerMessage: quoting.PerMessage));
    }

    /// <summary>Opens the reply (or reply-all) composer for a message in the reading pane's column,
    /// with the To/Cc the core suggests pre-filled and editable.</summary>
    internal void ComposeReply(string account, string key, bool replyAll, string subject) =>
        BeginCompose(ReplyContext(
            account, key, replyAll, subject, Model.OpenedMessage, Model.Reading));

    /// <summary>Opens the reply (or reply-all) composer for a message read in a detached window, in
    /// a composer window of its own (docs/reading-window.md).</summary>
    /// <remarks>
    /// The quoted original is that window's body, not the pane's: the two are routinely on
    /// different messages, which is the whole point of the window.
    /// </remarks>
    internal void ComposeReplyInWindow(OpenedMessage opened, ReadingBody? body, bool replyAll) =>
        OpenComposerWindow(ReplyContext(
            opened.Account, opened.Key, replyAll, opened.RawSubject, opened, body));

    /// <summary>
    /// Opens an assistant's draft in the composer, <b>unsent</b> (docs/mcp.md), and brings the
    /// window forward, a draft the user cannot see is not the review step this design is built
    /// around, and the request came from another process, so this one does not hold foreground
    /// rights and a bare Activate() would be ignored.
    /// </summary>
    /// <remarks>
    /// Structurally a new message: the same composer, the same Send button, the same submit path,
    /// merely arriving prefilled. It leaves the open composer first, as every route into the
    /// composer does: an assistant's draft arrives unprompted, at any moment, and whatever the
    /// user was writing is kept in Drafts rather than replaced.
    /// </remarks>
    internal async void ComposeAgentDraft(AgentDraft draft)
    {
        await LeaveComposerAsync();
        // Recipients, subject and body are the assistant's; none of them is logged.
        Log.Info("mcp: opening a prefilled draft in the composer");
        // The composer lives in the mail surface's detail column, and a draft can arrive while the
        // calendar or Contacts is up, where it would open behind them, unseen.
        Model.ShowMail();
        BeginCompose(new ComposeContext(
            RichComposeKind.New,
            Account: null,
            Key: null,
            // The draft may name the account to send from; when it does not, the app-level default
            // decides, exactly as a user-initiated new message does.
            InitialFrom: Model.SendAccount(draft.Account ?? Model.SelectedAccount)?.Id,
            InitialTo: draft.To,
            InitialCc: draft.Cc,
            Quote: null,
            QuoteStyle: Model.QuoteSettings.Style,
            QuoteStylePerMessage: Model.QuoteSettings.PerMessage,
            InitialBcc: draft.Bcc,
            InitialSubject: draft.Subject,
            InitialBody: draft.BodyText,
            SeedsSignature: false));
        BringToForeground();
    }

    /// <summary>
    /// Opens a message the core withdrew from the Outbox, <b>unsent</b>, so the user can change it
    /// and send it again (docs/sending.md).
    /// </summary>
    /// <remarks>
    /// <para>
    /// What arrives here is the <b>only</b> copy: the core took it out of the queue before raising
    /// the request, precisely so a drain cannot deliver the message while it is being edited. So
    /// this may not refuse, and it does not need to: leaving the composer already in the pane
    /// keeps that draft in Drafts.
    /// </para>
    /// <para>
    /// The same composer an assistant's draft opens (<see cref="ComposeAgentDraft"/>): a prefilled,
    /// unsent message a person reviews and sends themselves is the same thing either way. It seeds
    /// no signature for the same reason, the body already carries whatever was on it when it was
    /// queued, and a second one would be sent with it.
    /// </para>
    /// </remarks>
    // Qualified, and it has to be: this file has both namespaces in scope, and the core's
    // ComposeRequest and this client's ComposeContext are two different things (ComposeContext.cs).
    internal async void ComposeWithdrawnMessage(uniffi.mailcal_bindings.ComposeRequest request)
    {
        // The recipients, the subject and the body are the user's own mail; none is logged.
        Log.Info("outbox: a withdrawn message is going back into the composer");
        var context = new ComposeContext(
            RichComposeKind.New,
            Account: null,
            Key: null,
            InitialFrom: Model.SendAccount(request.Account)?.Id,
            InitialTo: request.To,
            InitialCc: request.Cc,
            Quote: null,
            QuoteStyle: Model.QuoteSettings.Style,
            QuoteStylePerMessage: Model.QuoteSettings.PerMessage,
            InitialBcc: request.Bcc,
            InitialSubject: request.Subject,
            InitialBody: request.BodyText,
            SeedsSignature: false);
        await LeaveComposerAsync();
        // The composer lives in the mail surface's detail column, and Edit is reachable from the
        // Outbox while the calendar or Contacts is up, where it would open unseen.
        Model.ShowMail();
        BeginCompose(context);
        BringToForeground();
        // Only now: the core holds this message and nothing else does, so it may forget it only
        // once a composer on screen has it (docs/sending.md).
        Model.DismissComposeRequest();
    }

    /// <summary>Opens the forward composer for a message in the reading pane's column, holding the
    /// files the original carries (recipients entered fresh).</summary>
    internal async void ComposeForward(string account, string key, string subject) =>
        BeginCompose(await ForwardContextAsync(
            account, key, subject, Model.OpenedMessage, Model.Reading));

    /// <summary>Opens the forward composer for a message read in a detached window, in a composer
    /// window of its own (docs/reading-window.md).</summary>
    internal async void ComposeForwardInWindow(OpenedMessage opened, ReadingBody? body) =>
        OpenComposerWindow(await ForwardContextAsync(
            opened.Account, opened.Key, opened.RawSubject, opened, body));

    // Swap the detail column over to a freshly-built composer. Any composer already up is torn down
    // first: the caller has already left it (LeaveComposerAsync), so what was written there is in
    // Drafts.
    private void BeginCompose(ComposeContext context)
    {
        TeardownComposer();

        var composer = new ComposerView();
        composer.Init(Model, context, CloseComposer);
        _composer = composer;
        ComposerHost.Content = composer;
        ComposerHost.Visibility = Visibility.Visible;

        // Two WebView2s would otherwise be alive at once, the message body sitting loaded behind a
        // composer nobody can see it through. Unload the reading body while composing; it re-renders
        // from the model when the pane comes back.
        ReadingPanel.Visibility = Visibility.Collapsed;
        ReadingPanel.SuspendBody();
    }

    /// <summary>
    /// Closes the composer and gives the detail column back to the reading pane. Called on Send
    /// (after the draft is queued), on Discard, and by the list when the user opens another message
    /// and the composer has been left, without that last one the message would open *behind* a
    /// composer still covering the column, and the click would look like it did nothing.
    ///
    /// A no-op when nothing is composing, so callers needn't check.
    /// </summary>
    internal void CloseComposer()
    {
        if (_composer is null)
        {
            return;
        }
        TeardownComposer();
        ComposerHost.Visibility = Visibility.Collapsed;
        ReadingPanel.Visibility = Visibility.Visible;
        ReadingPanel.ResumeBody();
    }

    private void TeardownComposer()
    {
        if (_composer is null)
        {
            return;
        }
        _composer.Teardown();
        _composer = null;
        ComposerHost.Content = null;
    }

    /// <summary>
    /// Leaves the open composer before something takes its place: another message, a different
    /// compose, a link, a share or an assistant's draft. What was written in it is kept in Drafts,
    /// and nobody is asked (docs/drafts.md, "Leaving a composer"). A no-op with no composer.
    /// </summary>
    /// <remarks>
    /// This only finishes the composition; the caller decides what replaces the composer, either
    /// closing it (<see cref="CloseComposer"/>) or building the next one (<c>BeginCompose</c>).
    /// </remarks>
    internal async Task LeaveComposerAsync()
    {
        if (_composer is not null)
        {
            await _composer.LeaveAsync();
        }
    }
}
