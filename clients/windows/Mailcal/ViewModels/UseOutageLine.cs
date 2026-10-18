// One line above the calendar week or the contacts list: a calendar or an address book that did not
// connect (docs/accounts.md rule 16). WinUI-free, so Mailcal.Tests links it beside UseOutageLines,
// which composes it.

namespace Allodia.Mailcal.ViewModels;

/// <summary>A use that is not connected, as its screen states it.</summary>
public sealed class UseOutageLine
{
    internal UseOutageLine(string accountId, string text)
    {
        AccountId = accountId;
        Text = text;
    }

    /// <summary>The account whose page "Account settings" opens.</summary>
    public string AccountId { get; }

    /// <summary>The title, the reason and "You're seeing what was last synced", as one line.</summary>
    public string Text { get; }
}
