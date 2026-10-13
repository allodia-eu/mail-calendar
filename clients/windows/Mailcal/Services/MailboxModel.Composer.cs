// Opening a composer and sending from it. Kept separate so MailboxModel.cs stays below the repo's
// 500-line file limit.

using uniffi.mailcal_bindings;

namespace Allodia.Mailcal.Services;

public sealed partial class MailboxModel
{
    /// <summary>
    /// Opens a composer on <paramref name="origin"/>: the one way every composer opens, a new
    /// message, a reply, a forward or a stored draft (docs/drafts.md). The core joins
    /// <paramref name="composition"/> to the message and answers with everything the composer opens
    /// holding; a forward's or a draft's files are staged into <paramref name="stagingDirectory"/>.
    /// Returns <c>null</c> when it cannot open, and the caller says so rather than opening an empty
    /// composer, which for a draft would save over it.
    /// </summary>
    /// <remarks>Blocks on the core and may fetch the original from the server, so call it off the
    /// UI thread.</remarks>
    internal ComposeOpening? OpenComposer(
        string composition,
        ComposeOrigin origin,
        ComposePrefill? prefill,
        string stagingDirectory)
    {
        if (_app is null)
        {
            return null;
        }
        try
        {
            return _app.OpenComposer(composition, origin, stagingDirectory, prefill);
        }
        catch (Exception ex)
        {
            Log.Warn($"composer: could not open ({ex.GetType().Name})");
            return null;
        }
    }

    /// <summary>
    /// Validate and queue what a composer holds, on the composition it opened on. The one send
    /// every composer makes: the core already knows what the message replies to or passes on, so a
    /// reply threads however the composer was reached, and an accepted send takes its stored draft
    /// out of Drafts. <paramref name="from"/> is the composer's From dropdown; <c>null</c> lets the
    /// core derive it. The body is never logged; only the error category is.
    /// </summary>
    internal bool SubmitComposition(
        string composition,
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
            _app.SubmitComposition(composition, recipients, subject, documentJson, files, from);
            return true;
        }
        catch (Exception ex)
        {
            Log.Warn($"composer submit failed: {ex.GetType().Name}");
            return false;
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
