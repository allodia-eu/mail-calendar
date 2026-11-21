// Draft a reply as the core draws it for each composer (docs/ai.md rules 17 to 24). Nothing is
// cached here: a composer reads its record from the core's memory each time it draws.

using uniffi.mailcal_bindings;

namespace Allodia.Mailcal.Services;

public sealed partial class MailboxModel
{
    /// <summary>Raised when a composer's Draft a reply may have changed (<c>Surface.ReplyDraft</c>,
    /// and <c>Surface.WritingStyle</c>, since a style learned or chosen meanwhile reaches an open
    /// composer): every open composer reads its own record again.</summary>
    internal event Action? ReplyDraftChanged;

    /// <summary>Draft a reply for <paramref name="composition"/>'s composer sending from
    /// <paramref name="from"/>, or <c>null</c>: not offered, or not a reply. Read from memory.</summary>
    internal ComposerReplyDraft? ComposerReplyDraft(string composition, string? from) =>
        _app?.ComposerReplyDraft(composition, from);

    /// <summary>A change to a composition's Draft a reply. Applied before it returns; a draft itself
    /// is made off this thread.</summary>
    internal void ChangeReplyDraft(ReplyDraftIntent intent) =>
        _app?.Dispatch(new Intent.Ai(new AiIntent.ReplyDraft(intent)));

    /// <summary>Hands the core a message the composition's editor posted; only its reply-draft
    /// report means anything here.</summary>
    internal void ReplyDraftReport(string composition, string message) =>
        _app?.ComposerReplyDraftReport(composition, message);

    private void RaiseReplyDraftChanged() => ReplyDraftChanged?.Invoke();
}
