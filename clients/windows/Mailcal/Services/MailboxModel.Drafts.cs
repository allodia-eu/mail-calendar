// Keeping the message being composed on the server: the core verbs, as the composer calls them
// (docs/drafts.md).
//
// Every one of them names a composition, which is the host's handle on one open composer. The core
// mints none: a composer takes an id when it opens and keeps it until it closes, and that is what
// lets two composers save without superseding each other's draft.
//
// Its own file rather than an addition to MailboxModel.Composer.cs, which is the send half.

using uniffi.mailcal_bindings;

namespace Allodia.Mailcal.Services;

public sealed partial class MailboxModel
{
    /// <summary>
    /// Stores what the composer holds in the Drafts folder, replacing what this
    /// <paramref name="composition"/>'s previous save left there.
    /// </summary>
    /// <remarks>
    /// Always the file-carrying call, even from a composer holding none. A save replaces the
    /// stored copy, so one that left the files out would take them off a draft the user is still
    /// writing, and a composer can pick a file at any moment.
    /// <para>Fire and forget: it returns as soon as the document validates, and the outcome
    /// arrives as a <c>Surface.DraftStatus</c> signal. A save made with no network is queued, not
    /// lost. Only the error category is surfaced for diagnostics; the body is never logged.</para>
    /// </remarks>
    internal void SaveDraft(
        string composition,
        Recipients recipients,
        string subject,
        string documentJson,
        ComposerFileAttachment[] files,
        string? from)
    {
        if (_app is null)
        {
            return;
        }
        try
        {
            _app.SaveDraftWithFiles(composition, recipients, subject, documentJson, files, from);
        }
        catch (Exception ex)
        {
            Log.Warn($"draft save failed: {ex.GetType().Name}");
        }
    }

    /// <summary>Removes this composition's stored draft from the server and forgets the
    /// composition. Safe on a composer that never saved: it reaches no server.</summary>
    internal void DiscardDraft(string composition)
    {
        try
        {
            _app?.DiscardDraft(composition);
        }
        catch (Exception ex)
        {
            Log.Warn($"draft discard failed: {ex.GetType().Name}");
        }
    }

    /// <summary>
    /// Forgets the composition, leaving the stored draft where it is: what closing a composer
    /// means. Called however the composer went, sent, discarded or dismissed, because without it
    /// the core holds a record per composer for the life of the process.
    /// </summary>
    internal void CloseComposition(string composition)
    {
        try
        {
            _app?.CloseComposition(composition);
        }
        catch (Exception ex)
        {
            Log.Warn($"composition close failed: {ex.GetType().Name}");
        }
    }

    /// <summary>
    /// How <paramref name="composition"/>'s most recent save ended. A <c>Surface.DraftStatus</c>
    /// signal says that some composition's save moved, not which, so every open composer re-pulls
    /// its own by naming it; one that has saved nothing reads <c>Idle</c>.
    /// </summary>
    internal DraftStatus DraftStatusOf(string composition)
    {
        try
        {
            return _app?.DraftStatus(composition) ?? DraftStatus.Idle;
        }
        catch (Exception)
        {
            return DraftStatus.Idle;
        }
    }

    /// <summary>
    /// Opens the stored draft <paramref name="key"/> (on <paramref name="account"/>) into
    /// <paramref name="composition"/>, so the composer about to show it saves over that copy
    /// rather than beside it.
    /// </summary>
    /// <remarks>
    /// <c>null</c> means the draft could not be opened, which the caller says rather than showing
    /// an empty composer: nothing is adopted on a failure, and a composer opened without the
    /// draft's content would replace it with what is on screen the next time it saved.
    /// <para>Blocks on the core's runtime, and unlike a forward's staging it cannot count on a
    /// warm cache: a draft is opened from a list row, so the first open of one fetches the
    /// message. Run it off the UI thread.</para>
    /// </remarks>
    internal DraftResume? ResumeDraft(
        string composition,
        string account,
        string key,
        string stagingDirectory)
    {
        if (_app is null)
        {
            return null;
        }
        try
        {
            return _app.ResumeDraft(composition, account, key, stagingDirectory);
        }
        catch (Exception ex)
        {
            Log.Warn($"draft resume failed: {ex.GetType().Name}");
            return null;
        }
    }
}
