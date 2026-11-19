// The write-capability gates: which edit affordances a read-only account keeps, and which it loses.
//
// The core stamps `canWrite` on every calendar record it emits; this side only reads the flag off
// the exact record being rendered. The policy is cross-client (Android and Apple apply the same
// one): a per-event delete is HIDDEN when its record cannot write, a disabled delete is just a
// mystery, while the global "New event" button is DISABLED rather than hidden, so the header keeps
// its shape. Kept in the pure Calendar layer, no WinUI, no Visibility, so both decisions compile
// into the test assembly and are unit-tested without a UI, exactly like CalendarWriteIndicators.
using System.Collections.Generic;
using System.Linq;
using uniffi.mailcal_bindings;

namespace Allodia.Mailcal.Calendar;

/// <summary>The pure write-capability gates for the calendar's edit affordances.</summary>
internal static class CalendarWriteGating
{
    /// <summary>
    /// Whether "New event" is offered at all: at least one calendar the user can write to. An
    /// empty list, nothing synced yet, means no: there is nowhere a new event could go.
    /// </summary>
    public static bool CanCreate(IReadOnlyList<CalendarRow> calendars) =>
        calendars.Any(calendar => calendar.CanWrite);

    /// <summary>Whether this agenda row offers a delete: the flag stamped on this exact record.</summary>
    public static bool OffersDelete(EventRow row) => row.CanWrite;
}
