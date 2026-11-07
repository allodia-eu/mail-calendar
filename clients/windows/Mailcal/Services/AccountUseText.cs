// The words of what Settings → Accounts says beneath a use's switch (docs/accounts.md rule 16). The
// decision is the core's (`account_page`); this only names it.

using uniffi.mailcal_bindings;

namespace Allodia.Mailcal.Services;

internal static class AccountUseText
{
    /// <summary>The line beneath a use's switch: what is wrong with the use, then why it cannot be
    /// pressed, so neither hides the other. Null where there is nothing to say.</summary>
    internal static string? Note(AccountUseSwitch use)
    {
        var status = use.Status switch
        {
            AccountUseStatus.Problem problem => Problem(use.Capability == AccountCapability.Calendar, problem.ProblemValue),
            AccountUseStatus.Withheld => L10n.SettingsAccountUseWithheld(),
            _ => null,
        };
        var blocked = use.Blocked switch
        {
            AccountUseBlock.LastUse => L10n.SettingsAccountUseLast(),
            AccountUseBlock.NeedsContacts => L10n.SettingsAccountColleaguesNeedsContacts(),
            AccountUseBlock.NeedsMail => L10n.SettingsAccountSharedNeedsMail(),
            AccountUseBlock.NotOffered => L10n.SettingsAccountUseNotOffered(),
            AccountUseBlock.NeedsServer => L10n.SettingsAccountUseNeedsEndpoint(),
            _ => null,
        };
        string[] lines = [.. new[] { status, blocked }.OfType<string>()];
        return lines.Length == 0 ? null : string.Join("\n", lines);
    }

    /// <summary>What a calendar or an address book that did not connect says. Only these fail on
    /// their own; mail decides whether the account connected at all.</summary>
    private static string Problem(bool calendar, UseProblem problem) => problem switch
    {
        UseProblem.Unreachable => calendar
            ? L10n.SettingsAccountUseUnreachableCalendar() : L10n.SettingsAccountUseUnreachableContacts(),
        UseProblem.SignInRefused => calendar
            ? L10n.SettingsAccountUseRefusedCalendar() : L10n.SettingsAccountUseRefusedContacts(),
        UseProblem.ServerChanged => calendar
            ? L10n.SettingsAccountUseServerChangedCalendar() : L10n.SettingsAccountUseServerChangedContacts(),
        UseProblem.NoneOnServer => calendar
            ? L10n.SettingsAccountUseNoneOnServerCalendar() : L10n.SettingsAccountUseNoneOnServerContacts(),
        _ => calendar ? L10n.SettingsAccountUseOwnSignInCalendar() : L10n.SettingsAccountUseOwnSignInContacts(),
    };
}
