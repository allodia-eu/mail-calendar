//! The calendar grid, end to end through the app: sync → occurrences → cache → laid-out page.

use std::sync::{Arc, Mutex, atomic::Ordering};

use fakes::{CalendarFake, calendar_account, calendar_app, calendar_app_on, event_from_today, evt};
use mailcal_account::WeekStart;
use mailcal_viewmodel::calendar::days::{date_at, day_number, weekday};

use crate::{App, Intent, Surface};

#[allow(clippy::duplicate_mod)]
#[path = "tests_fakes.rs"]
mod fakes;

/// Today's date, in UTC: the anchor the fixtures are built around.
fn today() -> engine_api::CalendarDate {
    date_at(
        i64::try_from(
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("after the epoch")
                .as_secs()
                / 86_400,
        )
        .expect("in range"),
    )
}

/// A refresh materializes the occurrences, joins them to their masters, and the page query
/// lays them out: the whole read path, without a client.
#[tokio::test]
async fn a_refresh_materializes_occurrences_that_the_page_query_lays_out() {
    let surfaces = Arc::new(Mutex::new(Vec::new()));
    // Two clashing meetings today, at 09:00 and 09:30 UTC.
    let provider = CalendarFake::with_events(vec![
        event_from_today("morning", 0, 9, 60),
        event_from_today("clash", 0, 9, 60),
    ]);
    let app = calendar_app(vec![calendar_account("acct", provider)], &surfaces);

    app.dispatch(Intent::RefreshCalendar).await;
    assert!(surfaces.lock().unwrap().contains(&Surface::Calendar));

    let page = app.calendar_range(app.week_start_date(today()), 7);
    assert!(
        page.is_materialized,
        "this week sits inside the rolling horizon"
    );
    assert_eq!(page.calendars.len(), 1);
    assert_eq!(page.calendars[0].name, "Calendar");
    // The calendar's colour resolved to a palette entry, ready for the client to paint with.
    assert!(page.calendars[0].color.hex.starts_with('#'));

    // Both meetings landed on today's column and split it, rather than drawing on top of
    // each other.
    assert_eq!(page.grid.timed.len(), 2);
    let column = u32::from(weekday(day_number(today())));
    for block in &page.grid.timed {
        assert_eq!(block.day, column);
        assert_eq!(block.start_minutes, 540);
        assert_eq!(block.columns, 2, "the two clashing meetings share the day");
    }
    assert_ne!(page.grid.timed[0].column, page.grid.timed[1].column);
}

/// `can_write` is sourced from the provider's write guard, not hardcoded: a writable
/// account's rows come back `true` and a read-only account's rows `false`, in the same page.
///
/// This is what lets a client hide the edit affordances on a subscribed feed. A hardcoded
/// `true` (the old `Calendar.access.may_write`) would pass every other calendar test while
/// quietly offering to edit a calendar the server will refuse: so the flag is only worth
/// anything if it can be observed to differ.
#[tokio::test]
async fn can_write_reflects_the_provider_write_guard_per_account() {
    let surfaces = Arc::new(Mutex::new(Vec::new()));
    let writable = CalendarFake::with_events(vec![event_from_today("editable", 0, 9, 60)]);
    let read_only = CalendarFake::read_only(vec![event_from_today("subscribed", 0, 11, 60)]);
    let app = calendar_app(
        vec![
            calendar_account("writer", writable),
            calendar_account("reader", read_only),
        ],
        &surfaces,
    );
    app.dispatch(Intent::RefreshCalendar).await;

    let page = app.calendar_range(app.week_start_date(today()), 7);

    // Both events materialized, and each segment's flag tracks its own account.
    let writer = page
        .grid
        .timed
        .iter()
        .find(|s| s.account == "writer")
        .expect("the writable account's event is on the grid");
    let reader = page
        .grid
        .timed
        .iter()
        .find(|s| s.account == "reader")
        .expect("the read-only account's event is on the grid");
    assert!(writer.can_write, "an Enforced-guard account can write");
    assert!(!reader.can_write, "a no-write-guard account cannot");

    // And the calendar rows carry the same per-account truth.
    let writer_cal = page
        .calendars
        .iter()
        .find(|c| c.account == "writer")
        .expect("writer calendar row");
    let reader_cal = page
        .calendars
        .iter()
        .find(|c| c.account == "reader")
        .expect("reader calendar row");
    assert!(writer_cal.can_write);
    assert!(!reader_cal.can_write);
}

