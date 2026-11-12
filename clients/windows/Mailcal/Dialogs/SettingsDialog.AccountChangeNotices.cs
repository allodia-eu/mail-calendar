// What a change on an account's page came to, as the page says it once (docs/waiting.md): a
// refusal in the core's words for why, a removal whose stored sign-in stayed behind, a browser
// sign-in that did not complete or that the provider answered with less than was asked for, and a
// separate sign-in that moved the calendar or contacts to an account of their own. Any other change
// that worked says nothing.

using Allodia.Mailcal.Services;
using Microsoft.UI.Xaml.Controls;
using uniffi.mailcal_bindings;

namespace Allodia.Mailcal.Dialogs;

public sealed partial class SettingsDialog
{
    /// <summary>A change the page could not make, and why.</summary>
    private static AccountNotice ChangeFailed(string detail) =>
        new(InfoBarSeverity.Error, L10n.SettingsAccountChangeFailedTitle(), detail);

    /// <summary>What a change on the page came to, in the core's words for why. A sign-in that
    /// worked says nothing, unless the provider still withholds a use or the uses moved to an
    /// account of their own.</summary>
    private static AccountNotice ChangeFailed(AccountChangeNotice notice) => notice switch
    {
        AccountChangeNotice.UseFailed use => ChangeFailed(AccountUseText.Refused(use.Refusal)),
        AccountChangeNotice.LinkFailed link => ChangeFailed(AccountUseText.Refused(link.Refusal)),
        AccountChangeNotice.RemoveFailed removal => RemovalFailed(L10n.RemoveAccountFailed(removal.Detail)),
        AccountChangeNotice.ServersFailed servers => new(
            InfoBarSeverity.Error, L10n.SettingsAccountServersFailed(), AccountUseText.Refused(servers.Refusal)),
        AccountChangeNotice.SharedStoreFailed shared => ChangeFailed(AccountUseText.Refused(shared.Refusal)),
        AccountChangeNotice.PasswordFailed password => ChangeFailed(AccountUseText.Refused(password.Refusal)),
        AccountChangeNotice.StopLinkingFailed stop => ChangeFailed(AccountUseText.Refused(stop.Refusal)),
        AccountChangeNotice.SignInFailed signIn => new(
            InfoBarSeverity.Error,
            signIn.Switching ? L10n.SettingsAccountSwitchFailed() : L10n.SettingsAccountSigninFailed(),
            AccountUseText.Refused(signIn.Refusal)),
        // The sign-in worked; what the provider still withholds is the person's to allow.
        AccountChangeNotice.SignedInWithheld withheld => new(
            InfoBarSeverity.Warning,
            L10n.SetupWithheldTitle(),
            L10n.SetupWithheldDetail(string.Join(", ", withheld.Uses.Select(UseName)))),
        // An explanation, not a failure: too long to read in passing, so it stands at the top of
        // the page until it is closed or the page's next change starts.
        AccountChangeNotice.SignedInSeparately separately => new(
            InfoBarSeverity.Informational,
            L10n.SettingsAccountSignedInSeparatelyTitle(),
            new ByUses(
                L10n.SettingsAccountSignedInSeparatelyCalendar(),
                L10n.SettingsAccountSignedInSeparatelyContacts(),
                L10n.SettingsAccountSignedInSeparatelyBoth()).For(separately.Uses)),
        // Not a failure of the app: the address is the person's to check.
        AccountChangeNotice.SharedStoreNotFound => new(
            InfoBarSeverity.Warning, L10n.SettingsAccountSharedNotFound(), string.Empty),
        _ => ChangeFailed(L10n.ErrorUnknown()),
    };

    /// <summary>A removal whose stored sign-in could not be erased, so it comes back at the next
    /// launch: said rather than left to surprise.</summary>
    private static AccountNotice RemovalFailed(string message) => new(InfoBarSeverity.Error, string.Empty, message);
}
