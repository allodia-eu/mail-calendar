// What an account's page draws of its links (docs/accounts.md rule 12): a use a link fills, as a
// row that opens the account it comes from, and the accounts taking a use from this one, each with
// Stop linking, asked first. WinUI-free and L10n-free, so each rule is reachable from
// Mailcal.Tests: the words arrive as parameters, the decisions are the core's and here.

using uniffi.mailcal_bindings;

namespace Allodia.Mailcal.Services;

/// <summary>One sentence in three wordings, as it names the calendar, the address book or
/// both.</summary>
internal sealed record ByUses(string Calendar, string Contacts, string Both)
{
    /// <summary>The wording for <paramref name="uses"/>; the calendar's when they name
    /// neither.</summary>
    internal string For(IEnumerable<AccountCapability> uses) => UnreachedUsesRules.Title(uses) switch
    {
        UnreachedTitle.Both => Both,
        UnreachedTitle.Contacts => Contacts,
        _ => Calendar,
    };
}

/// <summary>The words the Linked to group and Stop linking's question are made of.</summary>
/// <param name="Line">Beneath an account taking a use: what it takes.</param>
/// <param name="Stop">What stopping takes away from it.</param>
/// <param name="LeftWithout">Added when it has no server of its own for some of that.</param>
/// <param name="Title">The question's title, for the account it names.</param>
internal sealed record LinkedFromWords(
    AccountNameWords Names, ByUses Line, ByUses Stop, string LeftWithout, Func<string, string> Title);

/// <summary>A use a link fills: a row that opens the account it comes from, never a
/// switch.</summary>
/// <param name="Subtitle">The account it comes from, in "From {account}".</param>
/// <param name="Opens">The account whose page the row opens.</param>
/// <param name="Enabled">False while a change runs on the page.</param>
internal sealed record LinkedUseRow(string Subtitle, string Opens, bool Enabled);

/// <summary>An account taking a use from this one, as the Linked to group draws it.</summary>
/// <param name="Asking">What the page remembers Stop linking's open question by, across a
/// redraw.</param>
internal sealed record LinkedFromRow(
    string Title, string Line, string ConfirmTitle, string ConfirmMessage, ControlState Stop, string Asking)
{
    /// <summary>The question's Stop linking, held while another change runs; Cancel always
    /// can.</summary>
    internal bool ConfirmEnabled => Stop.Enabled;
}

internal static class AccountLinkedRules
{
    /// <summary>The row for a use a link fills; null for a use drawn as a switch.</summary>
    internal static LinkedUseRow? Row(AccountUseSwitch use, AccountNameWords names, Func<string, string> from) =>
        use.Linked is { } linked
            ? new LinkedUseRow(from(AccountNames.Of(linked, names)), linked.Id, use.Control.Enabled)
            : null;

    /// <summary>The row for <paramref name="taking"/> on <paramref name="account"/>'s page.</summary>
    internal static LinkedFromRow Row(string account, AccountLinkedFrom taking, LinkedFromWords words)
    {
        var name = AccountNames.Of(taking.Account, words.Names);
        return new LinkedFromRow(
            name,
            words.Line.For(taking.Uses),
            words.Title(name),
            StopMessage(taking, words),
            taking.Stop,
            $"stop-linking:{account}:{taking.Account.Id}");
    }

    /// <summary>What stopping the links with <paramref name="taking"/> does, said before it is
    /// confirmed: what it loses, and that it then has none where it has no server of its
    /// own.</summary>
    internal static string StopMessage(AccountLinkedFrom taking, LinkedFromWords words)
    {
        var message = words.Stop.For(taking.Uses);
        return taking.LeavesWithout.Length == 0 ? message : $"{message} {words.LeftWithout}";
    }

    /// <summary>What the question's answer sends: every link between <paramref name="account"/>
    /// and the account taking a use from it stops on yes, and nothing on Cancel.</summary>
    internal static AccountPageIntent? StopAnswer(bool yes, string account, AccountLinkedFrom taking) =>
        yes ? new AccountPageIntent.StopLinking(account, taking.Account.Id) : null;
}
