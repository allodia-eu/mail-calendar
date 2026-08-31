// The compose-request state: the one place that decides whether the detail column shows the
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

using Allodia.Mailcal.Dialogs;
using Allodia.Mailcal.Services;
using Allodia.Mailcal.ViewModels;
using Allodia.Mailcal.Views;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Controls;
using uniffi.mailcal_bindings;

namespace Allodia.Mailcal;

public sealed partial class MainWindow
{
    /// <summary>The composer currently in the detail column, or <c>null</c> when the reading pane
    /// has it. Built fresh per draft and torn down on Send/Cancel, never reused across messages,
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
        BeginCompose(new ComposeRequest(
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

    /// <summary>Opens the reply (or reply-all) composer for a message, with the To/Cc the core
    /// suggests pre-filled and editable.</summary>
    internal void ComposeReply(string account, string key, bool replyAll)
    {
        var prefill = Model.ReplyRecipients(account, key, replyAll);
        var quoting = Model.QuoteSettings;
        BeginCompose(new ComposeRequest(
            replyAll ? RichComposeKind.ReplyAll : RichComposeKind.Reply,
            account,
            key,
            // A reply opens on the account that received the mail; the user may still send it out
            // from another, and the core resolves the original in its own account either way.
            InitialFrom: Model.SendAccount(account)?.Id,
            InitialTo: prefill?.To ?? string.Empty,
            InitialCc: prefill?.Cc ?? string.Empty,
            Quote: QuoteSeedFor(account, key, isForward: false),
            QuoteStyle: quoting.Style,
            QuoteStylePerMessage: quoting.PerMessage));
    }

    /// <summary>
    /// Opens an assistant's draft in the composer, <b>unsent</b> (docs/mcp.md), and brings the
    /// window forward, a draft the user cannot see is not the review step this design is built
    /// around, and the request came from another process, so this one does not hold foreground
    /// rights and a bare Activate() would be ignored.
    /// </summary>
    /// <remarks>
    /// Structurally a new message: the same composer, the same Send button, the same submit path,
    /// merely arriving prefilled. Behind the same discard guard a message click uses, an assistant
    /// asking to open a draft arrives unprompted, at any moment, and must not be able to throw away
    /// a half-written message the user is in the middle of.
    /// </remarks>
    internal async void ComposeAgentDraft(AgentDraft draft)
    {
        if (!await ConfirmDiscardDraftAsync())
        {
            Log.Info("mcp: a prefilled draft was declined, the open draft was kept");
            return;
        }
        // Recipients, subject and body are the assistant's; none of them is logged.
        Log.Info("mcp: opening a prefilled draft in the composer");
        // The composer lives in the mail surface's detail column, and a draft can arrive while the
        // calendar or Contacts is up, where it would open behind them, unseen.
        Model.ShowMail();
        BeginCompose(new ComposeRequest(
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

    /// <summary>Opens the forward composer for a message (recipients entered fresh).</summary>
    internal void ComposeForward(string account, string key)
    {
        var quoting = Model.QuoteSettings;
        BeginCompose(new ComposeRequest(
            RichComposeKind.Forward,
            account,
            key,
            InitialFrom: Model.SendAccount(account)?.Id,
            InitialTo: string.Empty,
            InitialCc: string.Empty,
            Quote: QuoteSeedFor(account, key, isForward: true),
            QuoteStyle: quoting.Style,
            QuoteStylePerMessage: quoting.PerMessage));
    }

    // The quoted original for a reply/forward of (account, key). There is something to quote only
    // when that message is the one open in the reading pane, its sanitised body is what the quote
    // seeds from. Replying from the list's context menu to a row that has never been opened
    // therefore quotes nothing, which is what the dialog did too.
    private string? QuoteSeedFor(string account, string key, bool isForward)
    {
        if (Model.OpenedMessage is not { } opened || opened.Account != account || opened.Key != key)
        {
            return null;
        }
        // Lengths only, never content (docs/logging.md). Worth a line: a quoted original that
        // arrives with no HTML half is the case that used to render as an empty quote, and the
        // difference is invisible on screen once it works.
        Log.Info($"quote: seeding from html={Model.Reading?.Html?.Length ?? -1} plain={Model.Reading?.Plain?.Length ?? -1} chars");
        // In showcase mode the designated message also seeds sample reply text, so the store
        // screenshot shows a written reply rather than an empty composer.
        return ComposerQuote.SeedJson(
            Model.QuoteSettings.Style,
            opened,
            Model.Reading,
            isForward,
            isForward ? null : ShowcaseMode.ReplyText(opened.Account, opened.Key));
    }

    // Swap the detail column over to a freshly-built composer. Any composer already up is torn down
    // first, the caller has already asked the user about an unsent draft (ConfirmDiscardDraftAsync),
    // so reaching here means it may go.
    private void BeginCompose(ComposeRequest request)
    {
        TeardownComposer();

        var composer = new ComposerView();
        composer.Init(Model, request, CloseComposer);
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
    /// (after the draft is queued), on Cancel, and by the list when the user opens another message
    /// and has let the draft go, without that last one the message would open *behind* a composer
    /// still covering the column, and the click would look like it did nothing.
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
    /// Asks the user before an action that would drop the open draft, opening another message,
    /// or starting a different compose. Returns <c>true</c> when the action may proceed: there is
    /// no composer, nothing has been written into it, or the user chose Discard. Returns
    /// <c>false</c> for Keep editing, and the caller abandons whatever it was about to do.
    ///
    /// The composer being a pane rather than a modal is exactly what makes this reachable: a click
    /// on another message was impossible while the dialog was up. Silently losing a draft to that
    /// click was not an option.
    /// </summary>
    internal async Task<bool> ConfirmDiscardDraftAsync()
    {
        if (_composer is null || !await _composer.IsDirtyAsync())
        {
            return true;
        }
        // "Keep editing" rather than the helper's default "Cancel", next to "Discard", a button
        // labelled Cancel reads ambiguously as "cancel the draft".
        var result = await DialogHelper.ConfirmAsync(
            Content.XamlRoot,
            L10n.ComposeDiscardTitle(),
            L10n.ComposeDiscardMessage(),
            L10n.ActionDiscard(),
            L10n.ActionKeepEditing());
        return result == ContentDialogResult.Primary;
    }
}
