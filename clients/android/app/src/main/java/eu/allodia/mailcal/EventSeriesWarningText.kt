// What a series edit costs the occurrences the user singled out, as a sentence.
//
// Wording only, the same division of labour as `EventRepeatText`: the core decided *whether* there
// is anything to say and *which* of the three things it is, it pairs what this account's server
// does to overrides with whether this series holds any, so this is a `when` over a closed set and
// a catalog lookup. No provider is ever named: what the user needs is what is about to happen to
// their own calendar, and the transport it happens on is not their concern.
package eu.allodia.mailcal

import android.content.Context
import uniffi.mailcal_bindings.SeriesEditWarning

/**
 * The warning to show before committing a series-level edit, or `null` when there is nothing to
 * say, the common case, and deliberately so. A dialog that appears on every repeating event is
 * what teaches people to click past the one that mattered.
 */
internal fun seriesWarningText(ctx: Context, warning: SeriesEditWarning?): String? =
    when (warning) {
        null -> null
        SeriesEditWarning.OCCURRENCES_RESET -> L10n.event_series_warning_reset(ctx)
        SeriesEditWarning.RENAMES_SPREAD -> L10n.event_series_warning_renames(ctx)
        SeriesEditWarning.OCCURRENCES_RESET_AND_RENAMES_SPREAD ->
            L10n.event_series_warning_reset_and_renames(ctx)
    }
