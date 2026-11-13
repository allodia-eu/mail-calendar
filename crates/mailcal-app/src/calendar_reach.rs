//! How far the calendar has been looked at, and widening it to a page a client asks for.
//!
//! Every launch expands the rolling horizon. A page outside it is unknown rather than empty
//! (`docs/calendar.md` §4), and the page query cannot expand anything because it never touches the
//! store. So the query records the days it could not answer, and [`App::widen_calendar`] runs off
//! the caller's thread: it expands every account's stored events over a window reaching those days
//! and rebuilds the cache over it, which signals `Surface::Calendar` for the client to pull again.
//!
//! The window grows only towards the pages a user asks for, [`WIDEN_STEP_DAYS`] at a time, and
//! lasts the session: the next launch expands the rolling horizon alone. It is one window because
//! the engine expands one, so a page two years back expands the two years in between.

use std::time::Duration;

use engine_api::{ApiError, CalendarDate, Horizon, Provider, UnexpandableEvent};
use mailcal_viewmodel::calendar::days::day_number;

use super::{CalendarCache, covers, midnight_at, rolling_horizon};
use crate::{App, calendar_unexpandable::unexpandable_line};

/// How far past the page asked for a widening reaches, so paging a week at a time widens once a
/// quarter rather than once a page: every widening re-expands every stored event.
const WIDEN_STEP_DAYS: i64 = 91;

/// How many times a widening tries while another pass holds a calendar.
const WIDEN_ATTEMPTS: u32 = 5;

/// The wait before a widening's second try, doubled before each one after it.
const WIDEN_FIRST_WAIT: Duration = Duration::from_millis(250);

/// What expanding every account over the calendar window came to.
#[derive(Default)]
pub(crate) struct Expansion {
    /// Whether the cache changed, so the agenda and the host are owed a rebuild.
    pub(crate) changed: bool,
    /// Whether every account was expanded over the window. When one was not, the cache claims
    /// only the window expanded before, never days the store holds no occurrences for.
    pub(crate) complete: bool,
    /// The events the engine could not expand.
    pub(crate) refused: Vec<UnexpandableEvent>,
}

impl<P: Provider> App<P> {
    /// Whether a page has asked for days the calendar window does not reach since this was last
    /// asked. True once per widening owed; the caller then runs [`App::widen_calendar`].
    #[must_use]
    pub fn take_calendar_widening(&self) -> bool {
        let mut cache = self.calendar_cache.lock().expect("calendar cache poisoned");
        std::mem::take(&mut cache.widening_owed)
    }

    /// Expands every account's stored events over a window reaching every page asked for this
    /// session, and rebuilds the cache over it. No network.
    ///
    /// A calendar another pass holds is tried again a few times. One held for longer is widened
    /// by that pass, which expands the same window when it finishes, and a widening that gives up
    /// is owed again, so the next page asked for starts another.
    pub async fn widen_calendar(&self) {
        let mut wait = WIDEN_FIRST_WAIT;
        for attempt in 1..=WIDEN_ATTEMPTS {
            let expansion = self.expand_calendar().await;
            if expansion.changed {
                self.rebuild_calendar().await;
            }
            if let Some(line) = unexpandable_line(&expansion.refused) {
                log::warn!("widen_calendar: {line}");
            }
            if expansion.complete {
                return;
            }
            if attempt < WIDEN_ATTEMPTS {
                tokio::time::sleep(wait).await;
                wait *= 2;
            }
        }
        log::info!("widen_calendar: a calendar stayed busy, so the widening is owed again");
        self.owe_calendar_widening();
    }

    /// Marks a widening owed, for the next page query to hand to its caller.
    pub(crate) fn owe_calendar_widening(&self) {
        self.calendar_cache
            .lock()
            .expect("calendar cache poisoned")
            .widening_owed = true;
    }

