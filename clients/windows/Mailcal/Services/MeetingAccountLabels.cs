// What Settings → Videoconferencing says of each account that makes video meetings
// (docs/settings.md category 4): the state of its Video meetings use, and the name a screen reader
// reads for its row. WinUI-free and catalog-free so Mailcal.Tests reaches it; the dialog hands in
// the catalog's words.

using uniffi.mailcal_bindings;

namespace Allodia.Mailcal.Services;

/// <summary>The catalog's words for each state of the use.</summary>
internal sealed record MeetingStateWords(string On, string Off, string NeedsPermission, string NotOffered);

internal static class MeetingAccountLabels
{
    /// <summary>The account's Video meetings use, as its row says it.</summary>
    internal static string State(CapabilityState state, MeetingStateWords words) => state switch
    {
        CapabilityState.On => words.On,
        CapabilityState.NeedsPermission => words.NeedsPermission,
        CapabilityState.NotOffered => words.NotOffered,
        _ => words.Off,
    };

    /// <summary>What a screen reader reads for a row: the service, the account and the state, in
    /// the order the row draws them.</summary>
    internal static string Spoken(string service, string account, string state) => $"{service}, {account}, {state}";

    /// <summary>What the drawn group shows, so a signal that changed none of it leaves it
    /// alone.</summary>
    internal static string Fingerprint(IEnumerable<MeetingAccount> rows, Func<LinkedAccount, string> name) =>
        string.Join('\n', rows.Select(row => $"{row.Account.Id}|{name(row.Account)}|{row.Service}|{row.State}"));
}
