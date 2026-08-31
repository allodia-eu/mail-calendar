//! Calendar colour resolution: turning each cached calendar into a [`CalendarRow`] with a
//! decided swatch. Split out of [`crate::calendar_cache`] (which owns the cache and the page
//! query) so each file keeps one responsibility and stays under the size limit.
//!
//! The rule is a product decision and lives in the core so all three clients agree
//! (`docs/calendar.md` → "Colour defaults"): a calendar's colour is the user's override, else
//! the server's colour snapped to the palette, else a palette hue no other calendar is already
//! using: so a freshly connected account comes up in distinct hues, not a wall of blue.

use std::collections::HashSet;

use engine_api::Provider;
use mailcal_account::DefaultCalendar;
use mailcal_viewmodel::calendar::{
    CalendarRow,
    color::{PALETTE, nearest_index, resolve as resolve_color},
};

use crate::{App, calendar_cache::CalendarCache};

/// The next palette slot not in `taken`, claiming it so two colourless calendars never match.
///
/// `next` walks the palette in order across calls; the claimed slot is inserted into `taken` so a
/// later colourless calendar skips it. Once every slot is in use; more calendars than the palette
/// has hues; it cycles deterministically rather than failing, so a calendar always gets *a*
/// colour.
fn claim_free_palette(next: &mut usize, taken: &mut HashSet<usize>) -> usize {
    let count = PALETTE.len();
    for _ in 0..count {
        let index = *next % count;
        *next += 1;
        if taken.insert(index) {
            return index;
        }
    }
    let index = *next % count;
    *next += 1;
    index
}

impl<P: Provider> App<P> {
    /// The cached calendars with the user's decisions applied; shown/hidden, and the colour
    /// resolved from their override, else the server's, else their position in the list.
    ///
    /// Applied here rather than in the cache so the manager's toggles take effect on the next pull,
    /// with no sync and no network.
    /// The calendar list on its own, without pulling a page.
    ///
    /// Settings needs the rows (which calendars exist, and which one is the default) while no
    /// grid is on screen and no page is being drawn. Same in-memory cache and same resolution as
    /// the list that rides along with a page, so the two cannot disagree.
    #[must_use]
    pub fn calendars(&self) -> Vec<CalendarRow> {
        let cache = self.calendar_cache.lock().expect("calendar cache poisoned");
        self.resolved_calendars(&cache)
    }

    pub(crate) fn resolved_calendars(&self, cache: &CalendarCache) -> Vec<CalendarRow> {
        let prefs = self
            .calendar_prefs
            .lock()
            .expect("calendar-prefs mutex poisoned");

        // A calendar's colour is, in order: the user's override, else the server's colour, else a
        // palette hue **no other calendar is already using**. This runs in the core, so all three
        // clients inherit the same defaults; the user can still override any of them
        // (docs/calendar.md → "Colour defaults").
        //
        // First pass: which palette hues are already spoken for, by an override or a server
        // colour, each snapped to the palette. A colourless calendar avoids these, so a
        // freshly connected account comes up in distinct hues instead of a wall of blue,
        // across accounts too.
        let mut taken: HashSet<usize> = cache
            .calendars
            .iter()
            .filter_map(|calendar| {
                let decided = prefs.get(&calendar.account, &calendar.id);
                let fixed = decided.color.or_else(|| calendar.server_color.clone());
                fixed.as_deref().and_then(nearest_index)
            })
            .collect();

        // Second pass: resolve each, handing every colourless calendar the next free hue in the
        // cache's stable order: so adding a calendar takes a fresh colour without recolouring the
        // ones already on screen.
        let mut next = 0usize;
        // A DEBUG-only diagnostic: *why* is each calendar the colour it is; override, the server's
        // own colour, or an auto-assigned distinct hue. Answers a "the colour didn't change / isn't
        // what I set" question straight from the log, without inspecting the store. Off by default,
        // so it costs nothing in the steady state; the never-log-content rule holds: a calendar id
        // is a provider key (never an address), and no title, time, or attendee is logged.
        let debug = log::log_enabled!(log::Level::Debug);
        let mut sources: Vec<String> = Vec::new();
        let rows: Vec<CalendarRow> = cache
            .calendars
            .iter()
            .map(|calendar| {
                let decided = prefs.get(&calendar.account, &calendar.id);
                let (color, source) = if decided.color.is_some() {
                    (
                        resolve_color(
                            decided.color.as_deref(),
                            calendar.server_color.as_deref(),
                            0,
                        ),
                        "override",
                    )
                } else if calendar.server_color.is_some() {
                    (
                        resolve_color(None, calendar.server_color.as_deref(), 0),
                        "server",
                    )
                } else {
                    (
                        resolve_color(None, None, claim_free_palette(&mut next, &mut taken)),
                        "auto",
                    )
                };
                if debug {
                    sources.push(format!("{}={}({})", calendar.id, source, color.hex));
                }
                CalendarRow {
                    account: calendar.account.clone(),
                    id: calendar.id.clone(),
                    name: calendar.name.clone(),
                    visible: decided.visible,
                    color,
                    can_write: calendar.can_write,
                    is_default: false,
                }
            })
            .collect();
        if debug {
            log::debug!("resolved calendar colours: [{}]", sources.join(", "));
        }
        mark_default(rows, prefs.default_calendar())
    }
}

