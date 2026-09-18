// What a reply, a reply-all or a forward OPENS WITH, in one place, for both hosts.
//
// The shell renders a draft in two places now: in the reading pane's own column, and in a composer
// window raised out of a reading window (docs/reading-window.md). Where it is rendered is
// MainWindow.Compose.cs and MainWindow.ReadingWindows.cs; what is in it is here, once, so a reply
// raised in a window is seeded exactly as one raised in the pane. Two builders would be two answers
// to which recipients a reply-all suggests and what the quoted original looks like, and they would
// drift a field at a time.
//
// The quoted original is the one input that genuinely differs between the two: the pane quotes what
// the pane is reading, a window quotes what THAT window is reading. So it arrives as a parameter
// rather than being read off the model.

using System.Threading.Tasks;
using Allodia.Mailcal.Dialogs;
using Allodia.Mailcal.Services;
using Allodia.Mailcal.ViewModels;
using uniffi.mailcal_bindings;

namespace Allodia.Mailcal;

public sealed partial class MainWindow
{
    /// <summary>The reply (or reply-all) draft for a message, quoting <paramref name="body"/>.</summary>
    /// <remarks>
    /// <paramref name="quoting"/> and <paramref name="body"/> are the reader the reply was raised
    /// from; pass <c>null</c> for both when there is nothing to quote, which is a reply raised from
    /// the list's context menu on a row nobody has opened.
    /// </remarks>
    private ComposeContext ReplyContext(
        string account, string key, bool replyAll, string subject,
        OpenedMessage? quoting, ReadingBody? body)
    {
        var prefill = Model.ReplyRecipients(account, key, replyAll);
        var quotes = Model.QuoteSettings;
        return new ComposeContext(
            replyAll ? RichComposeKind.ReplyAll : RichComposeKind.Reply,
            account,
            key,
            // A reply opens on the account that received the mail; the user may still send it out
            // from another, and the core resolves the original in its own account either way.
            InitialFrom: Model.SendAccount(account)?.Id,
            InitialTo: prefill?.To ?? string.Empty,
            InitialCc: prefill?.Cc ?? string.Empty,
            Quote: QuoteSeed(account, key, quoting, body, isForward: false),
            // Derived by the CORE, not here: the field is editable, so what it opens with is what
            // gets sent unless the user changes it, and a client-side "Re: " + subject differs
            // from the core's on a reply to a reply.
            InitialSubject: MailcalBindingsMethods.ReplySubject(subject),
            QuoteStyle: quotes.Style,
            QuoteStylePerMessage: quotes.PerMessage);
    }

    /// <summary>The forward draft for a message, holding the files the original carries.</summary>
    /// <remarks>
    /// Staging is awaited before the composer is built, never after: a composer on screen holding
    /// nothing can be sent in the window before the files arrive, which is the forward without its
    /// attachments this staging exists to prevent. It reads from the raw source the reading view
    /// has already cached, so in the ordinary case there is nothing to wait for.
    /// </remarks>
    private async Task<ComposeContext> ForwardContextAsync(
        string account, string key, string subject,
        OpenedMessage? quoting, ReadingBody? body)
    {
        var quotes = Model.QuoteSettings;
        var quote = QuoteSeed(account, key, quoting, body, isForward: true);
        var directory = Path.Combine(
            Path.GetTempPath(),
            "forward-attachments",
            Guid.NewGuid().ToString("N"));
        var staged = await Task.Run(
            () => Model.StageForwardedAttachments(account, key, directory));
        return new ComposeContext(
            RichComposeKind.Forward,
            account,
            key,
            InitialFrom: Model.SendAccount(account)?.Id,
            InitialTo: string.Empty,
            InitialCc: string.Empty,
            Quote: quote,
            QuoteStyle: quotes.Style,
            QuoteStylePerMessage: quotes.PerMessage,
            InitialSubject: MailcalBindingsMethods.ForwardSubject(subject),
            Attachments: staged,
            AttachmentsFailed: staged is null);
    }

    // The quoted original for a reply/forward of (account, key). There is something to quote only
    // when the reader the draft was raised from is on that very message: its sanitised body is what
    // the quote seeds from. Replying from the list's context menu to a row that has never been
    // opened therefore quotes nothing, which is what the dialog did too.
    private string? QuoteSeed(
        string account, string key, OpenedMessage? quoting, ReadingBody? body, bool isForward)
    {
        if (quoting is not { } opened || opened.Account != account || opened.Key != key)
        {
            return null;
        }
        // Lengths only, never content (docs/logging.md). Worth a line: a quoted original that
        // arrives with no HTML half is the case that used to render as an empty quote, and the
        // difference is invisible on screen once it works.
        Log.Info($"quote: seeding from html={body?.Html?.Length ?? -1} plain={body?.Plain?.Length ?? -1} chars");
        // In showcase mode the designated message also seeds sample reply text, so the store
        // screenshot shows a written reply rather than an empty composer.
        return ComposerQuote.SeedJson(
            Model.QuoteSettings.Style,
            opened,
            body,
            isForward,
            isForward ? null : ShowcaseMode.ReplyText(opened.Account, opened.Key));
    }
}
