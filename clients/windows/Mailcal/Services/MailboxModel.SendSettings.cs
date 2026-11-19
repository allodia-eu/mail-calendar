// The app-level default send account: which account a new message is written from when nothing
// else names one. The core decides it among the accounts used for mail and offers Settings →
// Composing's row as one record; it re-signals Surface.Settings after the setter and after an
// account change.

using uniffi.mailcal_bindings;

namespace Allodia.Mailcal.Services;

public sealed partial class MailboxModel
{
    /// <summary>
    /// What Settings → Composing offers for the default send account: no account, the one address,
    /// or a choice on the account the core resolves as the default. Read from the core's memory,
    /// so it is safe on the UI thread; <c>null</c> until a core exists.
    /// </summary>
    internal SendAccountSetting? SendAccountSetting => _app?.SendAccountSetting();

    /// <summary>
    /// Changes the app-level default send account (<c>null</c> clears it, restoring "the first
    /// account used for mail").
    /// </summary>
    public void SetDefaultSendAccount(string? accountId) =>
        DispatchPreference(new PreferenceIntent.SetDefaultSendAccount(accountId));
}