/// Marks the one calendar a new event is filed on unless the user picks another.
///
/// The stored choice wins **while it still exists and can still be written to**; otherwise the
/// first writable calendar does. Both halves matter: a choice can outlive its calendar (an account
/// removed elsewhere, a calendar deleted on the server) and it can outlive its *writability* (a
/// share downgraded to read-only), and a default that refuses the write is worse than no default,
/// because the failure arrives at save time with the event already typed.
///
/// Resolved here rather than in each client so "the default calendar" means one thing across four
/// of them, and so a client needs no fallback rule of its own, only `is_default`.
fn mark_default(mut rows: Vec<CalendarRow>, stored: Option<&DefaultCalendar>) -> Vec<CalendarRow> {
    let chosen = stored
        .and_then(|choice| {
            rows.iter().position(|row| {
                row.account == choice.account && row.id == choice.calendar && row.can_write
            })
        })
        .or_else(|| rows.iter().position(|row| row.can_write));
    if let Some(index) = chosen {
        rows[index].is_default = true;
    }
    rows
}

#[cfg(test)]
mod default_calendar_tests {
    use mailcal_account::DefaultCalendar;
    use mailcal_viewmodel::calendar::{CalendarRow, color::resolve};

    use super::mark_default;

    fn row(account: &str, id: &str, can_write: bool) -> CalendarRow {
        CalendarRow {
            account: account.to_owned(),
            id: id.to_owned(),
            name: id.to_owned(),
            color: resolve(None, None, 0),
            visible: true,
            can_write,
            is_default: false,
        }
    }

    fn chose(account: &str, calendar: &str) -> DefaultCalendar {
        DefaultCalendar {
            account: account.to_owned(),
            calendar: calendar.to_owned(),
        }
    }

    fn defaulted(rows: &[CalendarRow]) -> Vec<&str> {
        rows.iter()
            .filter(|row| row.is_default)
            .map(|row| row.id.as_str())
            .collect()
    }

    #[test]
    fn nobody_having_chosen_falls_back_to_the_first_writable_calendar() {
        let rows = mark_default(
            vec![row("a", "readonly", false), row("a", "work", true)],
            None,
        );
        assert_eq!(defaulted(&rows), ["work"]);
    }

    #[test]
    fn a_stored_choice_wins_over_the_first_writable_one() {
        let rows = mark_default(
            vec![row("a", "work", true), row("a", "private", true)],
            Some(&chose("a", "private")),
        );
        assert_eq!(defaulted(&rows), ["private"]);
    }

    #[test]
    fn a_choice_is_matched_on_its_account_as_well_as_its_id() {
        // A calendar id is unique only within its account, so two accounts can each have a `work`.
        // Matching on the id alone would default to whichever came first.
        let rows = mark_default(
            vec![row("a", "work", true), row("b", "work", true)],
            Some(&chose("b", "work")),
        );
        let marked: Vec<&str> = rows
            .iter()
            .filter(|row| row.is_default)
            .map(|row| row.account.as_str())
            .collect();
        assert_eq!(marked, ["b"]);
    }

    #[test]
    fn a_choice_whose_calendar_is_gone_falls_back() {
        let rows = mark_default(vec![row("a", "work", true)], Some(&chose("a", "deleted")));
        assert_eq!(defaulted(&rows), ["work"]);
    }

    #[test]
    fn a_choice_that_turned_read_only_falls_back() {
        // A share downgraded to read-only. Keeping it as the default would fail at save time, with
        // the event already typed; worse than never having offered it.
        let rows = mark_default(
            vec![row("a", "shared", false), row("a", "work", true)],
            Some(&chose("a", "shared")),
        );
        assert_eq!(defaulted(&rows), ["work"]);
    }

    #[test]
    fn nothing_writable_means_no_default_at_all() {
        let rows = mark_default(vec![row("a", "shared", false)], Some(&chose("a", "shared")));
        assert!(defaulted(&rows).is_empty());
    }

    #[test]
    fn exactly_one_row_is_ever_the_default() {
        // The property every client relies on instead of keeping a fallback rule of its own.
        let rows = mark_default(
            vec![
                row("a", "work", true),
                row("a", "private", true),
                row("b", "work", true),
            ],
            Some(&chose("a", "private")),
        );
        assert_eq!(rows.iter().filter(|row| row.is_default).count(), 1);
    }
}

#[cfg(test)]
mod palette_assignment_tests {
    use std::collections::HashSet;

    use super::{PALETTE, claim_free_palette};

    #[test]
    fn colourless_calendars_get_distinct_palette_hues() {
        // No hue is spoken for: three colourless calendars come out in three different slots, in
        // order: the "wall of blue" this exists to prevent.
        let (mut taken, mut next) = (HashSet::new(), 0);
        let picks = [
            claim_free_palette(&mut next, &mut taken),
            claim_free_palette(&mut next, &mut taken),
            claim_free_palette(&mut next, &mut taken),
        ];
        assert_eq!(picks, [0, 1, 2]);
    }

    #[test]
    fn a_colourless_calendar_skips_hues_already_taken() {
        // Slots 0 and 2 are already worn (a server colour or an override, snapped to the palette);
        // the colourless calendars step over them rather than colliding.
        let mut taken: HashSet<usize> = [0, 2].into_iter().collect();
        let mut next = 0;
        assert_eq!(claim_free_palette(&mut next, &mut taken), 1);
        assert_eq!(claim_free_palette(&mut next, &mut taken), 3);
        assert_eq!(claim_free_palette(&mut next, &mut taken), 4);
    }

    #[test]
    fn it_cycles_rather_than_failing_once_every_hue_is_in_use() {
        // More calendars than the palette has hues: it wraps deterministically, so a calendar
        // always gets *a* colour rather than none.
        let mut taken: HashSet<usize> = (0..PALETTE.len()).collect();
        let mut next = 0;
        let first = claim_free_palette(&mut next, &mut taken);
        assert!(first < PALETTE.len());

        let mut taken_again: HashSet<usize> = (0..PALETTE.len()).collect();
        let mut next_again = 0;
        assert_eq!(claim_free_palette(&mut next_again, &mut taken_again), first);
    }
}
