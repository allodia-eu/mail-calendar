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
        var blocked = use.Blocked is { } block ? BlockNote(block) : null;
        string[] lines = [.. new[] { status, blocked }.OfType<string>()];
        return lines.Length == 0 ? null : string.Join("\n", lines);
    }

    /// <summary>Why a use's switch cannot be pressed.</summary>
    internal static string BlockNote(AccountUseBlock block) => block switch
    {
        AccountUseBlock.LastUse => L10n.SettingsAccountUseLast(),
        AccountUseBlock.NeedsContacts => L10n.SettingsAccountColleaguesNeedsContacts(),
        AccountUseBlock.NeedsMail => L10n.SettingsAccountSharedNeedsMail(),
        AccountUseBlock.NotOffered => L10n.SettingsAccountUseNotOffered(),
        _ => L10n.SettingsAccountUseNeedsEndpoint(),
    };

    /// <summary>Why a change on an account's page did not take: a refused switch as the switch
    /// says it beside itself, and the rest as <see cref="CoreError.Refused"/> words them.</summary>
    internal static string Refused(AccountChangeRefusal refusal) => refusal switch
    {
        AccountChangeRefusal.Blocked blocked => BlockNote(blocked.Block),
        AccountChangeRefusal.Withheld => L10n.SettingsAccountUseWithheld(),
        _ => CoreError.Refused(refusal, CoreErrorCopy.Words()),
    };

    /// <summary>What Save says the server fields leave out, naming the field as the form labels
    /// it.</summary>
    internal static string EndpointsGap(AccountEndpointsGap gap) => L10n.SettingsAccountServersGap(gap switch
    {
        AccountEndpointsGap.Imap => L10n.SettingsAccountFieldImap(),
        AccountEndpointsGap.Smtp => L10n.SetupFieldSmtp(),
        AccountEndpointsGap.CalDav => L10n.SetupFieldCaldav(),
        AccountEndpointsGap.CardDav => L10n.SettingsAccountFieldCarddav(),
        AccountEndpointsGap.Username => L10n.SettingsAccountFieldLogin(),
        _ => L10n.SettingsAccountFieldNewPassword(),
    });

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
