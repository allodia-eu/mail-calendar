// What a person reads an account by in Settings and in a removal's confirmation, where another
// account has the same address (docs/accounts.md rule 8). L10n-free, like AccountSettingsRules beside it: the words arrive as
// AccountNameWords, so which detail each distinction earns is tested in Mailcal.Tests.

using uniffi.mailcal_bindings;

namespace Allodia.Mailcal.Services;

/// <summary>The words an account's name is made of.</summary>
/// <param name="Kind">The kind's label, as the account row's kind line says it.</param>
/// <param name="Distinguished">"{address} ({detail})".</param>
internal sealed record AccountNameWords(Func<AccountKind, string> Kind, Func<string, string, string> Distinguished);

internal static class AccountNames
{
    /// <summary>A linked account as a link, a picker or a removal's confirmation names it.</summary>
    internal static string Of(LinkedAccount account, AccountNameWords words) =>
        Of(account.Address, account.Distinction, words);

    /// <summary>An account a message acts on, such as the one a removal removes.</summary>
    internal static string Of(AccountEntry entry, AccountNameWords words) => Of(entry.Address, entry.Distinction, words);

    /// <summary>The account <paramref name="id"/> as <see cref="Of(AccountEntry, AccountNameWords)"/>
    /// names it, or <paramref name="address"/> when the snapshot no longer lists it.</summary>
    internal static string Of(IEnumerable<AccountEntry> accounts, string id, string address, AccountNameWords words) =>
        accounts.FirstOrDefault(entry => entry.Id == id) is { } found ? Of(found, words) : address;

    /// <summary><paramref name="address"/>, with what tells it apart from another account of that
    /// address.</summary>
    internal static string Of(string address, AccountDistinction? distinction, AccountNameWords words) => distinction switch
    {
        AccountDistinction.Kind kind => words.Distinguished(address, words.Kind(kind.KindValue)),
        AccountDistinction.Server server => words.Distinguished(address, server.Host),
        _ => address,
    };

    /// <summary>An account's own kind line, on its row and its page. The kind is said already, so
    /// only a server's host is added.</summary>
    internal static string KindLine(AccountEntry entry, AccountNameWords words) =>
        entry.Distinction is AccountDistinction.Server server
            ? $"{words.Kind(entry.Kind)} · {server.Host}"
            : words.Kind(entry.Kind);
}
