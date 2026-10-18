// What a person reads an account by in Settings, where another account has the same address
// (docs/accounts.md rule 8). L10n-free, like AccountSettingsRules beside it: the words arrive as
// AccountNameWords, so which detail each distinction earns is tested in Mailcal.Tests.

using uniffi.mailcal_bindings;

namespace Allodia.Mailcal.Services;

/// <summary>The words an account's name is made of.</summary>
/// <param name="Kind">The kind's label, as the account row's kind line says it.</param>
/// <param name="Distinguished">"{address} ({detail})".</param>
internal sealed record AccountNameWords(Func<AccountKind, string> Kind, Func<string, string, string> Distinguished);

internal static class AccountNames
{
    /// <summary>A linked account as a link, a picker or a removal's confirmation names it: its
    /// address, with what tells it apart from another account of that address.</summary>
    internal static string Of(LinkedAccount account, AccountNameWords words) => account.Distinction switch
    {
        AccountDistinction.Kind kind => words.Distinguished(account.Address, words.Kind(kind.KindValue)),
        AccountDistinction.Server server => words.Distinguished(account.Address, server.Host),
        _ => account.Address,
    };

    /// <summary>An account's own kind line, on its row and its page. The kind is said already, so
    /// only a server's host is added.</summary>
    internal static string KindLine(AccountEntry entry, AccountNameWords words) =>
        entry.Distinction is AccountDistinction.Server server
            ? $"{words.Kind(entry.Kind)} · {server.Host}"
            : words.Kind(entry.Kind);
}
