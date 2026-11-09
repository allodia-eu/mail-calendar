// A calendar or address book paused for a server found on another site (docs/account-autodetect.md
// rule 14): the question as the core draws it, and allowing the new server, a change on the
// account's page. "Not now" needs no call, the use stays paused.

using uniffi.mailcal_bindings;

namespace Allodia.Mailcal.Services;

public sealed partial class MailboxModel
{
    /// <summary>The question <paramref name="accountId"/> waits on, read again on every
    /// <see cref="AccountsChanged"/>; null when there is none, such as one allowed elsewhere
    /// meanwhile.</summary>
    internal AccountServerChange? ServerChangeQuestion(string accountId) =>
        _app?.AccountServerChange(accountId);

    /// <summary>Allows <paramref name="shown"/>, the change the question drew; the core moves the
    /// account onto it only once it connects, and the question says what it came to.</summary>
    internal void AllowServerChange(string accountId, ServerChange shown) =>
        ChangeAccount(new AccountPageIntent.AllowServerChange(accountId, shown));

    /// <summary>The address of <paramref name="accountId"/>, which the question names it by.</summary>
    internal string AccountAddress(string accountId) =>
        AccountsSnapshot().Accounts.FirstOrDefault(entry => entry.Id == accountId)?.Address ?? accountId;
}
