// The calendar write-status badge's mapping from the core's CalendarWriteStatus (Surface.CalendarStatus).
//
// Kept in the pure Calendar layer, no WinUI, no Visibility, so the state machine compiles into the
// test assembly and is unit-tested without a UI, exactly like the Android and Apple clients. The model
// (MailboxModel.CalendarStatus.cs) turns the result into the bound view helpers and the words; the
// XAML renders them.
//
// Both failures draw the warning, and its tap is a refresh (a RefreshCalendar), never a re-send: a
// write that may have landed is settled by reading the calendar again (docs/calendar.md §13).
using uniffi.mailcal_bindings;

namespace Allodia.Mailcal.Calendar;

/// <summary>What the calendar header should show for the most recent write.</summary>
internal enum CalendarWriteIndicator
{
    /// <summary>Nothing to show.</summary>
    Hidden,

    /// <summary>A write is settling, a small spinner.</summary>
    Spinner,

    /// <summary>The write settled and the local view holds the server's copy, a brief check.</summary>
    Saved,

    /// <summary>The write was not saved or could not be confirmed, a warning the user can tap to
    /// refresh.</summary>
    Warning,
}

/// <summary>Which sentence the header's badge carries.</summary>
internal enum CalendarWriteLine
{
    None,
    Saving,
    Saved,
    /// <summary>"Couldn't save this change": nothing changed on the server.</summary>
    NotSaved,
    /// <summary>"Couldn't confirm": the change may have landed.</summary>
    Unconfirmed,
}

/// <summary>The pure mapping from a core write status to the header indicator.</summary>
internal static class CalendarWriteIndicators
{
    /// <summary>Maps a core <see cref="CalendarWriteStatus"/> to what the header shows. Total and pure.</summary>
    public static CalendarWriteIndicator Of(CalendarWriteStatus status) => status switch
    {
        CalendarWriteStatus.Idle => CalendarWriteIndicator.Hidden,
        CalendarWriteStatus.Saving => CalendarWriteIndicator.Spinner,
        CalendarWriteStatus.Saved => CalendarWriteIndicator.Saved,
        CalendarWriteStatus.NotSaved => CalendarWriteIndicator.Warning,
        CalendarWriteStatus.Unconfirmed => CalendarWriteIndicator.Warning,
        _ => CalendarWriteIndicator.Hidden,
    };

    /// <summary>Maps a core <see cref="CalendarWriteStatus"/> to the badge's sentence.</summary>
    public static CalendarWriteLine Line(CalendarWriteStatus status) => status switch
    {
        CalendarWriteStatus.Idle => CalendarWriteLine.None,
        CalendarWriteStatus.Saving => CalendarWriteLine.Saving,
        CalendarWriteStatus.Saved => CalendarWriteLine.Saved,
        CalendarWriteStatus.NotSaved => CalendarWriteLine.NotSaved,
        CalendarWriteStatus.Unconfirmed => CalendarWriteLine.Unconfirmed,
        _ => CalendarWriteLine.None,
    };

    /// <summary>Whether tapping the indicator should trigger a retry (a RefreshCalendar).</summary>
    public static bool OffersRetry(this CalendarWriteIndicator indicator) =>
        indicator == CalendarWriteIndicator.Warning;
}
