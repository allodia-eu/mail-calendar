// Rich composer commands for the Windows spike. Kept separate so MailboxModel.cs stays
// below the repo's 500-line file limit.

using uniffi.mailcal_bindings;

namespace Allodia.Mailcal.Services;

public sealed partial class MailboxModel
{
    /// <summary>
    /// Validate and queue a rich composer document to the entered <paramref name="recipients"/>.
    /// The body text/HTML is never logged; only the error category is surfaced for diagnostics.
    /// <paramref name="from"/> is the account the user picked in the composer's From dropdown; it
    /// decides both the <c>From:</c> identity and the outbox the draft goes out through.
    /// <c>null</c> lets the core derive it. An id naming an account that is no longer configured
    /// fails the send rather than substituting another sender.
    /// </summary>
    internal bool SubmitRich(
        Recipients recipients,
        string subject,
        string documentJson,
        ComposerFileAttachment[] files,
        string? from)
    {
        if (_app is null || BlockedByHarnessGate(recipients))
        {
            return false;
        }
        try
        {
            _app.SubmitRichMailWithFiles(recipients, subject, documentJson, files, from);
            return true;
        }
        catch (Exception ex)
        {
            Log.Warn($"rich composer submit failed: {ex.GetType().Name}");
            return false;
        }
    }

    /// <summary>
    /// Validate and queue a rich reply (or reply-all) to a message (by owning account + key)
    /// with the user-confirmed <paramref name="recipients"/> and <paramref name="subject"/>; the
    /// app derives the threading from the original. Only the error category is surfaced
    /// for diagnostics, the body is never logged.
    /// <paramref name="from"/> is the sending account (the composer's From dropdown), which may
    /// differ from <paramref name="account"/>: the core still resolves the original, and its
    /// <c>Re:</c> subject and <c>In-Reply-To</c>/<c>References</c> chain, in the account that
    /// holds it, so a cross-account reply still threads. <c>null</c> replies from the original's
    /// account.
    /// </summary>
    internal bool SubmitRichReply(
        string account,
        string key,
        Recipients recipients,
        string subject,
        string documentJson,
        ComposerFileAttachment[] files,
        string? from)
    {
        if (_app is null || BlockedByHarnessGate(recipients))
        {
            return false;
        }
        try
        {
            _app.SubmitRichReplyWithFiles(account, key, recipients, documentJson, files, from, subject);
            return true;
        }
        catch (Exception ex)
        {
            Log.Warn($"rich reply submit failed: {ex.GetType().Name}");
            return false;
        }
    }

    /// <summary>
    /// Validate and queue a rich forward of a message (by owning account + key) to the entered
    /// <paramref name="recipients"/> under <paramref name="subject"/>. Only the error category is
    /// surfaced for diagnostics, the body is never logged. <paramref name="from"/> is the sending
    /// account (the composer's From dropdown); <c>null</c> forwards from the original's account.
    /// </summary>
    internal bool SubmitRichForward(
        string account,
        string key,
        Recipients recipients,
        string subject,
        string documentJson,
        ComposerFileAttachment[] files,
        string? from)
    {
        if (_app is null || BlockedByHarnessGate(recipients))
        {
            return false;
        }
        try
        {
            _app.SubmitRichForwardWithFiles(account, key, recipients, documentJson, files, from, subject);
            return true;
        }
        catch (Exception ex)
        {
            Log.Warn($"rich forward submit failed: {ex.GetType().Name}");
            return false;
        }
    }

    /// <summary>
    /// The recipients to pre-fill a reply (<paramref name="replyAll"/> false) or reply-all
    /// composer for a message (by owning account + key): the core returns the suggested To
    /// and Cc (reply-all adds the other thread participants, minus the user). Returns
    /// <c>null</c> if the app isn't ready, so the composer opens with empty fields.
    /// </summary>
    internal RecipientSuggestion? ReplyRecipients(string account, string key, bool replyAll)
    {
        if (_app is null)
        {
            return null;
        }
        try
        {
            return _app.ReplyRecipients(account, key, replyAll);
        }
        catch (Exception ex)
        {
            Log.Warn($"reply recipients lookup failed: {ex.GetType().Name}");
            return null;
        }
    }

    // Refuses a send that would leave the local harness, in a DEBUG build connected to one. See
    // HarnessRecipientGate for why this is a gate rather than a rule: every reply to a fixture
    // opens with an external address already in To.
    private bool BlockedByHarnessGate(Recipients recipients)
    {
#if DEBUG
        if (!IsHarnessDevAccount)
        {
            return false;
        }
        var external = HarnessRecipientGate.ExternalRecipients(
            recipients.To, recipients.Cc, recipients.Bcc);
        if (external.Count == 0)
        {
            return false;
        }
        // The addresses themselves are fixture data, not the developer's mail, so naming them is
        // what makes the refusal actionable, you need to know WHICH one to clear.
        Log.Warn(
            $"send refused: connected to the local harness, but {external.Count} recipient(s) are "
            + $"outside {HarnessRecipientGate.LocalDomain}: {string.Join(", ", external)}. "
            + "Clear them, or use MAILCAL_DEV_ACCOUNT=personal to send for real.");
        return true;
#else
        _ = recipients;
        return false;
#endif
    }
}
