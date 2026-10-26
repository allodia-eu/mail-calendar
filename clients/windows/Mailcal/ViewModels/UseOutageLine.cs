// One line above the calendar week or the contacts list: a calendar or an address book that did not
// connect (docs/accounts.md rule 16). WinUI-free, so Mailcal.Tests links it beside UseOutageLines,
// which composes it.

namespace Allodia.Mailcal.ViewModels;

/// <summary>A use that is not connected, as its screen states it.</summary>
public sealed class UseOutageLine
{
    internal UseOutageLine(string accountId, string text, string action, bool reviews)
    {
        AccountId = accountId;
        Text = text;
        Action = action;
        Reviews = reviews;
    }

    /// <summary>The account whose page "Account settings" opens, or whose server change "Review"
    /// opens.</summary>
    public string AccountId { get; }

    /// <summary>The title, the reason and "You're seeing what was last synced", as one line.</summary>
    public string Text { get; }

    /// <summary>The button's label: "Review" where <see cref="Reviews"/>, "Account settings"
    /// otherwise.</summary>
    public string Action { get; }

    /// <summary>Whether the use is paused for a server change, so the button opens the question
    /// that allows it rather than the account's page.</summary>
    public bool Reviews { get; }
}
