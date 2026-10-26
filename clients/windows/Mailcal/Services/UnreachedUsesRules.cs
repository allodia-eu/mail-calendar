// A mailbox's calendar and contacts on a server its sign-in does not reach (docs/accounts.md rule
// 18): the question a switch to the provider asks first, the setup step after the connect, and
// the account page's offer of a sign-in of their own. WinUI-free and L10n-free, so each rule is
// reachable from Mailcal.Tests: the words are the views', the decisions are here.

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

/// <summary>The form that gives a calendar or an address book a sign-in of its own.</summary>
/// <param name="AccountId">The mailbox they move from.</param>
/// <param name="Uses">What moves: the calendar, the contacts or both.</param>
/// <param name="Host">The server they are on, as the core names it: a host, never a path.</param>
/// <param name="Username">The login the form starts with.</param>
internal sealed record SeparateServerStep(
    string AccountId, IReadOnlyList<AccountCapability> Uses, UnreachedTitle Title, string Host, string Username);

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

    /// <summary>
    /// The form for <paramref name="entry"/>: what its page offers
    /// <c>settings_account_use_separate_sign_in</c> for, and the step setup shows after the
    /// account's connect. Exactly when the core names a server its calendar or contacts need a
    /// sign-in of their own at (<see cref="AccountEntry.SeparateServer"/>), which holds for an
    /// account that signs in through its provider too, whose servers its page does not show.
    /// </summary>
    internal static SeparateServerStep? Step(AccountEntry entry)
    {
        if (entry.SeparateServer is not { } server)
        {
            return null;
        }
        var moving = Dav(server.Uses);
        if (Title(moving) is not { } title)
        {
            return null;
        }
        var username = string.IsNullOrWhiteSpace(server.Username) ? entry.Address : server.Username;
        return new SeparateServerStep(entry.Id, moving, title, server.Host, username);
    }

    /// <summary>The step setup shows once <paramref name="accountId"/> has connected, before the
    /// link step; null, and no step, while every use it is used for connects.</summary>
    internal static SeparateServerStep? SetupStep(AccountsSnapshot snapshot, string accountId) =>
        snapshot.Accounts.FirstOrDefault(candidate => candidate.Id == accountId) is { } entry ? Step(entry) : null;

    /// <summary>Whether a use's row on the account's page carries the action: one of the uses
    /// <paramref name="step"/> moves.</summary>
    internal static bool Offers(SeparateServerStep? step, AccountCapability capability) =>
        step is not null && step.Uses.Contains(capability);
}
