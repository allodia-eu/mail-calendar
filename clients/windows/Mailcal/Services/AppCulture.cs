// The formatting culture the app's Language choice implies, the other half of LanguageStore.
//
// Why this exists at all: picking a language has to change the DATES as well as the words. The
// chrome comes from an MRT-Core resource qualifier (L10n.SetLanguage), which is a resource-lookup
// mechanism and nothing more, it does not touch CultureInfo. But every date the client renders
// formats against CultureInfo.CurrentCulture: the mail list's relative label (TimeZones), the
// calendar's period title (CalendarFormat.PeriodTitle) and its weekday headers
// (MonthGridView.AbbreviatedDayNames). On Windows that culture follows the OS *regional format*,
// which the Language picker does not set, so the two resolve independently and disagree the moment
// the choice differs from the host. The symptom is a fully Dutch UI listing "Mon"/"Sun" and a
// calendar headed "Jul 2026 · Mon Tue Wed", which is exactly what the nl store screenshots showed.
//
// Android is the reference: AppCompatDelegate.setApplicationLocales updates the configuration, and
// every screen formats with configuration.locales[0], one resolution, both uses. This pins the
// same property here.
//
// "system" deliberately leaves the host culture alone rather than deriving one from the OS UI
// language. On Windows the display language and the regional format are two separate settings, and
// the regional format IS the user's stated formatting preference, overriding it would be this same
// bug with the sign flipped. It is also the closest analogue of Android's empty locale list.

using System.Globalization;

namespace Allodia.Mailcal.Services;

/// <summary>Maps the app's language choice onto the culture its dates are formatted in.</summary>
internal static class AppCulture
{
    /// <summary>
    /// The culture <paramref name="choice"/> ("system", or any locale the shared catalog ships)
    /// implies, or <c>null</c> to follow the host's regional format. Pure, so the mapping is
    /// unit-testable.
    /// </summary>
    /// <remarks>
    /// A neutral culture ("nl", not "nl-NL") on purpose: the choice is a *language*, and the
    /// neutral culture carries the month and day names that go with it without also asserting a
    /// country the user never picked.
    /// </remarks>
    /// <remarks>
    /// The accepted set is <see cref="L10n.Locales"/>, straight from the catalog, so a language
    /// added to messages/ needs no edit here. Hardcoding the pair is what this file exists to
    /// prevent one level up: a locale the picker offers but this method does not know would render
    /// its own words over the *host's* dates, the very split the class was written to close.
    /// </remarks>
    public static CultureInfo? Resolve(string choice) =>
        L10n.Locales.Contains(choice) ? CultureInfo.GetCultureInfo(choice) : null;

    /// <summary>
    /// Pins the process's formatting culture to <paramref name="choice"/>, so dates render in the
    /// language the chrome is in. A "system" choice is left to the host. Call at startup before any
    /// window is created (App's constructor), alongside the resource-language override.
    /// </summary>
    public static void Apply(string choice)
    {
        if (Resolve(choice) is not { } culture)
        {
            return;
        }
        // Both halves are needed: the Default* pair seeds threads that have not materialized a
        // culture yet (the pool threads the engine's callbacks arrive on), while the Current* pair
        // covers the UI thread we are already running on, whose culture is read the moment the
        // first row renders.
        CultureInfo.DefaultThreadCurrentCulture = culture;
        CultureInfo.DefaultThreadCurrentUICulture = culture;
        CultureInfo.CurrentCulture = culture;
        CultureInfo.CurrentUICulture = culture;
    }
}
