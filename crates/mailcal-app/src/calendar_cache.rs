//! The in-memory calendar cache, and the page query a grid pages over.
//!
//! # Why the grid is a pull with an argument, not a pushed snapshot
//!
//! Every other surface follows the mail pattern: the app mutates state, signals
//! [`Surface::Calendar`], and the host pulls one immutable snapshot from a `Mutex` slot.
//! That breaks under a pager.
//!
//! A calendar is swiped through continuously, and the host renders three pages at once
//! (the week either side of the one in view, so the next swipe is instant). One slot cannot
//! hold three. Worse, `dispatch` is fire-and-forget on a multi-threaded runtime, so two
//! quick swipes race and the snapshots can land out of order: the grid settles on *last*
//! week after the user has already swiped to next. And the observer is debounced at 250 ms
//! to stop list flicker, which would either blank the grid on every swipe or reintroduce
//! the flicker it was added to fix.
//!
//! So the grid is a **direct query**: [`App::calendar_page`] takes the page you want and
//! returns it, synchronously, from this cache. The pager owns the anchor, the core never
//! needs to know where the user is. A pull cannot arrive out of order, no observer sits in
//! the loop, and prefetching the neighbouring pages is three calls.
//!
//! [`Surface::Calendar`] stays, demoted to a **cache-invalidation signal**: "calendar data
//! changed; re-pull whatever you are showing."
//!
//! The query therefore must never touch the store or the network. It reads this cache and
//! nothing else.

use std::{
    collections::{HashMap, HashSet},
    time::{Instant, SystemTime, UNIX_EPOCH},
};

use engine_api::{CalendarDate, Event, Horizon, Provider, ProviderKey, UtcDateTime};
use mailcal_viewmodel::{
    ResponseStatus,
    calendar::{
        CalendarRow,
        days::{date_at, days_from, week_start},
        grid::{self, Occurrence, TimeGrid},
        month::{self, MonthGrid},
    },
};

use crate::App;

/// How far back the rolling horizon materializes: enough that last quarter is browsable
/// without a re-expansion, without carrying years of dead occurrences in memory.
const HORIZON_DAYS_BACK: i64 = 120;

/// How far forward. A year covers the planning a user actually does; paging past it widens
/// the horizon rather than silently showing an empty grid.
const HORIZON_DAYS_AHEAD: i64 = 400;

/// One calendar as the **server** describes it. What the *user* decided about it; shown/hidden, a
/// colour override, is applied at read time from [`crate::calendar_prefs`], not baked in here, so
/// toggling a calendar off redraws the grid with no re-sync.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct CachedCalendar {
    /// The owning account's id.
    pub(crate) account: String,
    /// The calendar's provider key, unique within its account.
    pub(crate) id: String,
    /// The display name.
    pub(crate) name: String,
    /// The colour the server sent, if any; snapped to the nearest palette entry on the way out.
    /// When absent, `resolved_calendars` assigns a palette hue no other calendar is already using,
    /// so two colourless calendars never come out the same.
    pub(crate) server_color: Option<String>,
    /// Whether this account's calendar provider supports writes.
    pub(crate) can_write: bool,
}

/// The occurrences and calendars a grid renders from, and the window they cover.
#[derive(Debug, Clone, Default)]
pub(crate) struct CalendarCache {
    /// Every materialized occurrence across every account, joined to its master event.
    pub(crate) occurrences: Vec<Occurrence>,
    /// Every calendar across every account, as the server describes it.
    pub(crate) calendars: Vec<CachedCalendar>,
    /// The window the occurrences cover, or `None` before the first sync. A page outside it
    /// is **not** empty; it is *unknown*, and the two must not look the same.
    pub(crate) window: Option<Horizon>,
}

/// One rendered page of the calendar.
#[derive(Debug, Clone)]
pub struct CalendarPage {
    /// The laid-out grid.
    pub grid: TimeGrid,
    /// The calendars, for the manager and the colour key.
    pub calendars: Vec<CalendarRow>,
    /// Whether every day on this page falls inside the materialized window.
    ///
    /// **`false` does not mean "no events".** It means the engine has not expanded this far
    /// yet, so the page is unknown. A host must say so ("loading…") rather than render a
    /// confidently empty week, which is a lie that looks exactly like a real answer.
    pub is_materialized: bool,
}

/// One rendered page of the month grid.
#[derive(Debug, Clone)]
pub struct MonthPage {
    /// The month, laid out: six weeks of day cells.
    pub grid: MonthGrid,
    /// The calendars, for the manager and the colour key.
    pub calendars: Vec<CalendarRow>,
    /// Whether every day on this page falls inside the materialized window.
    ///
    /// **`false` does not mean "no events"**; see [`CalendarPage::is_materialized`].
    pub is_materialized: bool,
}

