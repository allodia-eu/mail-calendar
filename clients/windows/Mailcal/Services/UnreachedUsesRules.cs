// A mailbox's calendar and contacts on a server its sign-in does not reach (docs/accounts.md rule
// 18): the question a switch to the provider asks first, and the form, the setup step after the
// connect and the account page's, drawn from the core's section. WinUI-free and L10n-free, so each
// rule is reachable from Mailcal.Tests: the words are the views', the decisions are here.

using uniffi.mailcal_bindings;

namespace Allodia.Mailcal.Services;

/// <summary>Which title a set of uses gets: what it names, calendar and contacts or one of
/// them.</summary>
internal enum UnreachedTitle
{
    Both,
    Calendar,
    Contacts,
}

/// <summary>How the person answered the question a sign-in that does not reach the calendar or
/// contacts asks before the browser opens.</summary>
internal enum UnreachedAnswer
{
    /// <summary>Keep them on the password, as an account of their own linked to this one.</summary>
    Keep,
    /// <summary>Stop using them on this account.</summary>
    Stop,
    /// <summary>Abandon the sign-in: the browser never opens and nothing changes.</summary>
    Cancel,
}

/// <summary>The form that gives a calendar or an address book a sign-in of its own, as the core
/// last drew it.</summary>
/// <param name="AccountId">The mailbox they move from.</param>
/// <param name="Section">The core's section: what moves, the server, the login, Connect, Skip and
/// the browser sign-in, and what the last of them came to.</param>
internal sealed record SeparateServerStep(string AccountId, AccountSeparateServer Section, UnreachedTitle Title)
{
    /// <summary>What moves: the calendar, the contacts or both.</summary>
    internal IReadOnlyList<AccountCapability> Uses => Section.Uses;

    /// <summary>The server they are on, as the core names it: a host, never a path.</summary>
    internal string Host => Section.Host;

    /// <summary>The login the form starts with.</summary>
    internal string Username => Section.Username;
}

internal static class UnreachedUsesRules
{
    /// <summary>The calendar and contacts among <paramref name="uses"/>, calendar first, each
    /// once: the only uses a server of their own can hold.</summary>
    internal static IReadOnlyList<AccountCapability> Dav(IEnumerable<AccountCapability>? uses)
    {
        var set = (uses ?? []).ToHashSet();
        return new[] { AccountCapability.Calendar, AccountCapability.Contacts }.Where(set.Contains).ToList();
    }

    /// <summary>The title for <paramref name="uses"/>; null when they name neither calendar nor
    /// contacts.</summary>
    internal static UnreachedTitle? Title(IEnumerable<AccountCapability>? uses) => Dav(uses) switch
    {
        { Count: 2 } => UnreachedTitle.Both,
        [AccountCapability.Calendar] => UnreachedTitle.Calendar,
        [AccountCapability.Contacts] => UnreachedTitle.Contacts,
        _ => null,
    };

    /// <summary>Whether a sign-in whose start named <paramref name="unreached"/> asks before the
    /// browser opens. One that names nothing behaves as it always has, and passes no answer.</summary>
    internal static bool Asks(IEnumerable<AccountCapability>? unreached) => Title(unreached) is not null;

    /// <summary>The answer the core is given; null for <see cref="UnreachedAnswer.Cancel"/>, which
    /// abandons the sign-in before anything reaches the core.</summary>
    internal static UnreachedUses? Choice(UnreachedAnswer answer) => answer switch
    {
        UnreachedAnswer.Keep => UnreachedUses.KeepOnPassword,
        UnreachedAnswer.Stop => UnreachedUses.StopUsing,
        _ => null,
    };

    /// <summary>The form for <paramref name="accountId"/>'s <paramref name="section"/>, the core's
    /// for a mailbox whose calendar or contacts need a sign-in of their own: the step setup shows
    /// after the account's connect, and what its page opens. Null while nothing needs one.</summary>
    internal static SeparateServerStep? Step(string accountId, AccountSeparateServer? section) =>
        section is not null && Title(section.Uses) is { } title ? new SeparateServerStep(accountId, section, title) : null;

    /// <summary>What a redraw compares: the section as drawn, so a read that changed nothing
    /// leaves the form, and what is typed in it, alone.</summary>
    internal static string Fingerprint(AccountSeparateServer? section) => section is null
        ? "-"
        : string.Join('/', string.Join(',', section.Uses), section.Host, section.Username, section.SignIn,
            $"{section.Connect.Enabled}{section.Connect.Running}", $"{section.StopUsing.Enabled}{section.StopUsing.Running}",
            section.Failed?.ToString() ?? "-", $"{section.SignInControl.Enabled}{section.SignInControl.Running}",
            BrowserSignInRules.Fingerprint(section.Browser));
}
