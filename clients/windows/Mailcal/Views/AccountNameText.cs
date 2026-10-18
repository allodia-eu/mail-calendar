// The words AccountNames composes an account's name from, for Settings and the setup link step.
// The choices are AccountNames' and unit-tested; this maps them to L10n, which the test-linked half
// cannot reach.

using Allodia.Mailcal.Services;
using uniffi.mailcal_bindings;

namespace Allodia.Mailcal.Views;

internal static class AccountNameText
{
    internal static readonly AccountNameWords Words = new(KindLabel, L10n.SettingsAccountDistinguished);

    /// <summary>A linked account, with what tells it apart from another of its address.</summary>
    internal static string Of(LinkedAccount account) => AccountNames.Of(account, Words);

    /// <summary>An account's own kind line, with its server's host where only that tells it apart.</summary>
    internal static string KindLine(AccountEntry entry) => AccountNames.KindLine(entry, Words);

    /// <summary>The label for an account's kind, as the setup form names the same choice.</summary>
    internal static string KindLabel(AccountKind kind) => kind switch
    {
        AccountKind.Dav => L10n.AccountKindDav(),
        AccountKind.Jmap => L10n.SetupAccountTypeJmap(),
        AccountKind.Microsoft => L10n.SetupAccountTypeMicrosoft(),
        AccountKind.Google => L10n.SetupAccountTypeGoogle(),
        _ => L10n.SetupAccountTypePassword(),
    };
}