impl<P: Provider> App<P> {
    /// The month containing `anchor`, laid out.
    ///
    /// A different layout from the time grid, not the same one with more columns: a month cell has
    /// no hour axis and no overlap solving, only a list. Same cache, same query discipline;
    /// synchronous, in-memory, never the store or the network.
    #[must_use]
    pub fn month_page(&self, anchor: CalendarDate) -> MonthPage {
        let cache = self.calendar_cache.lock().expect("calendar cache poisoned");
        let calendars = self.resolved_calendars(&cache);
        let visible = filter_visible(&cache.occurrences, &calendars);
        let grid = month::build(
            anchor,
            self.display_settings().week_start.starts_monday(),
            visible,
            &self.active_zone(),
        );
        let days: Vec<CalendarDate> = grid
            .cells
            .iter()
            .filter_map(|cell| cell.date.parse().ok())
            .collect();
        MonthPage {
            is_materialized: covers(cache.window, &days),
            grid,
            calendars,
        }
    }

    /// The page a grid renders: the days `view` shows around `anchor`, laid out.
    ///
    /// A **direct, synchronous query**: no intent, no observer, no snapshot slot. It reads
    /// the in-memory cache and never the store or the network, so a host may call it freely
    /// while paging and prefetch its neighbours by calling it again.
    ///
    /// The days run **consecutively from `from`** and are not snapped to anything. That is what
    /// lets a client zoom the day axis without the grid relocating: widening three columns to
    /// seven keeps the same first day, so the days the user was reading stay where they were. A
    /// Monday-aligned week cannot do that; it must relocate to its own Monday, which is a
    /// jump.
    ///
    /// Alignment is a separate, deliberate act: [`App::week_start_date`], applied when the user
    /// picks "Week" from a menu rather than every time the column count changes.
    #[must_use]
    pub fn calendar_range(&self, from: CalendarDate, columns: u32) -> CalendarPage {
        let days = days_from(from, columns);
        let cache = self.calendar_cache.lock().expect("calendar cache poisoned");
        let calendars = self.resolved_calendars(&cache);
        let visible = filter_visible(&cache.occurrences, &calendars);
        CalendarPage {
            grid: grid::build(&days, visible, &self.active_zone()),
            calendars,
            is_materialized: covers(cache.window, &days),
        }
    }

    /// The first day of the week containing `date`, per the persisted week-start setting.
    ///
    /// The core owns the rule so three clients cannot disagree about which day a week begins on;
    /// but it is applied only when a client asks, because aligning on every zoom is exactly the
    /// jump this API exists to avoid.
    #[must_use]
    pub fn week_start_date(&self, date: CalendarDate) -> CalendarDate {
        week_start(date, self.display_settings().week_start.starts_monday())
    }

