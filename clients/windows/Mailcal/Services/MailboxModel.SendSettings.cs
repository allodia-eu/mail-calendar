// The send-account surface of the model: the app-level default send account (which account new
// mail in the combined inbox goes out from) and the rule that resolves the composer's From
// dropdown against the accounts that actually exist. Split into its own partial to keep
// MailboxModel.cs under the 500-line limit. State lives in Rust (persisted); the core re-signals
// Surface.Settings after the setter.

using System.Linq;
using Allodia.Mailcal.ViewModels;

namespace Allodia.Mailcal.Services;

public sealed partial class MailboxModel
{
    /// <summary>
    /// The persisted default send account's id, or <c>null</c> when the user hasn't chosen one.
    /// Read fresh from the core (which owns the value), so the settings dialog and the composer
    /// reflect the latest choice. This is the <em>stored</em> id, which may name an account the
    /// user has since removed, resolve it through <see cref="SendAccount"/>.
    /// </summary>
    public string? DefaultSendAccount => _app?.DefaultSendAccount();

    /// <summary>
    /// Sets and persists the app-level default send account (<c>null</c> clears it, restoring
    /// "the first configured account").
    /// </summary>
    public void SetDefaultSendAccount(string? accountId) => _app?.SetDefaultSendAccount(accountId);

    /// <summary>
    /// The account the composer's From dropdown opens on, resolved against the configured
    /// accounts. <paramref name="preferred"/> is the context's own choice, the account that
    /// received the mail being replied to/forwarded, or the selected mailbox's account for a new
    /// message. Falling back: the app-level default send account, then the first configured one.
    /// A stored default naming a removed account therefore degrades to the first rather than to
    /// nothing, which is the core's own resolution order.
    /// </summary>
    public AccountItem? SendAccount(string? preferred)
    {
        foreach (var candidate in new[] { preferred, DefaultSendAccount })
        {
            if (candidate is null)
            {
                continue;
            }
            if (Accounts.FirstOrDefault(account => account.Id == candidate) is { } match)
            {
                return match;
            }
        }
        return Accounts.FirstOrDefault();
    }
}
