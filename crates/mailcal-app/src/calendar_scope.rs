//! Which occurrence a write is allowed to name.
//!
//! Acting on a repeating event is two different requests (this Tuesday, or the standup) and
//! `docs/calendar.md` puts the question to the user. This is the other half of that rule: the
//! **answer** is checked before anything is sent.
//!
//! The token a client sends back is one the core minted for a block it drew
//! ([`mailcal_account::occurrence_wall_clock`]). So the check is not "is this a plausible
//! time" but "is this an occurrence we have", asked against the store the grid was drawn
//! from, and answered by re-minting rather than by parsing, so it cannot drift from the
//! emitter.
//!
//! It has to live here rather than beside the other write guards in `mailcal-account`: only
//! this layer reaches the store, and the store is the only thing that knows which occurrences
//! a series actually has. A builder can see that an event has *a* rule; it cannot see that the
//! rule produces the Tuesday it is being asked about.
//!
//! What it catches is not a misbehaving client so much as a **wrong token**: one the core
//! minted from the wrong field, one a client built itself instead of handing ours back, or one
//! that was valid until the series changed underneath it. Any of those addresses no instance,
//! and the transports do not agree about what happens next: a second override split at a time
//! the rule never produces, or a silent no-op reported as a save.

use engine_api::{Event, Horizon, LocalDateTime, Provider, UtcDateTime};
use mailcal_account::DetailOccurrence;
use mailcal_viewmodel::calendar::days::{date_at, from_civil};

use crate::{App, reference::EventRef};

impl<P: Provider> App<P> {
    /// Whether `occurrence` names an occurrence of `stored` that this core actually drew.
    ///
    /// Reads the store for the civil day the token names, one day either side so an
    /// occurrence's own zone cannot put it outside the window, and asks
    /// [`mailcal_account::names_an_occurrence`] whether any of those rows is the one.
    ///
    /// A store read the grid has already done, repeated once per write: not per frame.
    pub(crate) async fn names_a_stored_occurrence(
        &self,
        event: &EventRef,
        stored: &Event,
        occurrence: LocalDateTime,
    ) -> bool {
        let Some(window) = day_window(occurrence) else {
            return false;
        };
        let rows = self
            .engine
            .occurrences_in(&event.account, window)
            .await
            .unwrap_or_default();
        mailcal_account::names_an_occurrence(stored, &rows, occurrence, &self.active_zone())
    }

    /// The occurrence `token` names, with the times the expander gave it, or `None` when it
    /// names none of `stored`'s.
    ///
    /// The read behind an occurrence's own detail. `None` is not an error: a token goes stale
    /// when the series changes underneath the view it was drawn in, and the honest answer then
    /// is the series; shown as the series, with no scope question offered over it.
    pub(crate) async fn resolve_occurrence(
        &self,
        event: &EventRef,
        stored: &Event,
        token: &str,
    ) -> Option<DetailOccurrence> {
        let named = token.parse::<LocalDateTime>().ok()?;
        let window = day_window(named)?;
        let rows = self
            .engine
            .occurrences_in(&event.account, window)
            .await
            .unwrap_or_default();
        let zone = self.active_zone();
        let row = mailcal_account::stored_occurrence(stored, &rows, named, &zone)?;
        Some(DetailOccurrence {
            token: token.to_owned(),
            start: mailcal_account::occurrence_local(stored, row.start, &zone)?,
            end: mailcal_account::occurrence_local(stored, row.end, &zone)?,
        })
    }
}

/// The UTC window covering the civil day `occurrence` names, plus a day each side.
///
/// The token is a wall clock in the event's **own** zone, and the store keys occurrences by
/// instant. A day of slack each way covers every offset a zone can have (±14h) without having
/// to resolve the clock first, and resolving it is exactly what an all-day or floating
/// occurrence cannot do.
fn day_window(occurrence: LocalDateTime) -> Option<Horizon> {
    let day = from_civil(occurrence.year(), occurrence.month(), occurrence.day());
    Horizon::new(midnight_at(day - 1)?, midnight_at(day + 2)?).ok()
}

/// UTC midnight on civil day number `day`.
fn midnight_at(day: i64) -> Option<UtcDateTime> {
    let date = date_at(day);
    UtcDateTime::new(date.year(), date.month(), date.day(), 0, 0, 0).ok()
}