/// A page outside the materialized window reports itself **unknown**, not empty.
///
/// This is the difference between "you have nothing on" and "we have not looked yet". A grid
/// that renders the second as the first tells the user a confident lie that looks exactly
/// like a real answer, and it is the failure mode a rolling horizon produces by default.
#[tokio::test]
async fn a_page_beyond_the_horizon_is_unknown_rather_than_confidently_empty() {
    let surfaces = Arc::new(Mutex::new(Vec::new()));
    let provider = CalendarFake::with_events(vec![event_from_today("soon", 0, 9, 60)]);
    let app = calendar_app(vec![calendar_account("acct", provider)], &surfaces);
    app.dispatch(Intent::RefreshCalendar).await;

    // Three years out is far past the rolling horizon.
    let far = date_at(day_number(today()) + 3 * 365);
    let page = app.calendar_range(app.week_start_date(far), 7);
    assert!(page.grid.timed.is_empty());
    assert!(
        !page.is_materialized,
        "the engine has not expanded this far, so the page is unknown: not empty"
    );

    // A week well inside the horizon *is* materialized, even with nothing on it, so the two
    // states are genuinely distinguishable.
    let next_week = date_at(day_number(today()) + 7);
    let page = app.calendar_range(app.week_start_date(next_week), 7);
    assert!(page.grid.timed.is_empty());
    assert!(page.is_materialized);
}

/// Before any sync, every page is unknown: the cache has no window at all.
#[tokio::test]
async fn before_the_first_sync_nothing_is_materialized() {
    let surfaces = Arc::new(Mutex::new(Vec::new()));
    let provider = CalendarFake::with_events(vec![event_from_today("soon", 0, 9, 60)]);
    let app = calendar_app(vec![calendar_account("acct", provider)], &surfaces);

    let page = app.calendar_range(app.week_start_date(today()), 7);
    assert!(!page.is_materialized);
    assert!(page.grid.timed.is_empty());
    assert!(page.calendars.is_empty());
}

/// The view picks the columns, and the same cache serves all of them; day, three-day,
/// work-week and week are one grid with a different column count, not four features.
#[tokio::test]
async fn every_time_view_reads_the_same_cache_with_a_different_column_count() {
    let surfaces = Arc::new(Mutex::new(Vec::new()));
    let provider = CalendarFake::with_events(vec![event_from_today("meeting", 0, 9, 60)]);
    let app = calendar_app(vec![calendar_account("acct", provider)], &surfaces);
    app.dispatch(Intent::RefreshCalendar).await;

    // Anchor on a Monday, so an aligned week is unambiguous whatever day the test runs.
    let monday = date_at(day_number(today()) - i64::from(weekday(day_number(today()))));
    let columns = |n| app.calendar_range(monday, n).grid.days.len();
    assert_eq!(columns(1), 1);
    assert_eq!(columns(3), 3);
    assert_eq!(columns(7), 7);
}

