// The words of what Settings → Accounts says beneath a use's switch, and of the line the calendar and
// contacts screens draw over a use that is not connected, which gives the same reason
// (docs/accounts.md rule 16). The decisions are AccountSettingsRules and UseOutageLines.

namespace Allodia.Mailcal.Services;

internal static class AccountUseText
{
    /// <summary>The line beneath a use's switch, or null where it has none.</summary>
    internal static string? Note(UseNote note) => note switch
    {
        UseNote.Last => L10n.SettingsAccountUseLast(),
        UseNote.NeedsContacts => L10n.SettingsAccountColleaguesNeedsContacts(),
        UseNote.NeedsMail => L10n.SettingsAccountSharedNeedsMail(),
        UseNote.Withheld => L10n.SettingsAccountUseWithheld(),
        UseNote.NotOffered => L10n.SettingsAccountUseNotOffered(),
        UseNote.UnreachableCalendar => L10n.SettingsAccountUseUnreachableCalendar(),
        UseNote.UnreachableContacts => L10n.SettingsAccountUseUnreachableContacts(),
        UseNote.RefusedCalendar => L10n.SettingsAccountUseRefusedCalendar(),
        UseNote.RefusedContacts => L10n.SettingsAccountUseRefusedContacts(),
        UseNote.OwnSignInCalendar => L10n.SettingsAccountUseOwnSignInCalendar(),
        UseNote.OwnSignInContacts => L10n.SettingsAccountUseOwnSignInContacts(),
        UseNote.ServerChangedCalendar => L10n.SettingsAccountUseServerChangedCalendar(),
        UseNote.ServerChangedContacts => L10n.SettingsAccountUseServerChangedContacts(),
        _ => null,
    };

    /// <summary>The words <see cref="UseOutageLines"/> composes a line from.</summary>
    internal static UseOutageWords OutageWords() => new(
        L10n.CalendarOutageTitle,
        L10n.ContactsOutageTitle,
        Note,
        L10n.UseOutageLastSynced(),
        L10n.UseOutageOpenAccount(),
        L10n.ServerChangedReview());
}
