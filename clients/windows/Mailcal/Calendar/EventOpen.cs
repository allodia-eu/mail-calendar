// What a tap on a calendar surface yields: which event, and which occurrence of it the user was
// looking at.
//
// It travels with the reference rather than being re-derived from the detail, because by the time
// the detail is loaded it describes the *series*, which day was tapped is no longer knowable from
// it. Pure (no WinUI type), so the rule below is pinned from Mailcal.Tests.
namespace Allodia.Mailcal.Calendar;

/// <summary>An event a tap opened, and the occurrence of it that was drawn.</summary>
/// <param name="Account">The owning account's id.</param>
/// <param name="Key">The event's provider key.</param>
/// <param name="Occurrence">
/// That occurrence's own start, as the core minted it, empty when there is none to name: a one-off
/// event, or an agenda row, which lists the series rather than any one of its occurrences.
/// </param>
internal readonly record struct EventOpen(string Account, string Key, string Occurrence)
{
    /// <summary>Whether a write from here has to ask <i>This event · All events</i> first.</summary>
    internal bool AsksAboutTheSeries => !string.IsNullOrEmpty(Occurrence);
}