    /// Rebuilds the cache from the store: every account's calendars, and every occurrence in
    /// the rolling window joined to the master event that carries its content.
    ///
    /// The occurrences say *when*; the events say *what*. A host reading `events()` alone
    /// sees a recurring series exactly once, at its series start.
    /// Returns whether the cache actually **changed**; see the comment at the store, below. The
    /// caller uses it to decide whether to signal the host, and a signal is a synchronous re-pull
    /// of every page on screen.
    pub(super) async fn rebuild_calendar_cache(&self, window: Horizon) -> bool {
        let started = Instant::now();
        // The zone the horizon was expanded in; needed only to recover a **floating** recurring
        // event's occurrence wall clock, which the engine resolved through this same zone.
        let zone = self.active_zone();
        let mut occurrences = Vec::new();
        let mut calendars = Vec::new();
        // Read by the window claim below: whether a calendar could ever arrive, and whether every
        // account has been dialed yet. A boot placeholder has no providers of any kind
        // (`boot/stored.rs`), so "no calendar provider" alone cannot tell a mail-only account from
        // one nobody has connected to.
        let mut expects_calendars = false;
        let mut every_account_connected = true;
        for account in self.account_handles().await {
            let account_id = account.id.as_str().to_owned();
            expects_calendars |= !account.calendar_providers.is_empty();
            every_account_connected &=
                !account.providers.is_empty() || !account.calendar_providers.is_empty();
            let can_write = account.calendar_providers.first().is_some_and(|provider| {
                provider
                    .connection_info()
                    .capabilities
                    .calendar_write_guard()
                    .is_some()
            });
            for calendar in self.engine.calendars(&account.id).await.unwrap_or_default() {
                calendars.push(CachedCalendar {
                    account: account_id.clone(),
                    id: calendar.id.key().as_str().to_owned(),
                    name: calendar.name,
                    server_color: calendar.color,
                    can_write,
                });
            }
            // The account's own addresses; primary plus configured aliases. The *persisted* set
            // only: a grid has no message to read delivery headers from, so the zero-configuration
            // alias source the reading view enjoys is unavailable here (`docs/invitations.md`).
            let addresses = self.account_address_set(&account.id).await;
            // The occurrences in the window first; cheap index rows, no event payloads.
            let occurrence_rows = self
                .engine
                .occurrences_in(&account.id, window)
                .await
                .unwrap_or_default();
            // Decode **only** the masters those occurrences reference, not the account's whole
            // event history. The join needs the ~hundreds of masters live in this window; a real
            // diary holds ~10,000 events, and `events()` would deserialize every one of them;
            // this was the multi-second `rebuild_calendar_cache` on the boot/refresh path.
            let wanted: Vec<ProviderKey> = occurrence_rows
                .iter()
                .map(|row| row.event.clone())
                .collect::<HashSet<_>>()
                .into_iter()
                .collect();
            let events = self
                .engine
                .events_by_keys(&account.id, &wanted)
                .await
                .unwrap_or_default();
            // Index the masters before the join. It is a **join**, and doing it by scanning the
            // event list once per occurrence is quadratic: a real calendar has ~1,000 occurrences
            // in the rolling window over ~1,000 masters, so the scan cost a million
            // string comparisons and very nearly a second; on the boot path, in front
            // of the mail list.
            let by_key: HashMap<&str, &Event> = events
                .iter()
                .map(|event| (event.id.key().as_str(), event))
                .collect();

            for row in occurrence_rows {
                let key = row.event.as_str();
                let Some(event) = by_key.get(key).copied() else {
                    // An occurrence whose master is gone: a torn read between the two calls.
                    // Skipping it is right; rendering a block with no title would be worse.
                    continue;
                };
                // How this account answered, against its whole address set (primary + aliases),
                // so an invitation that arrived at an alias is still recognised as ours.
                let participation = crate::invitations::diary_participation(event, &addresses);
                // **Declined events are hidden from every calendar surface**; one rule, applied
                // here at the single join point, so the grid, the month and the agenda cannot
                // disagree. Provider behaviour is not uniform (Exchange removes the event,
                // Google keeps it, CalDAV and JMAP keep the object), which is exactly why this
                // belongs in the core rather than being inherited four different ways.
                //
                // This *hides* data, so it says so: the event still exists, it is still returned
                // by search, and the invitation email remains the way back; its card shows the
                // current answer and can change it. `docs/calendar.md` §4 forbids hiding
                // anything silently; `docs/invitations.md` records the reversal path.
                if participation == ResponseStatus::Declined {
                    continue;
                }
                // Whether the block may be *dragged*, which is a strictly narrower question than
                // whether its calendar may be written: a meeting somebody else called can sit on
                // a writable calendar and still must not be silently re-timed.
                let can_move =
                    can_write && crate::invitations::owns_or_organizes(event, &addresses);
                // The token that names *this* occurrence to a write, minted only for a series;
                // a client hands it straight back after asking the user whether they meant one
                // occurrence or all of them. Empty on a one-off, which is also how a client
                // knows not to ask.
                //
                // Minted from the **recurrence id**, not from where the block sits. The two are
                // the same instant until somebody moves an occurrence, and then they are not:
                // an occurrence's identity is the slot in the series it came from, and that is
                // what a `RECURRENCE-ID` names. Naming the moved time would address no
                // occurrence at all: a second drag would leave the first override behind and
                // split another at a time the rule never produces.
                let occurrence_start = event
                    .recurrence
                    .as_ref()
                    .and_then(|_| {
                        mailcal_account::occurrence_wall_clock(
                            event,
                            row.recurrence_id.unwrap_or(row.start),
                            &zone,
                        )
                    })
                    .unwrap_or_default();
                occurrences.push(Occurrence {
                    account: account_id.clone(),
                    event: key.to_owned(),
                    calendar: event
                        .calendars
                        .iter()
                        .next()
                        .map(|id| id.key().as_str().to_owned())
                        .unwrap_or_default(),
                    title: if event.title.is_empty() {
                        "(no title)".to_owned()
                    } else {
                        event.title.clone()
                    },
                    start: row.start,
                    end: row.end,
                    all_day: event.is_all_day(),
                    can_write,
                    can_move,
                    occurrence_start,
                    participation,
                });
            }
        }

        let mut cache = self.calendar_cache.lock().expect("calendar cache poisoned");
        // Did anything actually change? In the steady state, no: a refresh syncs calendars the
        // provider reports no changes to, and rebuilds a cache identical to the one it replaces.
        //
        // It matters because the **signal** that follows is not free. `Surface::Calendar`
        // invalidates every page the grid is showing, and the host re-pulls all three of them;
        // synchronously, on the UI thread, because the page query is a direct call by design. Fire
        // that while the user is mid-swipe and the fling stalls part-way through, which is
        // indistinguishable from the page getting stuck between two weeks. **A refresh that changed
        // nothing must invalidate nothing.**
        // The window is what `is_materialized` is derived from, so claiming it is a claim to have
        // **looked**, and it is claimed only on evidence: a calendar in the store, or an account
        // set that is fully connected and has no calendar in it. Claiming it over a store that
        // holds no calendars draws a confidently empty week, the one lie `docs/calendar.md` §4
        // forbids, and this rebuild runs whether or not the sync in front of it reached anything,
        // so a launch with no network would otherwise draw that week on every launch.
        //
        // The second clause is the opposite lie, and it needs guarding just as much: an account
        // that connected and reported no calendar will never produce one, so withholding the
        // window there leaves "loading this period…" on screen for the life of the account.
        let known = !calendars.is_empty() || (!expects_calendars && every_account_connected);
        let claimed = known.then_some(window);
        let changed = cache.window != claimed
            || cache.occurrences != occurrences
            || cache.calendars != calendars;

        // Counts, a duration, and whether it moved; never a title, an attendee or an address. The
        // grid reads from this cache and nothing else, so this is what tells a slow *query* from a
        // slow *sync*, and a real change from a pointless redraw.
        log::info!(
            "rebuild_calendar_cache: {} occurrence(s), {} calendar(s) in {}ms{}",
            occurrences.len(),
            calendars.len(),
            started.elapsed().as_millis(),
            if changed { "" } else { "; unchanged" }
        );

        cache.occurrences = occurrences;
        cache.calendars = calendars;
        cache.window = claimed;
        changed
    }
}

