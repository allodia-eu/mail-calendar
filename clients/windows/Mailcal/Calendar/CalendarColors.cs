// Turning the core's resolved calendar colours into WinUI colours, shared by every surface that draws
// a calendar or an event, so they never disagree about what a calendar looks like.
//
// The core owns the colour: a calendar resolves to a solid [Swatch] per theme, and an event on it to
// the tinted [EventSwatch] (`calendar_event_swatch`), each with a label already guaranteed ≥ 4.5:1
// (WCAG AA) against its fill (docs/calendar.md §1). A client never computes contrast, it picks the
// theme's colours and paints them. The hex is always well-formed here; the fallbacks are
// belt-and-braces so a draw can never throw on a malformed string.
using System;
using System.Collections.Generic;
using System.Globalization;
using System.Linq;
using Windows.UI;
using uniffi.mailcal_bindings;

namespace Allodia.Mailcal.Calendar;

/// <summary>
/// How an event wears its calendar's colour in one theme (docs/calendar.md §1): a tint to fill it,
/// the colour itself down its leading edge, and a title that clears WCAG AA on the tint.
/// </summary>
internal readonly record struct EventColors(Color Fill, Color Accent, Color Text);

/// <summary>Resolves calendar colours to WinUI <see cref="Color"/>s.</summary>
internal static class CalendarColors
{
    // The colour of an event whose calendar is not in the list (hidden mid-view, an account just
    // removed, or the invitation card's preview, which carries no list): a neutral grey, the one
    // Apple and Android fall back to, so the event is still legible rather than invisible.
    private const string FallbackHex = "#6b7280";

    // The core's answer for each colour and theme, asked once rather than for every event drawn.
    private static readonly Dictionary<(string Hex, bool Dark), EventColors> Events = [];

    /// <summary>
    /// The event colours of the calendar an event belongs to, from the core
    /// (<c>calendar_event_swatch</c>), or the neutral grey's for a calendar the page does not list.
    /// </summary>
    internal static EventColors EventColorsFor(
        IReadOnlyList<CalendarRow> calendars, string account, string calendar, bool dark) =>
        EventColorsOf(
            calendars.FirstOrDefault(c => c.Account == account && c.Id == calendar)?.Color.Hex ?? FallbackHex,
            dark);

    /// <summary>The event colours a calendar colour (<c>CalendarColor.Hex</c>) resolves to.</summary>
    internal static EventColors EventColorsOf(string hex, bool dark)
    {
        lock (Events)
        {
            if (!Events.TryGetValue((hex, dark), out var colors))
            {
                var swatch = MailcalBindingsMethods.CalendarEventSwatch(hex, dark);
                colors = new EventColors(Parse(swatch.Fill), Parse(swatch.Accent), Parse(swatch.Text));
                Events[(hex, dark)] = colors;
            }
            return colors;
        }
    }

    /// <summary>"#rrggbb" → an opaque colour. Neutral grey on anything unexpected.</summary>
    internal static Color Parse(string hex)
    {
        if (hex.Length == 7 && hex[0] == '#'
            && byte.TryParse(hex.AsSpan(1, 2), NumberStyles.HexNumber, CultureInfo.InvariantCulture, out var r)
            && byte.TryParse(hex.AsSpan(3, 2), NumberStyles.HexNumber, CultureInfo.InvariantCulture, out var g)
            && byte.TryParse(hex.AsSpan(5, 2), NumberStyles.HexNumber, CultureInfo.InvariantCulture, out var b))
        {
            return Color.FromArgb(255, r, g, b);
        }
        return Color.FromArgb(255, 128, 128, 128);
    }
}