/// Widening the day axis keeps the SAME first day: the grid never relocates under a zoom.
///
/// This is why the query takes a column count rather than a named view. A Monday-aligned week
/// cannot contain an arbitrary three-day window, so snapping to one has to jump: a user reading
/// Sunday, Monday and Tuesday who pinched outwards would be shown the *previous* Monday-to-Sunday,
/// and two of the three days they were reading would vanish.
#[tokio::test]
async fn widening_the_day_axis_keeps_the_days_the_user_was_reading() {
    let surfaces = Arc::new(Mutex::new(Vec::new()));
    let provider = CalendarFake::with_events(vec![event_from_today("meeting", 0, 9, 60)]);
    let app = calendar_app(vec![calendar_account("acct", provider)], &surfaces);
    app.dispatch(Intent::RefreshCalendar).await;

    let three = app.calendar_range(today(), 3);
    let seven = app.calendar_range(today(), 7);
    let three_days: Vec<String> = three.grid.days.iter().map(|d| d.date.clone()).collect();
    let seven_days: Vec<String> = seven.grid.days.iter().map(|d| d.date.clone()).collect();

    assert_eq!(three_days[0], today().to_string());
    // The three days are still the first three of the seven, nothing moved, the view just grew.
    assert_eq!(three_days, seven_days[..3]);
}

/// The first-day-of-week **setting** reaches the grid: the core applies it, no client passes it.
///
/// This is the whole reason the setting lives here. A client that had to pass the flag could pass
/// the wrong one, and two clients could pass different ones; the failure is silent, because a week
/// starting on the wrong day still renders a perfectly plausible week; just with every column
/// shifted, so the user reads Tuesday's meetings under Monday's heading.
#[tokio::test]
async fn the_week_start_setting_moves_the_grids_first_column() {
    let surfaces = Arc::new(Mutex::new(Vec::new()));
    let provider = CalendarFake::with_events(vec![event_from_today("meeting", 0, 9, 60)]);
    let app = calendar_app(vec![calendar_account("acct", provider)], &surfaces);

    // The default is Monday; European, and not derived from any device locale.
    assert_eq!(app.display_settings().week_start, WeekStart::Monday);

    // Anchor on a Sunday: the day that belongs to a *different* week under each convention, and so
    // the only anchor that can tell the two apart.
    let sunday = date_at(day_number(today()) - i64::from(weekday(day_number(today()))) + 6);
    let first_of_week = |app: &App<_>| -> String {
        // Aligning is a deliberate act, and this is it: ask the core where the week begins.
        let page = app.calendar_range(app.week_start_date(sunday), 7);
        page.grid
            .days
            .first()
            .expect("a week has columns")
            .date
            .clone()
    };

    // Monday-start: that Sunday CLOSES its week, so the week runs back to the Monday six days ago.
    let monday_start = first_of_week(&app);
    assert_eq!(monday_start, date_at(day_number(sunday) - 6).to_string());

    app.set_week_start(WeekStart::Sunday).await;
    assert_eq!(app.display_settings().week_start, WeekStart::Sunday);

    // Sunday-start: that same Sunday now OPENS its week: the first column *is* the anchor.
    let sunday_start = first_of_week(&app);
    assert_eq!(sunday_start, sunday.to_string());
    assert_ne!(sunday_start, monday_start);

    // And the grid is told it went stale, not just the settings screen; otherwise the user would
    // be left staring at a week that still starts on the old day.
    let seen = surfaces.lock().expect("surfaces");
    assert!(seen.contains(&Surface::Calendar));
    assert!(seen.contains(&Surface::Settings));
}

