// The two things the subscription card has to turn into words: a date the account service sent,
// and an amount it sent as minor units.
//
// Both are here rather than in the card because the core carries no locale data at all
// (`AGENTS.md`, "Localisation is client-side"), so formatting is this side's job; and both are in
// their own file, WinUI-free and L10n-free, so Mailcal.Tests can link them. A wrong date in
// "you keep everything until …" is a promise about somebody's money, and it is the kind of wrong
// that still reads as a sentence.

using System;
using System.Globalization;

namespace Allodia.Mailcal.Services;

/// <summary>Dates and amounts from the account service, as a reader sees them.</summary>
internal static class AllodiaSubscriptionFormat
{
    /// <summary>
    /// An ISO 8601 date from the account service as a plain calendar date in
    /// <paramref name="culture"/>, or <c>null</c> for an empty string.
    /// </summary>
    /// <remarks>
    /// Day precision on purpose: a renewal is a date somebody's bank statement will agree with,
    /// and an hour and minute in this sentence would invite a comparison with a clock that means
    /// nothing here.
    /// <para>
    /// ⚠️ The account service is not the sync engine and does not send the engine's
    /// <c>Z</c>-suffixed instants: an offset like <c>+02:00</c>, and a plain day for a period that
    /// ends on one, both arrive. A shape this build cannot read still has the right day in its
    /// leading ten characters, so the sentence stays true and only stops being localised, which is
    /// the same fallback Apple's and Android's cards take.
    /// </para>
    /// <para>
    /// ⚠️ <b>The day is the one the service wrote, and no zone conversion is done to it.</b> That
    /// is what keeps this method and the fallback below saying the same thing: a period ending
    /// <c>2026-10-11T00:00:00+02:00</c> restated in UTC is the tenth, so a build that could parse
    /// it would name a different day from one that could not, about somebody's money, and neither
    /// would look wrong. The service names a calendar day and this repeats it.
    /// </para>
    /// </remarks>
    internal static string? Date(string? raw, CultureInfo culture)
    {
        if (string.IsNullOrEmpty(raw))
        {
            return null;
        }
        if (DateTimeOffset.TryParse(
                raw,
                CultureInfo.InvariantCulture,
                DateTimeStyles.None,
                out var instant))
        {
            return instant.Date.ToString(LongDateWithoutWeekday(culture), culture);
        }
        return raw.Length <= 10 ? raw : raw[..10];
    }

    // The culture's long date with its weekday dropped, which is what Apple's `.long` style and
    // Android's `FormatStyle.LONG` both give and .NET's "D" does not. Keeping the weekday would
    // put a different sentence on this one client for no reason anybody chose.
    private static string LongDateWithoutWeekday(CultureInfo culture)
    {
        var pattern = culture.DateTimeFormat.LongDatePattern;
        var weekday = pattern.IndexOf("dddd", StringComparison.Ordinal);
        if (weekday < 0)
        {
            return pattern;
        }
        var rest = pattern[(weekday + 4)..].TrimStart(' ', ',', '،', '、');
        var before = pattern[..weekday].TrimEnd(' ', ',', '،', '、');
        return (before.Length == 0 ? rest : $"{before} {rest}").Trim();
    }

    /// <summary>
    /// Minor units and an ISO 4217 code as something a reader can price, in
    /// <paramref name="culture"/>.
    /// </summary>
    /// <remarks>
    /// ⚠️ <b>Only for what Allodia bills directly.</b> A store's price is the store's own formatted
    /// string, drawn and never recomputed; this is the other route, where the service sends 199 and
    /// EUR. Windows reaches only this route, since it ships no store purchase at all.
    /// <para>
    /// A currency this host cannot name formats as a plain number rather than throwing: an amount
    /// with no symbol is still the right amount, and a card that threw over a code would say
    /// nothing at all.
    /// </para>
    /// </remarks>
    internal static string MinorUnits(long minorUnits, string? currency, CultureInfo culture)
    {
        var amount = minorUnits / 100m;
        if (string.IsNullOrWhiteSpace(currency))
        {
            return amount.ToString("N2", culture);
        }
        // The culture's own currency formatting with this code's symbol substituted, rather than
        // the culture's own currency: the account is billed in what the service says, which is not
        // necessarily what the reader's region spends.
        var format = (NumberFormatInfo)culture.NumberFormat.Clone();
        format.CurrencySymbol = CurrencySymbol(currency);
        return amount.ToString("C", format);
    }

    // The symbol for an ISO 4217 code, or the code itself, which is what a reader gets for one
    // this host has no region for. Never an exception: see the remark above.
    private static string CurrencySymbol(string currency) => currency.ToUpperInvariant() switch
    {
        "EUR" => "€",
        "GBP" => "£",
        "USD" => "$",
        // A code with no symbol here reads as the code, which is unambiguous and is what a bank
        // statement shows too. Adding a table of every currency would be a second copy of data the
        // platform already holds and the service already decided.
        var code => code,
    };
}