    /// The window the calendar is expanded over: the rolling horizon, widened to every page asked
    /// for this session.
    pub(crate) fn calendar_window(&self) -> Option<Horizon> {
        let rolling = rolling_horizon()?;
        let cache = self.calendar_cache.lock().expect("calendar cache poisoned");
        Some(
            cache
                .visited
                .map_or(rolling, |visited| span(rolling, visited)),
        )
    }

    /// Expands every account over [`Self::calendar_window`] and rebuilds the cache over what was
    /// expanded.
    ///
    /// Held under the expansion lock with every other rebuild, so a rebuild over a narrower
    /// window cannot land after this one and take back the days it claimed.
    pub(crate) async fn expand_calendar(&self) -> Expansion {
        let _held = self.calendar_expansion.lock().await;
        let Some(window) = self.calendar_window() else {
            return Expansion::default();
        };
        let zone = self.active_zone();
        let accounts = self.account_handles().await;
        let results = futures::future::join_all(
            accounts
                .iter()
                .map(|account| self.engine.expand_horizon(&account.id, window, &zone)),
        )
        .await;
        let mut expansion = Expansion {
            complete: true,
            ..Expansion::default()
        };
        for (acct, result) in results.into_iter().enumerate() {
            match result {
                Ok(expanded) => expansion.refused.extend(expanded.unexpandable),
                Err(ApiError::Busy) => {
                    expansion.complete = false;
                    log::debug!("calendar[a{acct}]: busy, another pass holds a calendar");
                }
                Err(error) => {
                    expansion.complete = false;
                    log::warn!("calendar[a{acct}]: expansion failed: {error}");
                }
            }
        }
        let claim = if expansion.complete {
            self.calendar_cache
                .lock()
                .expect("calendar cache poisoned")
                .expanded = Some(window);
            window
        } else {
            self.expanded_window().unwrap_or(window)
        };
        expansion.changed = self.rebuild_calendar_cache(claim).await;
        expansion
    }

    /// Rebuilds the cache, expanding nothing, over the window last expanded.
    ///
    /// Returns whether the cache changed.
    pub(crate) async fn rebuild_calendar_expanded(&self) -> bool {
        let _held = self.calendar_expansion.lock().await;
        let Some(window) = self.expanded_window() else {
            return false;
        };
        self.rebuild_calendar_cache(window).await
    }

    /// The window every account was last expanded over in this session, or the rolling horizon
    /// before any was: the one a launch's store was left expanded over.
    fn expanded_window(&self) -> Option<Horizon> {
        let expanded = self
            .calendar_cache
            .lock()
            .expect("calendar cache poisoned")
            .expanded;
        expanded.or_else(rolling_horizon)
    }
}

/// Records that a page asked for `days`, and owes a widening when the calendar window, as widened
/// so far, does not reach them.
///
/// Days the window reaches but the cache has not claimed are already on their way: the first
/// sync, or a widening still running.
pub(super) fn note_page(cache: &mut CalendarCache, days: &[CalendarDate]) {
    if covers(cache.window, days) {
        return;
    }
    let Some(rolling) = rolling_horizon() else {
        return;
    };
    let reach = cache
        .visited
        .map_or(rolling, |visited| span(rolling, visited));
    if covers(Some(reach), days) {
        return;
    }
    let numbers = days.iter().map(|&day| day_number(day));
    let (Some(first), Some(last)) = (numbers.clone().min(), numbers.max()) else {
        return;
    };
    let wanted = midnight_at(first - WIDEN_STEP_DAYS)
        .zip(midnight_at(last + 1 + WIDEN_STEP_DAYS))
        .and_then(|(start, end)| Horizon::new(start, end).ok());
    let Some(wanted) = wanted else {
        return;
    };
    cache.visited = Some(span(reach, wanted));
    cache.widening_owed = true;
}

/// The one window running from the earlier start of `a` and `b` to the later end.
fn span(a: Horizon, b: Horizon) -> Horizon {
    let start = if b.start() < a.start() {
        b.start()
    } else {
        a.start()
    };
    let end = if b.end() > a.end() { b.end() } else { a.end() };
    Horizon::new(start, end).unwrap_or(a)
}