/// The rolling window the calendar materializes: a few months back, a bit over a year on.
///
/// Relative to today, never a fixed year. A hard-coded window silently empties the moment
/// the calendar leaves it, and it always does, eventually.
pub(crate) fn rolling_horizon() -> Option<Horizon> {
    let today = today_utc()?;
    let start = midnight_at(today - HORIZON_DAYS_BACK)?;
    let end = midnight_at(today + HORIZON_DAYS_AHEAD)?;
    Horizon::new(start, end).ok()
}

/// The occurrences belonging to a calendar the user has left visible.
///
/// Matched on account **and** calendar: a calendar id is unique only within its account, so
/// matching on the id alone would hide one account's events when the user hid the other account's
/// calendar of the same name, and only for the users unlucky enough to have two.
///
/// **Borrows.** It used to `.cloned()`, and that was a frame-rate bug rather than a style one: this
/// runs on *every* page pull, a pull happens per page composed, and the pager composes three pages
/// per swipe. Over a real calendar (~1,100 occurrences in the rolling window, each carrying several
/// heap-allocated strings) that was thousands of allocations landing in the middle of the fling;
/// on the UI thread, because the page query is synchronous by design. The swipe stalled part-way
/// through, which reads exactly like the page getting stuck between two weeks.
fn filter_visible<'a>(
    occurrences: &'a [Occurrence],
    calendars: &[CalendarRow],
) -> Vec<&'a Occurrence> {
    occurrences
        .iter()
        .filter(|occurrence| {
            calendars.iter().any(|calendar| {
                calendar.visible
                    && calendar.account == occurrence.account
                    && calendar.id == occurrence.calendar
            })
        })
        .collect()
}

/// Whether the materialized window covers every day on a page.
pub(crate) fn covers(window: Option<Horizon>, days: &[CalendarDate]) -> bool {
    let Some(window) = window else {
        return false;
    };
    days.iter().all(|&day| {
        midnight_at(mailcal_viewmodel::calendar::days::day_number(day)).is_some_and(|start| {
            // The day is covered when it opens inside the window and its own end does too.
            start >= window.start() && start < window.end()
        })
    })
}

/// UTC midnight at a civil day number.
fn midnight_at(day: i64) -> Option<UtcDateTime> {
    let date = date_at(day);
    UtcDateTime::new(date.year(), date.month(), date.day(), 0, 0, 0).ok()
}

/// Today's civil day number, in UTC.
fn today_utc() -> Option<i64> {
    let seconds = SystemTime::now().duration_since(UNIX_EPOCH).ok()?.as_secs();
    i64::try_from(seconds / 86_400).ok()
}
