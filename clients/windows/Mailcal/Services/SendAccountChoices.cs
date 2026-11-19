// What Settings → Composing's default send account draws from the core's SendAccountSetting, and
// which signals leave it as drawn. WinUI-free and catalog-free so Mailcal.Tests reaches it.

using uniffi.mailcal_bindings;

namespace Allodia.Mailcal.Services;

internal static class SendAccountChoices
{
    /// <summary>What the row shows apart from which account is chosen, so a signal that moved only
    /// the default moves the picker in place.</summary>
    internal static string Fingerprint(SendAccountSetting? setting) => setting switch
    {
        SendAccountSetting.Choose choose =>
            "choose\n" + string.Join('\n', choose.Accounts.Select(account => $"{account.Id}\t{account.Email}")),
        SendAccountSetting.Only only => "only\n" + only.Email,
        _ => string.Empty,
    };

    /// <summary>Where the core's chosen account is among those it offers. The core always names
    /// one of them, so the first is only what a malformed answer falls to.</summary>
    internal static int SelectedIndex(SendAccountSetting.Choose choose) =>
        Math.Max(0, Array.FindIndex(choose.Accounts, account => account.Id == choose.Selected));
}