/// The grid paints from the **store**, and opening it costs no round-trip.
///
/// The bug this pins: the calendar cache was built by nothing but [`Intent::RefreshCalendar`],
/// which syncs every calendar over the network *first*. So the cache was cold on every launch, and
/// opening the calendar meant seconds of "loading this period…" over a store that had held the week
/// all along. Mail had primed from the store since the beginning; the calendar never did.
///
/// It takes **two apps over one on-disk store** to see that. A single app that syncs and then
/// primes proves nothing: the sync already warmed the cache, so priming can be a no-op and the
/// test still passes. (It did, and it was.) The question is only ever about the *next* launch.
#[tokio::test]
async fn a_primed_grid_paints_from_the_store_without_touching_the_network() {
    let db = std::env::temp_dir().join("mailcal-prime-calendar.sqlite3");
    let _ = std::fs::remove_file(&db);
    let surfaces = Arc::new(Mutex::new(Vec::new()));

    // Launch one: the store fills from the network, and the app is dropped.
    {
        let provider = CalendarFake::with_events(vec![event_from_today("standup", 0, 9, 30)]);
        let app = calendar_app_on(
            engine_api::Engine::open(&db).expect("open store"),
            vec![calendar_account("acct", provider)],
            &surfaces,
        );
        app.dispatch(Intent::RefreshCalendar).await;
        assert_eq!(
            app.calendar_range(app.week_start_date(today()), 7)
                .grid
                .timed
                .len(),
            1,
            "the first launch synced the standup into the store"
        );
    }

    // Launch two: a NEW app over the SAME store. Boot primes (off its blocking path: the mail list
    // must not wait a few hundred milliseconds of SQLite for a surface the user has not opened),
    // and nothing else runs.
    let provider = CalendarFake::with_events(vec![event_from_today("standup", 0, 9, 30)]);
    let syncs = provider.syncs();
    let app = calendar_app_on(
        engine_api::Engine::open(&db).expect("reopen store"),
        vec![calendar_account("acct", provider)],
        &surfaces,
    );
    app.prime_calendar().await;

    let page = app.calendar_range(app.week_start_date(today()), 7);
    assert!(
        page.is_materialized,
        "the stored week must be ON SCREEN at boot, not 'loading this period…'"
    );
    assert_eq!(
        page.grid.timed.len(),
        1,
        "the stored occurrence was laid out"
    );
    assert_eq!(page.calendars.len(), 1);
    assert_eq!(
        syncs.load(Ordering::Relaxed),
        0,
        "painting the grid went to the network, that is the several-second wait, back again"
    );
    let _ = std::fs::remove_file(&db);
}

/// ...but a store that has never been synced must say so.
///
/// The trap in the fix above: priming sets the cache window, and `is_materialized` is derived from
/// it: so priming an **empty** store would flip it to `true` and show a first-run user a
/// confidently empty week. `false` means "we have not looked", and rendering that as "you have
/// nothing on" is the one lie `docs/calendar.md` exists to forbid: it looks exactly like a real
/// answer.
#[tokio::test]
async fn priming_a_never_synced_store_says_loading_rather_than_confidently_empty() {
    let surfaces = Arc::new(Mutex::new(Vec::new()));
    let provider = CalendarFake::with_events(vec![event_from_today("standup", 0, 9, 30)]);
    let app = calendar_app(vec![calendar_account("acct", provider)], &surfaces);

    // Boot, with nothing ever synced.
    app.prime_calendar().await;

    let page = app.calendar_range(app.week_start_date(today()), 7);
    assert!(
        !page.is_materialized,
        "an unsynced store must read as loading, never as an empty week"
    );
    assert!(page.grid.timed.is_empty());
}

/// A refresh that changed nothing must not tell the host anything changed.
///
/// `Surface::Calendar` is not a free signal. It invalidates every page the grid is showing, and the
/// host re-pulls all three of them **synchronously, on its UI thread**, that is the deal the pull
/// architecture makes. In the steady state a refresh syncs calendars the provider reports no
/// changes to and rebuilds a byte-identical cache, so firing the signal anyway buys a full
/// re-layout for nothing. Land that while the user is mid-swipe and the fling stalls part-way
/// through, which is indistinguishable from the page sticking between two weeks.
#[tokio::test]
async fn a_refresh_that_changed_nothing_does_not_redraw_the_grid() {
    let surfaces = Arc::new(Mutex::new(Vec::new()));
    let provider = CalendarFake::with_events(vec![event_from_today("standup", 0, 9, 30)]);
    let app = calendar_app(vec![calendar_account("acct", provider)], &surfaces);

    // The first refresh is a real one: it fills an empty cache, so the grid must redraw.
    app.dispatch(Intent::RefreshCalendar).await;
    assert!(
        surfaces.lock().unwrap().contains(&Surface::Calendar),
        "the first refresh brought the calendar in, that IS a change"
    );

    // The second changes nothing: same events, same window, same cache.
    surfaces.lock().unwrap().clear();
    app.dispatch(Intent::RefreshCalendar).await;
    assert!(
        !surfaces.lock().unwrap().contains(&Surface::Calendar),
        "a refresh that changed nothing invalidated the grid, that is a re-layout, on the UI \
         thread, for nothing; mid-swipe it stalls the fling"
    );

    // And the grid is still there. "Don't redraw" must never mean "lose the page".
    let page = app.calendar_range(app.week_start_date(today()), 7);
    assert_eq!(page.grid.timed.len(), 1);
    assert!(page.is_materialized);
}

