// The calendar write-status badge's mapping from the core's CalendarWriteStatus (Surface.CalendarStatus).
//
// Kept in the pure Calendar layer, no WinUI, no Visibility, so the state machine compiles into the
// test assembly and is unit-tested without a UI, exactly like the Android and Apple clients. The model
// (MailboxModel.CalendarStatus.cs) turns the result into the bound view helpers; the XAML renders them.
//
// Warning is deliberately NOT "your change was rejected": the core has confirmed the write reached the
// server, and a refresh reconciles the local view, so the warning offers a retry (a RefreshCalendar).
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

    /// <summary>The write could not be confirmed, a warning the user can tap to retry.</summary>
    Warning,
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
        CalendarWriteStatus.Failed => CalendarWriteIndicator.Warning,
        _ => CalendarWriteIndicator.Hidden,
    };

    /// <summary>Whether tapping the indicator should trigger a retry (a RefreshCalendar).</summary>
    public static bool OffersRetry(this CalendarWriteIndicator indicator) =>
        indicator == CalendarWriteIndicator.Warning;
}
