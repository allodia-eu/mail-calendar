// The shared mailboxes an account's page offers (docs/shared-mailboxes.md rules 3, 7 and 8). Asking
// the server and checking a typed address are requests to it, and switching one off deletes what
// the device holds of it, so every call runs off the UI thread.

using uniffi.mailcal_bindings;

namespace Allodia.Mailcal.Services;

/// <summary>How checking a typed address ended: the mailbox, ready to switch on; neither, when
/// nothing the account opens answers to it; or why the server could not be asked.</summary>
internal sealed record SharedLookup(SharedMailboxEntry? Mailbox, string? Error);

public sealed partial class MailboxModel
{
    /// <summary>Which mailboxes the account's sign-in opens besides its own, as its server
    /// answered; null when the account is not set up or the core failed.</summary>
    internal async Task<SharedMailboxesView?> SharedMailboxesAsync(string accountId)
    {
        if (_app is not { } app)
        {
            return null;
        }
        try
        {
            return await Task.Run(() => app.SharedMailboxes(accountId));
        }
        catch (Exception ex)
        {
            Log.Warn($"asking for an account's shared mailboxes failed: {CoreError.Describe(ex)}");
            return null;
        }
    }

    /// <summary>The mailbox at <paramref name="address"/>, when the account's sign-in opens it.</summary>
    internal async Task<SharedLookup> ResolveSharedMailboxAsync(string accountId, string address)
    {
        if (_app is not { } app)
        {
            return new SharedLookup(null, L10n.AppUnavailable());
        }
        try
        {
            return new SharedLookup(await Task.Run(() => app.ResolveSharedMailbox(accountId, address)), null);
        }
        catch (Exception ex)
        {
            Log.Warn($"looking up a shared mailbox failed: {CoreError.Describe(ex)}");
            return new SharedLookup(null, CoreErrorCopy.Said(ex));
        }
    }

    /// <summary>Uses the account for <paramref name="mailbox"/>, or stops.</summary>
    /// <returns>Null when stored, otherwise why not.</returns>
    internal async Task<string?> SetSharedMailboxAsync(string accountId, SharedMailboxEntry mailbox, bool on)
    {
        if (_app is not { } app)
        {
            return L10n.AppUnavailable();
        }
        try
        {
            // Stored through the Credential Manager, and switched off, the mailbox's mail is deleted.
            await Task.Run(() => app.SetSharedMailbox(accountId, mailbox, on));
            return null;
        }
        catch (Exception ex)
        {
            Log.Error($"switching a shared mailbox failed: {CoreError.Describe(ex)}");
            return CoreErrorCopy.Said(ex);
        }
    }
}
