// What Settings → Calendar's default calendar and Settings → Videoconferencing draw from the
// core's records, and which signals leave them as drawn. WinUI-free and catalog-free so
// Mailcal.Tests reaches it.

using uniffi.mailcal_bindings;

namespace Allodia.Mailcal.Services;

internal static class SettingsCalendarPages
{
    /// <summary>Whether the Jitsi field says its address was refused: while it holds exactly the
    /// text the core last refused, so typing takes the line away.</summary>
    internal static bool JitsiRefused(string text, VideoconferencingPage? page) =>
        page?.JitsiRefused is { } refused && text == refused;

    /// <summary>What the default-calendar group shows apart from which row is checked, so a signal
    /// that moved only the default moves the check in place.</summary>
    internal static string DefaultCalendarsFingerprint(NewEventCalendars? offered) =>
        offered is null
            ? string.Empty
            : $"{offered.Named}\n" + string.Join('\n', offered.Groups.Select(group =>
                $"{group.Account}|{group.Address}\t"
                + string.Join('\t', group.Calendars.Select(row => $"{row.Id}|{row.Name}"))));

    /// <summary>What the default-meeting group shows apart from each account's choice, so a signal
    /// that moved only a choice moves its picker in place.</summary>
    internal static string MeetingDefaultsFingerprint(IEnumerable<MeetingDefault> rows) =>
        string.Join('\n', rows.Select(row =>
            $"{row.Account}\t{row.AccountAddress}\t"
            + string.Join('\t', row.Choices.Select(choice => $"{choice.Id}|{choice.Name}|{choice.Through}"))));
}
