// What the reading-pane composer is currently composing: the core's answer about the message
// (ComposeOpening, from open_composer, or raised when a queued send is edited), plus the little the
// host adds to it. One way in for every composer, a new message, a reply, a forward, a resumed
// draft, a link, a share, an assistant's draft and an Outbox edit, so no route can open a composer
// that holds less than another (docs/drafts.md).
//
// The Windows twin of the Apple client's `ComposeContext`, deliberately the same shape, an optional
// context, not a presented modal.

using System.Collections.Generic;
using System.Linq;
using Allodia.Mailcal.Dialogs;
using Allodia.Mailcal.Services;
using uniffi.mailcal_bindings;

namespace Allodia.Mailcal.ViewModels;

// RichComposeKind itself lives in RichComposeKind.cs, WinUI- and L10n-free, so the rules it drives
// (the signature slot) can be linked into the test project.

/// <summary>One open composer: the core's answer about the message it opens on, and what the host
/// derived from it in the user's language.</summary>
/// <param name="Opening">What the core opened the composer with, on the composition it joined.</param>
/// <param name="InitialFrom">The account id the From picker opens on: the opening's account as a
/// sending account.</param>
/// <param name="QuoteStyle">The style a fresh quote is seeded in (the persisted app default).</param>
/// <param name="QuoteStylePerMessage">Whether the user opted into choosing the style per message,
/// which is the only case where the composer shows a style picker at all.</param>
/// <param name="Quote">The <c>setComposerQuote</c> seed for a fresh reply or forward, or
/// <c>null</c>.</param>
/// <param name="QuoteAttribution">The <c>completeComposerQuoteAttribution</c> argument for a
/// reopened message whose original is known, or <c>null</c>.</param>
/// <param name="ExtraAttachments">Files the host adds to what the core staged: a share's.</param>
/// <param name="InitialError">A line the composer opens showing, for something the route could not
/// bring: files a share could not hand over.</param>
// Internal, not public: it carries generated records, and every record the C# bindgen emits is
// internal. The public types in this directory are the ones XAML binds to; this one is only ever
// constructed and read from code-behind.
internal sealed record ComposeContext(
    ComposeOpening Opening,
    string? InitialFrom,
    QuoteStyleChoice QuoteStyle,
    bool QuoteStylePerMessage,
    string? Quote,
    string? QuoteAttribution,
    IReadOnlyList<ComposerFileAttachment>? ExtraAttachments = null,
    string? InitialError = null)
{
    /// <summary>Whether this is a new message, reply, reply-all, or forward.</summary>
    public RichComposeKind Kind => ComposeOpeningSeed.KindOf(Opening.Kind);

    /// <summary>The composition the composer opens on: every save, the discard and the send name
    /// it, because the core joined it to the message when the composer opened.</summary>
    public string Composition => Opening.Composition;

    public string InitialTo => Opening.To;
    public string InitialCc => Opening.Cc;
    public string InitialBcc => Opening.Bcc;
    public string InitialSubject => Opening.Subject;

    /// <summary>A stored message's HTML, which the editor reads back into its document.</summary>
    public string? StoredHtml => Opening.BodyHtml.Length > 0 ? Opening.BodyHtml : null;

    /// <summary>The text the editor opens with when there is neither a stored body nor a quote.
    /// With a stored body it is that body's text, for one with no HTML.</summary>
    public string? InitialBody => StoredHtml is not null
        ? Opening.BodyText
        : ComposeOpeningSeed.PlainBody(Opening, Quote);

    /// <summary>Whether the account's signature is seeded: a stored message carries its own.</summary>
    public bool SeedsSignature => Opening.SeedsSignature;

    /// <summary>Every file the composer opens holding: what the core staged, and a share's.</summary>
    public IReadOnlyList<ComposerFileAttachment> Attachments =>
        Opening.Attachments.Concat(ExtraAttachments ?? []).ToList();

    /// <summary>Whether a forward's originals could not be staged, so the composer says so.</summary>
    public bool AttachmentsFailed => Opening.AttachmentsFailed;

    /// <summary>The composer's heading, the action it is performing.</summary>
    public string Title => Kind switch
    {
        RichComposeKind.Reply => L10n.ActionReply(),
        RichComposeKind.ReplyAll => L10n.ActionReplyAll(),
        RichComposeKind.Forward => L10n.ActionForward(),
        _ => L10n.ComposeTitleNew(),
    };

    /// <summary>Whether the composer shows the per-message quote-style picker: there is a quote
    /// whose both attributions are known, a fresh one or a reopened one whose original is still
    /// here, and the user opted into per-message styling in Settings.</summary>
    public bool ShowsStylePicker =>
        ComposerQuote.ShowsStylePicker(Opening.Quote is not null, QuoteStylePerMessage);

    /// <summary>Whether the composer opens with the caret in the message body. A reply/forward
    /// already has its From/To/Subject filled in, so writing is the only thing left to do; a new
    /// message starts in its empty To field instead, unless something already filled it in, which
    /// is a mail link or an assistant's draft, and then the body is where the user has to start
    /// there too.</summary>
    public bool FocusesBody => Kind != RichComposeKind.New || !string.IsNullOrWhiteSpace(InitialTo);
}
