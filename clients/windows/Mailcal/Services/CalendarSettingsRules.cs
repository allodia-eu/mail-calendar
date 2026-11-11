// What the calendar half of the main window reads from the settings, so a Settings signal that
// moved none of it leaves the agenda and the grid alone. Most Settings signals are a change on an
// account's page, which moves nothing the calendar draws, and rebuilding every event, re-pulling
// the grid's page and re-applying its display settings for each held the UI thread long enough
// that the control pressed never got to show it was pressed.

using System.Collections.Generic;
using System.Linq;
using uniffi.mailcal_bindings;

namespace Allodia.Mailcal.Services;

internal static class CalendarSettingsRules
{
    /// <summary>The settings the agenda and the grid are drawn with: the zone and clock the
    /// events are formatted in, the display settings the grid applies, and each calendar's name,
    /// colour, visibility and rights, which decide what is shown and whether a new event can be
    /// made.</summary>
    internal static string Fingerprint(
        string zone, bool use24Hour, DisplaySettings? display, IReadOnlyList<CalendarRow> calendars) =>
        string.Join('\n', [
            $"zone:{zone}/{use24Hour}",
            $"display:{display}",
            .. calendars.Select(calendar => $"calendar:{calendar}"),
        ]);
}
