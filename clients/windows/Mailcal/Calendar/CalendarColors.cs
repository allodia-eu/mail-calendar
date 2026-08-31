// Turning the core's resolved calendar colours into WinUI brushes, shared by the month grid and the
// calendar manager, so they never disagree about what a calendar looks like.
//
// The core owns the colour: each calendar resolves to a light and a dark [Swatch] whose label is
// already guaranteed ≥ 4.5:1 (WCAG AA) against its fill (docs/calendar.md §1). A client never computes
// contrast, it picks the theme's swatch and paints it. The hex is always well-formed here; the
// fallbacks are belt-and-braces so a draw can never throw on a malformed string.
using System;
using System.Collections.Generic;
using System.Globalization;
using System.Linq;
using Windows.UI;
using uniffi.mailcal_bindings;

namespace Allodia.Mailcal.Calendar;

/// <summary>Resolves calendar colours to WinUI <see cref="Color"/>s.</summary>
internal static class CalendarColors
{
    // A neutral swatch for a chip whose calendar is no longer in the list (hidden mid-view, or an
    // account just removed), grey fill, white label, so it is still legible rather than invisible.
    private static readonly Swatch Fallback = new("#8E8E93", "#FFFFFF", "#8E8E93");

    /// <summary>The theme's swatch for the calendar a chip belongs to, or a neutral fallback.</summary>
    internal static Swatch SwatchFor(
        IReadOnlyList<CalendarRow> calendars, string account, string calendar, bool dark)
    {
        var row = calendars.FirstOrDefault(c => c.Account == account && c.Id == calendar);
        if (row is null)
        {
            return Fallback;
        }
        return dark ? row.Color.Dark : row.Color.Light;
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
