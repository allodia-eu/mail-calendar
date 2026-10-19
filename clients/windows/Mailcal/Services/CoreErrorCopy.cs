// The catalog's half of CoreError: the sentences it chooses from, in the person's language. Apart
// from CoreError because L10n cannot be linked into Mailcal.Tests, which tests the choosing.

namespace Allodia.Mailcal.Services;

internal static class CoreErrorCopy
{
    /// <summary>The words <see cref="CoreError"/> says a core error with.</summary>
    internal static CoreErrorWords Words() => new(
        L10n.SigninRefused(),
        L10n.AccountAlreadySetUp(),
        L10n.SigninDifferentAccount(),
        L10n.SigninStale(),
        L10n.SeparateServerUnreachableCalendar(),
        L10n.SeparateServerUnreachableContacts(),
        L10n.SettingsAccountUseOwnSignInCalendar(),
        L10n.SettingsAccountUseOwnSignInContacts(),
        L10n.StatusConnectFailed);

    /// <summary><see cref="CoreError.Said(Exception, CoreErrorWords)"/> in the catalog's words.</summary>
    internal static string Said(Exception error) => CoreError.Said(error, Words());

    /// <summary><see cref="CoreError.SaidOnBanner"/> in the catalog's words.</summary>
    internal static string SaidOnBanner(Exception? error) =>
        CoreError.SaidOnBanner(error, Words(), L10n.SigninExpiredFailed());

    /// <summary><see cref="CoreError.SaidAsConnect"/> in the catalog's words.</summary>
    internal static string SaidAsConnect(Exception error) => CoreError.SaidAsConnect(error, Words());
}
