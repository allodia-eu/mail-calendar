// What the calendar and contacts screens say over a use that did not connect (docs/accounts.md rule
// 16, docs/calendar.md §4, docs/contacts.md §3). WinUI-free and L10n-free, like AccountSettingsRules
// beside it: the words arrive as UseOutageWords, so the order of the three sentences and which reason
// each problem earns are tested in Mailcal.Tests.

using Allodia.Mailcal.ViewModels;
using uniffi.mailcal_bindings;

namespace Allodia.Mailcal.Services;

/// <summary>The words an outage line is made of.</summary>
/// <param name="CalendarTitle">"The calendar for {address} isn't connected."</param>
/// <param name="ContactsTitle">"The address book for {address} isn't connected."</param>
/// <param name="Reason">What the account's page says beneath the use's switch.</param>
/// <param name="LastSynced">"You're seeing what was last synced."</param>
/// <param name="OpenAccount">"Account settings", the button that opens the account's page.</param>
/// <param name="Review">"Review", the button that opens the question a server change asks.</param>
internal sealed record UseOutageWords(
    Func<string, string> CalendarTitle,
    Func<string, string> ContactsTitle,
    Func<UseNote, string?> Reason,
    string LastSynced,
    string OpenAccount,
    string Review);

internal static class UseOutageLines
{
    /// <summary>
    /// One line per outage, in the snapshot's order. The core has already left out what the
    /// offline banner and an expired sign-in's prompt cover, so nothing is filtered here.
    /// </summary>
    /// <param name="use">Calendar or contacts: which title, and which of the page's reasons.</param>
    /// <param name="addressOf">The account's address by id; the id is shown only where no account
    /// carries it.</param>
    internal static IReadOnlyList<UseOutageLine> Of(
        IEnumerable<UseOutage> outages,
        AccountCapability use,
        Func<string, string?> addressOf,
        UseOutageWords words) =>
        outages
            .Select(outage =>
            {
                var reviews = AccountSettingsRules.ReviewsServerChange(AccountSettingsRules.ProblemNote(use, outage.Problem));
                return new UseOutageLine(
                    outage.AccountId,
                    Text(outage, use, addressOf(outage.AccountId) ?? outage.AccountId, words),
                    reviews ? words.Review : words.OpenAccount,
                    reviews);
            })
            .ToList();

    /// <summary>The title, the reason and the last-synced sentence, joined with a space.</summary>
    internal static string Text(UseOutage outage, AccountCapability use, string address, UseOutageWords words)
    {
        var title = use == AccountCapability.Calendar ? words.CalendarTitle(address) : words.ContactsTitle(address);
        var reason = words.Reason(AccountSettingsRules.ProblemNote(use, outage.Problem));
        return string.Join(' ', new[] { title, reason, words.LastSynced }.Where(part => !string.IsNullOrEmpty(part)));
    }

    /// <summary>Whether two sets of lines draw the same, so an unchanged signal leaves the bars alone.</summary>
    internal static bool Same(IReadOnlyList<UseOutageLine> drawn, IReadOnlyList<UseOutageLine> next) =>
        drawn.Select(line => (line.AccountId, line.Text, line.Action))
            .SequenceEqual(next.Select(line => (line.AccountId, line.Text, line.Action)));
}
