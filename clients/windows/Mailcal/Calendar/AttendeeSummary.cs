// The pure half of an attendee row: what the second line under a name says. Lives here rather than
// beside the dialog because Mailcal.Tests is a plain net10.0 assembly with no WinUI TFM, a rule that
// can be stated without a UI type is tested, not trusted.
using System.Collections.Generic;
using uniffi.mailcal_bindings;

namespace Allodia.Mailcal.Calendar;

/// <summary>The text of an attendee row, with no WinUI in it.</summary>
internal static class AttendeeSummary
{
    /// <summary>
    /// The second line under an attendee: their address (when the first line used their name) and
    /// whether they called the meeting. Empty when there is nothing left to say, an attendee with
    /// no display name is shown by address on the first line, so repeating it below would be noise.
    /// </summary>
    /// <param name="attendee">The core's attendee row.</param>
    /// <param name="organizerLabel">The localised "Organiser" word, passed in so this stays pure.</param>
    internal static string Subtitle(EventAttendee attendee, string organizerLabel)
    {
        var parts = new List<string>(2);
        if (attendee.Name.Length > 0)
        {
            parts.Add(attendee.Email);
        }
        if (attendee.IsOrganizer)
        {
            parts.Add(organizerLabel);
        }
        return string.Join(" · ", parts);
    }

    /// <summary>The name to show on an attendee's first line: their name, or their address.</summary>
    internal static string Title(EventAttendee attendee) =>
        attendee.Name.Length > 0 ? attendee.Name : attendee.Email;
}
