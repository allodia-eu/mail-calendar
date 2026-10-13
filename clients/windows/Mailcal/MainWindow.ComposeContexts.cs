// What every composer OPENS WITH, in one place, for both hosts.
//
// A new message, a reply, a reply-all, a forward, a resumed draft, a mail link, a share, an
// assistant's draft and an Outbox edit all open here: the core is asked once (open_composer, or the
// request it raises for an Outbox edit), and its answer is turned into the composer's context by one
// builder. The shell renders a composer in two places, the reading pane's column and a composer
// window raised out of a reading window (docs/reading-window.md); where it is rendered is
// MainWindow.Compose.cs and MainWindow.ReadingWindows.cs, and what is in it is here, once.

using System.Collections.Generic;
using System.Threading.Tasks;
using Allodia.Mailcal.Dialogs;
using Allodia.Mailcal.Services;
using Allodia.Mailcal.ViewModels;
using uniffi.mailcal_bindings;

namespace Allodia.Mailcal;

public sealed partial class MainWindow
{
    private bool _composerOpening;

    /// <summary>
    /// Opens a composer on <paramref name="origin"/> and answers with what it holds, or <c>null</c>
    /// when it could not open, which this has already said, or when another open is still waiting
    /// on the core.
    /// </summary>
    /// <remarks>
    /// The core is asked off the UI thread: a reply may have to fetch the message it quotes, and a
    /// draft the copy on the server. The composer appears when the answer does, holding everything:
    /// a composer shown before its files arrived could be sent, or a draft saved, without them.
    /// One open at a time: a second Reply pressed while the first fetches would otherwise open over
    /// it, and whichever answer arrived last would hold the pane.
    /// </remarks>
    private async Task<ComposeContext?> OpenComposerAsync(
        ComposeOrigin origin,
        ComposePrefill? prefill = null,
        IReadOnlyList<ComposerFileAttachment>? extraAttachments = null,
        string? initialError = null)
    {
        if (_composerOpening)
        {
            return null;
        }
        _composerOpening = true;
        ComposeOpening? opening;
        try
        {
            var composition = Guid.NewGuid().ToString("N");
            var directory = MailboxModel.ComposerStagingDirectory();
            opening = await Task.Run(() => Model.OpenComposer(composition, origin, prefill, directory));
        }
        finally
        {
            _composerOpening = false;
        }
        if (opening is null)
        {
            await DialogHelper.TellAsync(
                Content.XamlRoot,
                origin is ComposeOrigin.Draft ? L10n.ComposeDraftOpenFailed() : L10n.ComposeOpenFailed());
            return null;
        }
        return ContextFor(opening, extraAttachments, initialError);
    }

    /// <summary>The composer's context for what the core opened it with.</summary>
    /// <remarks>
    /// The quote's attribution is worded here, in the user's language and with the original's date
    /// as the reading header shows it: for a fresh reply or forward it seeds the quote, and for a
    /// reopened one it completes the style the stored quote does not carry.
    /// </remarks>
    internal ComposeContext ContextFor(
        ComposeOpening opening,
        IReadOnlyList<ComposerFileAttachment>? extraAttachments = null,
        string? initialError = null)
    {
        var quotes = Model.QuoteSettings;
        var words = opening.Quote is { } quote
            ? ComposerQuote.Words(
                quote,
                TimeZones.LocalDateTime(quote.Date, Model.ActiveZone),
                opening.Kind == ComposeKind.Forward)
            : null;
        return new ComposeContext(
            opening,
            InitialFrom: Model.SendAccount(opening.Account)?.Id,
            QuoteStyle: quotes.Style,
            QuoteStylePerMessage: quotes.PerMessage,
            Quote: ComposeOpeningSeed.QuoteSeed(opening, words, ComposerQuote.Token(quotes.Style), opening.BodyText),
            QuoteAttribution: ComposeOpeningSeed.AttributionCompletion(opening, words),
            ExtraAttachments: extraAttachments,
            InitialError: initialError);
    }

    /// <summary>A reply's origin, with the sample answer showcase mode puts above its quote so the
    /// store screenshot shows a written reply rather than an empty composer.</summary>
    private static (ComposeOrigin Origin, ComposePrefill? Prefill) ReplyOrigin(string account, string key, bool replyAll)
    {
        var origin = new ComposeOrigin.Reply(account, key, replyAll);
        return ShowcaseMode.ReplyText(account, key) is { } text
            ? (origin, new ComposePrefill(string.Empty, string.Empty, string.Empty, string.Empty, text, false, null))
            : (origin, null);
    }
}
