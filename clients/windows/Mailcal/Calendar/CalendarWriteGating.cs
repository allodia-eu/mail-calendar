// A per-event delete is HIDDEN when the record it is drawn from cannot write: a disabled delete is
// just a mystery. The core stamps `canWrite` on every record it emits; this side reads the flag off
// the exact record being rendered. Whether "New event" is offered at all is the core's
// NewEventCalendars, read by MailboxModel. Kept in the pure Calendar layer, no WinUI, so it
// compiles into the test assembly.
using uniffi.mailcal_bindings;

namespace Allodia.Mailcal.Calendar;

/// <summary>The pure write-capability gates for the calendar's edit affordances.</summary>
internal static class CalendarWriteGating
{
    /// <summary>Whether this agenda row offers a delete: the flag stamped on this exact record.</summary>
    public static bool OffersDelete(EventRow row) => row.CanWrite;
}
