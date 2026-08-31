//! Which sentence each of the core's three verdicts gets.
//!
//! No harness can raise this dialog; the warning is the AND of what the server does to overrides
//! and whether the series holds any, and both transports the local server speaks are ones that
//! keep the user's work. So the mapping is the only half of it a machine here can hold: the core
//! decides *which* of the three things is about to happen, and this pins that each one is spoken,
//! and spoken differently. Nothing matches a literal English string; the suite runs in whatever
//! language the machine is in, so the expectations come from the same catalog the code reaches for.

use std::collections::HashSet;

use mailcal_bindings::SeriesEditWarning;

use super::series_warning_text;
use crate::l10n;

const VERDICTS: [SeriesEditWarning; 3] = [
    SeriesEditWarning::OccurrencesReset,
    SeriesEditWarning::RenamesSpread,
    SeriesEditWarning::OccurrencesResetAndRenamesSpread,
];

#[test]
fn every_verdict_is_spoken() {
    for verdict in &VERDICTS {
        assert!(
            !series_warning_text(verdict).is_empty(),
            "{verdict:?} left the user with an empty warning"
        );
    }
}

#[test]
fn three_verdicts_get_three_distinct_sentences() {
    // A catalog key wired twice would say the wrong thing about the user's calendar, and nothing
    // on screen would tell the two apart.
    let sentences: HashSet<String> = VERDICTS.iter().map(series_warning_text).collect();
    assert_eq!(sentences.len(), VERDICTS.len());
}

#[test]
fn each_verdict_reaches_for_its_own_catalog_key() {
    assert_eq!(
        series_warning_text(&SeriesEditWarning::OccurrencesReset),
        l10n::event_series_warning_reset()
    );
    assert_eq!(
        series_warning_text(&SeriesEditWarning::RenamesSpread),
        l10n::event_series_warning_renames()
    );
    assert_eq!(
        series_warning_text(&SeriesEditWarning::OccurrencesResetAndRenamesSpread),
        l10n::event_series_warning_reset_and_renames()
    );
}
