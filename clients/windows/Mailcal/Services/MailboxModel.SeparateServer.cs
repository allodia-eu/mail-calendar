// A calendar and contacts the account's sign-in does not reach, given a sign-in of their own
// (docs/accounts.md rule 18): the setup step after the mail sign-in, and the form on the account's
// page. The core adds them as an account of their own, linked to the mailbox, once their server
// has accepted the password; any refusal leaves both accounts as they were.

using uniffi.mailcal_bindings;

namespace Allodia.Mailcal.Services;

public sealed partial class MailboxModel
{
    /// <summary>Moves <paramref name="uses"/> of <paramref name="accountId"/> to an account of
    /// their own at the same servers, signing in as <paramref name="username"/>.</summary>
    /// <returns>Null when moved, otherwise why not, to say under the form's fields.</returns>
    internal async Task<string?> AddLinkedCalendarAccountAsync(
        string accountId, IReadOnlyList<AccountCapability> uses, string username, string password)
    {
        if (_app is not { } app)
        {
            return L10n.AppUnavailable();
        }
        try
        {
            // Two connects: the new account's, then the mailbox's without what moved.
            var moved = await Task.Run(() => app.AddLinkedCalendarAccount(accountId, uses.ToArray(), username, password));
            Log.Info($"an account's calendar and contacts moved to {LogHandle(moved)}, linked to {LogHandle(accountId)}");
            SyncAfterAccountChange();
            return null;
        }
        catch (Exception ex)
        {
            Log.Error($"giving a calendar and contacts a sign-in of their own failed: {CoreError.Describe(ex)}");
            return CoreError.Said(ex, L10n.SigninRefused(), L10n.AccountAlreadySetUp(), L10n.SigninDifferentAccount());
        }
    }

    /// <summary>Connect on the setup's separate-server step: on to the link step once the server
    /// has taken the password.</summary>
    /// <returns>Null when connected, otherwise why not; the step stays.</returns>
    internal async Task<string?> SetupSeparateServerConnectAsync(string username, string password)
    {
        if (_setupFlow.SeparateServer is not { } step || IsSubmitting)
        {
            return null;
        }
        IsSubmitting = true;
        var error = await AddLinkedCalendarAccountAsync(step.AccountId, step.Uses, username, password);
        IsSubmitting = false;
        if (error is null)
        {
            SeparateServerDone();
        }
        return error;
    }

    /// <summary>Skip on the setup's separate-server step: the new account stops being used for
    /// what its sign-in does not reach, so it does not open on a calendar that cannot connect.</summary>
    internal async Task SetupSeparateServerSkipAsync()
    {
        if (_setupFlow.SeparateServer is not { } step || IsSubmitting)
        {
            return;
        }
        IsSubmitting = true;
        foreach (var use in step.Uses)
        {
            // A refusal is in the log already; the account's page still offers the use.
            await SetAccountUseAsync(step.AccountId, use, false);
        }
        IsSubmitting = false;
        SeparateServerDone();
    }
}
