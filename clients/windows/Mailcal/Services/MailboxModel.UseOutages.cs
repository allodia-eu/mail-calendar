// The calendars and address books that did not connect on an account that did, from the connectivity
// snapshot (docs/accounts.md rule 16): one line each above the calendar week and above the contacts
// list. The composition is UseOutageLines, tested in Mailcal.Tests.

using System.Collections.ObjectModel;
using Allodia.Mailcal.ViewModels;
using uniffi.mailcal_bindings;

namespace Allodia.Mailcal.Services;

public sealed partial class MailboxModel
{
    /// <summary>One line per account whose calendar is not connected, in the snapshot's order.</summary>
    public ObservableCollection<UseOutageLine> CalendarOutageLines { get; } = new();

    /// <summary>One line per account whose address book is not connected, in the snapshot's order.</summary>
    public ObservableCollection<UseOutageLine> ContactsOutageLines { get; } = new();

    private void UpdateUseOutages(ConnectivitySnapshot connectivity)
    {
        if (connectivity.CalendarOutages.Length == 0 && connectivity.ContactsOutages.Length == 0)
        {
            ReplaceOutageLines(CalendarOutageLines, []);
            ReplaceOutageLines(ContactsOutageLines, []);
            return;
        }
        // Every account, mail or not: a calendar-only account is in no mail surface, and its line
        // still names it by its address.
        var addresses = AccountsSnapshot().Accounts.ToDictionary(entry => entry.Id, entry => entry.Address);
        string? AddressOf(string id) => addresses.GetValueOrDefault(id);
        var words = AccountUseText.OutageWords();
        ReplaceOutageLines(
            CalendarOutageLines,
            UseOutageLines.Of(connectivity.CalendarOutages, AccountCapability.Calendar, AddressOf, words));
        ReplaceOutageLines(
            ContactsOutageLines,
            UseOutageLines.Of(connectivity.ContactsOutages, AccountCapability.Contacts, AddressOf, words));
    }

    // Left alone when nothing it draws changed, so a signal that moved something else does not
    // rebuild the bars under the pointer.
    private static void ReplaceOutageLines(ObservableCollection<UseOutageLine> drawn, IReadOnlyList<UseOutageLine> next)
    {
        if (UseOutageLines.Same(drawn, next))
        {
            return;
        }
        drawn.Clear();
        foreach (var line in next)
        {
            drawn.Add(line);
        }
    }
}
