// The calendar write-status hint, pulled on a Surface.CalendarStatus change: Saving while a
// create/edit/delete settles, then the terminal Saved/Failed. Split out of MailboxModel.cs to keep it
// under the 500-line cap; the partial class is defined there and this extends it.
//
// The status -> indicator mapping is the pure, unit-tested CalendarWriteIndicators.Of (Calendar layer);
// this file only turns the indicator into the bound Visibility/text the XAML needs (so the view needs no
// converters), mirroring how UpdateSendStatus exposes the send hint. The write-capability gate for the
// "New event" button (CalendarWriteGating, same pure layer) is bound from here too.
using Allodia.Mailcal.Calendar;
using Microsoft.UI.Xaml;
using uniffi.mailcal_bindings;

namespace Allodia.Mailcal.Services;

public sealed partial class MailboxModel
{
    private bool _newEventEnabled;

    /// <summary>
    /// Whether the "New event" button is enabled: at least one calendar accepts writes
    /// (CalendarWriteGating.CanCreate). Starts disabled, before any calendar has synced there is
    /// nowhere a new event could go, and is refreshed on the calendar leg of Reload. Disabled, not
    /// hidden, so the header keeps its shape.
    /// </summary>
    public bool NewEventEnabled
    {
        get => _newEventEnabled;
        private set => Set(ref _newEventEnabled, value);
    }

    private CalendarWriteStatus _calendarWriteStatus = CalendarWriteStatus.Idle;

    /// <summary>
    /// The calendar write currently settling, raw.
    /// </summary>
    /// <remarks>
    /// The header badge reads the mapped helpers below; the invitation card needs the status itself,
    /// because its respond row says something different about each state (and takes the buttons out
    /// of reach while one is in flight). An RSVP settles through this same surface as every other
    /// calendar write, it changes the calendar, so it reports where calendar changes report.
    /// </remarks>
    internal CalendarWriteStatus CalendarWrite => _calendarWriteStatus;

    /// <summary>Records the latest calendar-write status and refreshes its bound view helpers.</summary>
    private void UpdateCalendarWriteStatus(CalendarWriteStatus status)
    {
        _calendarWriteStatus = status;
        Raise(nameof(CalendarWrite));
        Raise(nameof(CalendarWriteStatusVisible));
        Raise(nameof(CalendarWriteBusyVisibility));
        Raise(nameof(CalendarWriteSavedVisibility));
        Raise(nameof(CalendarWriteWarningVisibility));
        Raise(nameof(CalendarWriteStatusText));
    }

    private CalendarWriteIndicator Indicator => CalendarWriteIndicators.Of(_calendarWriteStatus);

    // --- Bindable view helpers (so the XAML needs no converters) --------------

    /// <summary>Whether the write hint should show (a write is settling or just finished).</summary>
    public bool CalendarWriteStatusVisible => Indicator != CalendarWriteIndicator.Hidden;
    /// <summary>The in-flight spinner, shown only while the write is still settling.</summary>
    public Visibility CalendarWriteBusyVisibility => Vis(Indicator == CalendarWriteIndicator.Spinner);
    /// <summary>The success check, shown briefly once the write is confirmed in the store.</summary>
    public Visibility CalendarWriteSavedVisibility => Vis(Indicator == CalendarWriteIndicator.Saved);
    /// <summary>The tap-to-retry warning, shown when the write could not be confirmed.</summary>
    public Visibility CalendarWriteWarningVisibility => Vis(Indicator == CalendarWriteIndicator.Warning);
    /// <summary>The hint / accessibility text for the current status.</summary>
    public string CalendarWriteStatusText => _calendarWriteStatus switch
    {
        CalendarWriteStatus.Saving => L10n.CalendarSaving(),
        CalendarWriteStatus.Saved => L10n.CalendarSaved(),
        CalendarWriteStatus.Failed => L10n.CalendarSaveUnconfirmed(),
        _ => string.Empty,
    };

    private static Visibility Vis(bool on) => on ? Visibility.Visible : Visibility.Collapsed;
}
