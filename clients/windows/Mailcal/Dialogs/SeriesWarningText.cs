// What a series edit costs the occurrences the user singled out, as a sentence.
//
// Wording only, the same division of labour as EventRepeatText: the core decided *whether* there is
// anything to say and *which* of the three things it is, it pairs what this account's server does
// to overrides with whether this series holds any, so this is a switch over a closed set and a
// catalog lookup. No provider is ever named: what the user needs is what is about to happen to
// their own calendar, and the transport it happens on is not their concern.
//
// It lives here rather than beside EventOpen in Calendar/ because it reaches L10n, which needs a
// Windows TFM Mailcal.Tests does not have, so what a test can reach stays pure and this cannot.
using uniffi.mailcal_bindings;

namespace Allodia.Mailcal.Dialogs;

/// <summary>The series-edit warning, in the user's language.</summary>
internal static class SeriesWarningText
{
    /// <summary>
    /// The warning to show before committing a series-level edit, or <c>null</c> when there is
    /// nothing to say, the common case, and deliberately so. A dialog that appears on every
    /// repeating event is what teaches people to click past the one that mattered.
    /// </summary>
    internal static string? For(SeriesEditWarning? warning) => warning switch
    {
        SeriesEditWarning.OccurrencesReset => L10n.EventSeriesWarningReset(),
        SeriesEditWarning.RenamesSpread => L10n.EventSeriesWarningRenames(),
        SeriesEditWarning.OccurrencesResetAndRenamesSpread =>
            L10n.EventSeriesWarningResetAndRenames(),
        _ => null,
    };
}