/// The agenda lists the events in the materialized **window**, not every event the store holds.
///
/// A real diary is thousands of events, most of them long past or far future; the agenda used to
/// project *all* of them, which was the `events=9,888` a client rebuilt on its UI thread every
/// refresh (and read `engine.events()` (a full decode) to produce). It is now the same windowed
/// set the grid draws: an event with no occurrence in the rolling horizon does not appear.
#[tokio::test]
async fn the_agenda_lists_only_the_events_in_the_materialized_window() {
    let surfaces = Arc::new(Mutex::new(Vec::new()));
    // One event this week (inside the horizon) and one three years out (well past it).
    let provider = CalendarFake::with_events(vec![
        event_from_today("near", 0, 9, 60),
        event_from_today("far", 3 * 365, 9, 60),
    ]);
    let app = calendar_app(vec![calendar_account("acct", provider)], &surfaces);
    app.dispatch(Intent::RefreshCalendar).await;

    let keys: Vec<String> = app
        .calendar_list()
        .events
        .into_iter()
        .map(|row| row.key)
        .collect();
    assert_eq!(
        keys,
        vec!["near".to_owned()],
        "the agenda is the windowed set: the far-future master is stored but not agenda'd"
    );
}

/// The event-detail read is a **targeted** resolve by key: so it opens any stored event, even
/// one the grid has not materialized, and it returns the *named* event, never merely the first.
///
/// This pins the fix for the multi-second tap-to-open: `stored_event` used to decode the account's
/// entire event history and scan it. It now resolves the one key. The window-independence half
/// matters too: a client must be able to open an event the agenda/grid does not currently show
/// without the detail read quietly narrowing to the horizon.
#[tokio::test]
async fn event_detail_is_a_targeted_read_that_resolves_any_stored_event() {
    let surfaces = Arc::new(Mutex::new(Vec::new()));
    let mut near = event_from_today("standup", 0, 9, 60);
    near.title = "Standup".to_owned();
    // Deliberately outside the rolling horizon: it is stored, but never materialized.
    let mut far = event_from_today("review", 3 * 365, 14, 60);
    far.title = "Yearly review".to_owned();
    let app = calendar_app(
        vec![calendar_account(
            "acct",
            CalendarFake::with_events(vec![near, far]),
        )],
        &surfaces,
    );
    app.dispatch(Intent::RefreshCalendar).await;

    // The in-window event resolves to *its own* detail…
    let standup = app
        .event_detail(&evt("acct", "standup"), None)
        .await
        .expect("the named event resolves");
    assert_eq!(standup.title, "Standup");
    // …and so does the out-of-window one: the detail read is by key, not by horizon.
    let review = app
        .event_detail(&evt("acct", "review"), None)
        .await
        .expect("an event outside the window still opens");
    assert_eq!(review.title, "Yearly review");
    // A key that names nothing resolves to nothing (a stale reference, a torn read).
    assert!(
        app.event_detail(&evt("acct", "ghost"), None)
            .await
            .is_none()
    );
}
