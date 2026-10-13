// The compose state: the one place that decides whether the detail column shows the
// reading pane or the composer.
//
// The composer used to be a ContentDialog, constructed independently at five call sites (three in
// MailListView, two in ReadingView), a modal that blacked out the mailbox behind it. It now
// REPLACES the reading pane: the sidebar and the message list stay live and clickable while you
// write. Every route into it opens through OpenComposerAsync (MainWindow.ComposeContexts.cs), so
// what a composer holds is the core's one answer whichever way it was reached.
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
    internal async void ComposeNew()
    {
        // Composing while one mailbox is open sends from that account; in the combined inbox there
        // is no such context, and the core decides.
        if (await OpenComposerAsync(new ComposeOrigin.New(Model.SelectedAccount)) is { } context)
        {
            BeginCompose(context);
        }
    }

    /// <summary>Opens the reply (or reply-all) composer for a message in the reading pane's column,
    /// with the To/Cc the core suggests pre-filled and editable.</summary>
    internal async void ComposeReply(string account, string key, bool replyAll)
    {
        var (origin, prefill) = ReplyOrigin(account, key, replyAll);
        if (await OpenComposerAsync(origin, prefill) is { } context)
        {
            BeginCompose(context);
        }
    }

    /// <summary>Opens the reply (or reply-all) composer for a message read in a detached window, in
    /// a composer window of its own (docs/reading-window.md).</summary>
    internal async void ComposeReplyInWindow(OpenedMessage opened, bool replyAll)
    {
        var (origin, prefill) = ReplyOrigin(opened.Account, opened.Key, replyAll);
        if (await OpenComposerAsync(origin, prefill) is { } context)
        {
            OpenComposerWindow(context);
        }
    }

    /// <summary>
    /// Opens an assistant's draft in the composer, <b>unsent</b> (docs/mcp.md), and brings the
    /// window forward, a draft the user cannot see is not the review step this design is built
    /// around, and the request came from another process, so this one does not hold foreground
    /// rights and a bare Activate() would be ignored.
    /// </summary>
    /// <remarks>
    /// The same composer, the same Send button, the same submit path, merely arriving prefilled: a
    /// reply when it names the message it answers, so it stays in that conversation, and a new
    /// message otherwise. It leaves the open composer first, as every route into the composer does:
    /// an assistant's draft arrives unprompted, at any moment, and whatever the user was writing is
    /// kept in Drafts rather than replaced.
    /// </remarks>
    internal async void ComposeAgentDraft(AgentDraft draft)
    {
        await LeaveComposerAsync();
        // Recipients, subject and body are the assistant's; none of them is logged.
        Log.Info("mcp: opening a prefilled draft in the composer");
        // The composer lives in the mail surface's detail column, and a draft can arrive while the
        // calendar or Contacts is up, where it would open behind them, unseen.
        Model.ShowMail();
        ComposeOrigin origin = draft is { ReplyToAccount: { } account, ReplyToKey: { } key }
            ? new ComposeOrigin.Reply(account, key, false)
            : new ComposeOrigin.New(draft.Account ?? Model.SelectedAccount);
        // Signed: the assistant chose how to sign its words, so the account's signature is not seeded
        // under them (docs/mcp.md).
        var prefill = new ComposePrefill(draft.To, draft.Cc, draft.Bcc, draft.Subject, draft.BodyText, true);
        if (await OpenComposerAsync(origin, prefill) is { } context)
        {
            BeginCompose(context);
            BringToForeground();
        }
    }

    /// <summary>
    /// Opens a queued send the core moved back into Drafts, <b>unsent</b>, so the user can change
    /// it and send it again (docs/sending.md).
    /// </summary>
    /// <remarks>
    /// The core saved it as a draft and took it out of the queue before raising the request, so a
    /// drain cannot deliver the message while it is being edited, and joined it to the composition
    /// the request names, which the composer opens on: never a fresh one, or the first save would
    /// store a second draft beside it. It holds every staged file, because that save replaces the
    /// draft and a file left out is taken off it. This may not refuse, and it does not need to:
    /// leaving the composer already in the pane keeps that draft in Drafts.
    /// </remarks>
    internal async void ComposeWithdrawnMessage(ComposeOpening opening)
    {
        // The recipients, the subject and the body are the user's own mail; none is logged.
        Log.Info($"outbox: a queued message is going back into the composer with {opening.Attachments.Length} files");
        var context = ContextFor(opening);
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
    internal async void ComposeForward(string account, string key)
    {
        if (await OpenComposerAsync(new ComposeOrigin.Forward(account, key)) is { } context)
        {
            BeginCompose(context);
        }
    }

    /// <summary>Opens the forward composer for a message read in a detached window, in a composer
    /// window of its own (docs/reading-window.md).</summary>
    internal async void ComposeForwardInWindow(OpenedMessage opened)
    {
        if (await OpenComposerAsync(new ComposeOrigin.Forward(opened.Account, opened.Key)) is { } context)
        {
            OpenComposerWindow(context);
        }
    }
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
