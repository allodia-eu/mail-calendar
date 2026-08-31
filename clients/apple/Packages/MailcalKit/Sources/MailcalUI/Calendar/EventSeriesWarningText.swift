// What a series edit costs the occurrences the user singled out, as a sentence.
//
// Wording only, the same division of labour as `EventRepeatText`: the core decided *whether* there
// is anything to say and *which* of the three things it is, it pairs what this account's server
// does to overrides with whether this series holds any, so this is a `switch` over a closed set
// and a catalog lookup. No provider is ever named: what the user needs is what is about to happen
// to their own calendar, and the transport it happens on is not their concern.

import MailcalBindings

/// The warning to show before committing a series-level edit, or `nil` when there is nothing to
/// say, the common case, and deliberately so. A dialog that appears on every repeating event is
/// what teaches people to click past the one that mattered.
func seriesWarningText(_ warning: SeriesEditWarning?) -> String? {
    switch warning {
    case .none:
        return nil
    case .occurrencesReset:
        return L10n.event_series_warning_reset()
    case .renamesSpread:
        return L10n.event_series_warning_renames()
    case .occurrencesResetAndRenamesSpread:
        return L10n.event_series_warning_reset_and_renames()
    }
}
