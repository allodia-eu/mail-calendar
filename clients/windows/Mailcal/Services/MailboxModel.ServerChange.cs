// A calendar or address book paused for a server found on another site (docs/account-autodetect.md
// rule 14): what the question names, and allowing the new server. "Not now" needs no call, the use
// stays paused.

using uniffi.mailcal_bindings;

namespace Allodia.Mailcal.Services;

public sealed partial class MailboxModel
{
    /// <summary>The change waiting on the person for <paramref name="accountId"/>; null when there
    /// is none, such as one allowed elsewhere meanwhile.</summary>
    internal async Task<ServerChange?> PendingServerChangeAsync(string accountId)
    {
        if (_app is not { } app)
        {
            return null;
        }
        try
        {
            return await Task.Run(() => app.PendingServerChange(accountId));
        }
        catch (Exception ex)
        {
            Log.Error($"reading a server change failed: {CoreError.Describe(ex)}");
            return null;
        }
    }

    /// <summary>Allows the new server if it is still <paramref name="shown"/>, which the core
    /// connects before it stores anything.</summary>
    /// <returns><see cref="ServerChangeAnswer.Settled"/> once nothing waits any more, the account
    /// having moved now or by an answer given elsewhere; <see cref="ServerChangeAnswer.Replaced"/>
    /// with the question to ask instead when a lookup replaced it while it was shown; null when it
    /// failed and nothing changed.</returns>
    internal async Task<ServerChangeAnswer?> AcceptServerChangeAsync(string accountId, ServerChange shown)
    {
        if (_app is not { } app)
        {
            return null;
        }
        try
        {
            // Blocking: it dials the new server.
            return await Task.Run(() => app.AcceptServerChange(accountId, shown));
        }
        catch (Exception ex)
        {
            Log.Error($"allowing a new server failed: {CoreError.Describe(ex)}");
            return null;
        }
    }

    /// <summary>The address of <paramref name="accountId"/>, which the question names it by.</summary>
    internal string AccountAddress(string accountId) =>
        AccountsSnapshot().Accounts.FirstOrDefault(entry => entry.Id == accountId)?.Address ?? accountId;
}
