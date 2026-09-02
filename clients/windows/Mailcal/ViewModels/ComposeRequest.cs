// What the reading-pane composer is currently composing. One request replaces the five
// dialog-construction call sites the composer used to have (three in MailListView, two in
// ReadingView): the shell holds at most one of these, and it is the single thing that decides
// whether the detail column shows the reading pane or the composer.
//
// The Windows twin of the Apple client's `ComposeContext` (Mailcal.swift's `@State var compose`),
// deliberately the same shape, an optional request, not a presented modal.

using System.Collections.Generic;
using Allodia.Mailcal.Dialogs;
using uniffi.mailcal_bindings;

namespace Allodia.Mailcal.ViewModels;

// RichComposeKind itself lives in RichComposeKind.cs, WinUI- and L10n-free, so the rules it drives
// (the signature slot) can be linked into the test project.

/// <summary>
/// One open composer: what it is for, the original it answers (reply/forward), and the state it
/// opens with. <paramref name="Account"/>/<paramref name="Key"/> identify the original message and
/// are <c>null</c> for a new message. <paramref name="InitialFrom"/> is the account the From picker
/// opens on, the one that received the mail for a reply/forward, the selected mailbox's for a new
/// message. <paramref name="Quote"/> is the quoted-original seed, <c>null</c> when there is nothing
/// to quote (a new message, or a reply to a row whose body hasn't been read yet).
/// </summary>
/// <param name="Kind">Whether this is a new message, reply, reply-all, or forward.</param>
/// <param name="Account">The original's owning account, or <c>null</c> for a new message.</param>
/// <param name="Key">The original's message key, or <c>null</c> for a new message.</param>
/// <param name="InitialFrom">The account id the From picker opens on.</param>
/// <param name="InitialTo">The pre-filled To recipients (reply/reply-all), else empty.</param>
/// <param name="InitialCc">The pre-filled Cc recipients (reply-all), else empty.</param>
/// <param name="Quote">The quoted-original seed JSON, or <c>null</c>.</param>
/// <param name="QuoteStyle">The style the quote is seeded in (the persisted app default).</param>
/// <param name="QuoteStylePerMessage">Whether the user opted into choosing the style per message,
/// which is the only case where the composer shows a style picker at all.</param>
/// <param name="InitialBcc">The pre-filled Bcc recipients. Only an assistant's draft
/// (docs/mcp.md) fills this, the user's own compose paths open it empty.</param>
/// <param name="InitialSubject">The pre-filled Subject, for a kind that shows one.</param>
/// <param name="InitialBody">A plain-text body to seed the editor with, or <c>null</c>. Mutually
/// exclusive with <paramref name="Quote"/> in practice: an assistant's draft is a new message.</param>
/// <param name="SeedsSignature">Whether the account's signature is seeded and the picker offered.
/// False for an assistant's draft, which arrives with a body someone else wrote and its own
/// sign-off, matching macOS, which passes the composer no signature library at all in that case.</param>
/// <param name="Attachments">Files the composer opens already holding, from a share
/// (<c>docs/os-integration.md</c>). Each is the shared core's answer about one shared item, name
/// and media type included, so the list is displayed as given and never re-derived. Empty for
/// every other route: the picker fills it. Removable like any picked file, a share proposes an
/// attachment, it does not impose one.</param>
public sealed record ComposeRequest(
    RichComposeKind Kind,
    string? Account,
    string? Key,
    string? InitialFrom,
    string InitialTo,
    string InitialCc,
    string? Quote,
    QuoteStyleChoice QuoteStyle,
    bool QuoteStylePerMessage,
    string InitialBcc = "",
    string InitialSubject = "",
    string? InitialBody = null,
    bool SeedsSignature = true,
    IReadOnlyList<ComposerFileAttachment>? Attachments = null)
{
    /// <summary>The composer's heading, the action it is performing.</summary>
    public string Title => Kind switch
    {
        RichComposeKind.Reply => L10n.ActionReply(),
        RichComposeKind.ReplyAll => L10n.ActionReplyAll(),
        RichComposeKind.Forward => L10n.ActionForward(),
        _ => L10n.ComposeTitleNew(),
    };

    /// <summary>Only a new message edits the Subject, a reply/forward derives <c>Re:</c>/<c>Fwd:</c>
    /// in the core, so showing an editable field would imply an override that doesn't exist.</summary>
    public bool ShowsSubject => Kind == RichComposeKind.New;

    /// <summary>Whether the composer shows the per-message quote-style picker: there is a quoted
    /// original to style, and the user opted into per-message styling in Settings.</summary>
    public bool ShowsStylePicker => ComposerQuote.ShowsStylePicker(Quote is not null, QuoteStylePerMessage);

    /// <summary>Whether the composer opens with the caret in the message body. A reply/forward
    /// already has its From/To/Subject filled in, so writing is the only thing left to do; a new
    /// message starts in its empty To field instead, unless something already filled it in, which
    /// is a mail link or an assistant's draft, and then the body is where the user has to start
    /// there too.</summary>
    public bool FocusesBody => Kind != RichComposeKind.New || !string.IsNullOrWhiteSpace(InitialTo);
}
