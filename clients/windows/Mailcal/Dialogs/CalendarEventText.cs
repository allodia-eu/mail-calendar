// The reminder and attendee summaries, localised, the WinUI twin of Android's reminderText. The
// *bucketing* is pure and unit-tested (CalendarEventSummary in the Calendar layer); this maps the
// bucket / token to an L10n string, which is a WinUI-resource call and so lives here rather than in
// the test-linked pure layer. Shared by the editor's display-only rows and the detail view, so the
// two never phrase the same event differently. The repeat rule's own sentence is EventRepeatText.
using Allodia.Mailcal.Calendar;
using uniffi.mailcal_bindings;

namespace Allodia.Mailcal.Dialogs;

/// <summary>Localised reminder / recurrence summaries for the calendar editor and detail views.</summary>
internal static class CalendarEventText
{
    /// <summary>The reminder summary (e.g. "15 minutes before", "At start", "None").</summary>
    internal static string Reminder(int? minutes) => CalendarEventSummary.ReminderBucketOf(minutes) switch
    {
        ReminderBucket.AtStart => L10n.EventReminderAtStart(),
        ReminderBucket.Minutes m => L10n.EventReminderMinutes(m.N),
        ReminderBucket.Hours h => L10n.EventReminderHours(h.N),
        ReminderBucket.Days d => L10n.EventReminderDays(d.N),
        _ => L10n.EventReminderNone(),
    };

    /// <summary>How one attendee answered. Third person, this is somebody else's answer, unlike
    /// the invitation card's "You accepted".</summary>
    internal static string AttendeeResponse(ResponseStatus response) => response switch
    {
        ResponseStatus.Accepted => L10n.EventAttendeeAccepted(),
        ResponseStatus.Declined => L10n.EventAttendeeDeclined(),
        ResponseStatus.Tentative => L10n.EventAttendeeTentative(),
        ResponseStatus.Delegated => L10n.EventAttendeeDelegated(),
        _ => L10n.EventAttendeeNeedsAction(),
    };
}
