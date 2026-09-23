// The numbers, dates and language names the Writing style screens put into their sentences. The
// core carries no locale data (AGENTS.md, "Localisation is client-side"), so formatting is this
// side's job; WinUI-free and L10n-free, with the culture, the zone and "now" passed in, so
// Mailcal.Tests can state each rule rather than trust it.

using System;
using System.Collections.Generic;
using System.Globalization;
using System.Linq;

namespace Allodia.Mailcal.Services;

/// <summary>Formatting for the Writing style surface.</summary>
internal static class WritingStyleFormat
{
    /// <summary>A credit balance, grouped the way <paramref name="culture"/> groups, with at most
    /// one decimal.</summary>
    internal static string Credits(double credits, CultureInfo culture) =>
        credits.ToString("#,##0.#", culture);

    /// <summary>
    /// When the relay reported a balance: the clock when it was today in <paramref name="zone"/>,
    /// else the day, with the year only when it is not this one. The same buckets as the mail
    /// list's label, less the weekday, which says nothing useful about a balance.
    /// </summary>
    internal static string AsOf(long unixSeconds, TimeZoneInfo zone, DateTimeOffset now, CultureInfo culture)
    {
        var local = TimeZoneInfo.ConvertTime(DateTimeOffset.FromUnixTimeSeconds(unixSeconds), zone);
        var today = TimeZoneInfo.ConvertTime(now, zone);
        var pattern = local.Date == today.Date ? "HH:mm"
            : local.Year == today.Year ? "d MMM"
            : "d MMM yyyy";
        return local.ToString(pattern, culture);
    }

    /// <summary>A day, such as when a style was learned or how far back sent mail goes.</summary>
    internal static string Date(long unixSeconds, TimeZoneInfo zone, CultureInfo culture) =>
        TimeZoneInfo.ConvertTime(DateTimeOffset.FromUnixTimeSeconds(unixSeconds), zone)
            .ToString("d MMM yyyy", culture);

    /// <summary>A habit's share of messages, as <paramref name="culture"/> writes a percentage.</summary>
    internal static string Share(byte share, CultureInfo culture) =>
        (share / 100.0).ToString("P0", culture);

    /// <summary>
    /// Languages by their own names ("Deutsch", never "German"), in the order given. A code the
    /// catalog does not ship is shown as the code, since there is no endonym to look up.
    /// </summary>
    internal static string Languages(IEnumerable<string> codes, Func<string, string> endonym) =>
        string.Join(", ", codes.Select(code => L10n.Locales.Contains(code) ? endonym(code) : code));

    /// <summary>
    /// The catalog locale the app is shown in, which is the language a style's description is
    /// written in: the stored choice, else the system's language when the catalog ships it, else
    /// the catalog's base locale, which is what the resources fall back to.
    /// </summary>
    internal static string CatalogLocale(string stored, string system) =>
        L10n.Locales.Contains(stored) ? stored
        : L10n.Locales.Contains(system) ? system
        : L10n.Locales[0];

    /// <summary>The zone the app shows dates in, or the host's when the id does not resolve.</summary>
    internal static TimeZoneInfo Zone(string id)
    {
        try
        {
            return TimeZoneInfo.FindSystemTimeZoneById(id);
        }
        catch (Exception ex) when (ex is TimeZoneNotFoundException or InvalidTimeZoneException)
        {
            return TimeZoneInfo.Local;
        }
    }
}
