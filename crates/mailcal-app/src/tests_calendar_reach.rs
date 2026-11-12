//! A page outside the rolling horizon widens the calendar to it (`docs/calendar.md` §4, §5).
//!
//! The page query never touches the store, so it answers `is_materialized: false` first and owes
//! a widening; once that has run, the same page is covered and carries its events.

use std::sync::{Arc, Mutex};

use mailcal_viewmodel::calendar::days::{date_at, day_number};

use super::{
    fakes::{CalendarFake, calendar_account, calendar_app, event_from_today, evt},
    today,
};
use crate::{EventIntent, Intent, Surface};

/// The week holding the day `offset` days from today, as a client asks for it.
fn week_at<P: engine_api::Provider>(app: &crate::App<P>, offset: i64) -> engine_api::CalendarDate {
    app.week_start_date(date_at(day_number(today()) + offset))
}

#[tokio::test]
async fn a_page_beyond_the_horizon_loads_once_the_window_widens_to_it() {
    let surfaces = Arc::new(Mutex::new(Vec::new()));
    let provider = CalendarFake::with_events(vec![
        event_from_today("past", -200, 9, 60),
        event_from_today("future", 500, 9, 60),
        event_from_today("now", 0, 9, 60),
    ]);
    let app = calendar_app(vec![calendar_account("acct", provider)], &surfaces);
    app.dispatch(Intent::RefreshCalendar).await;

    for (offset, key) in [(-200, "past"), (500, "future")] {
        let page = app.calendar_range(week_at(&app, offset), 7);
        assert!(
            !page.is_materialized,
            "{offset} days out has not been expanded yet, so it is unknown"
        );
        assert_eq!(page.grid.timed, []);
        assert!(app.take_calendar_widening(), "the page owes a widening");
        assert!(
            !app.take_calendar_widening(),
            "one widening per page asked for, not one per pull"
        );

        surfaces.lock().unwrap().clear();
        app.widen_calendar().await;
        assert!(
            surfaces.lock().unwrap().contains(&Surface::Calendar),
            "the client is told to pull again"
        );
        let page = app.calendar_range(week_at(&app, offset), 7);
        assert!(page.is_materialized, "{offset} days out is covered now");
        assert_eq!(
            page.grid
                .timed
                .iter()
                .map(|block| block.event.as_str())
                .collect::<Vec<_>>(),
            [key],
            "and draws the event stored there"
        );
    }

    // Widening reaches towards the page; it does not move the window away from today.
    let page = app.calendar_range(week_at(&app, 0), 7);
    assert!(page.is_materialized);
    assert_eq!(page.grid.timed.len(), 1);
    // The agenda is the windowed set the grid draws, so it widens with it.
    let mut keys: Vec<String> = app
        .calendar_list()
        .events
        .into_iter()
        .map(|row| row.key)
        .collect();
    keys.sort();
    assert_eq!(keys, ["future", "now", "past"]);
}

/// The widened window outlives the next refresh and the next write: each expands or rebuilds over
/// the window as widened, never the rolling horizon alone, so a page already loaded does not go
/// back to "loading".
#[tokio::test]
async fn a_widened_page_stays_covered_through_a_refresh_and_a_write() {
    let surfaces = Arc::new(Mutex::new(Vec::new()));
    let provider = CalendarFake::with_events(vec![
        event_from_today("past", -200, 9, 60),
        event_from_today("now", 0, 9, 60),
    ]);
    let app = calendar_app(vec![calendar_account("acct", provider)], &surfaces);
    app.dispatch(Intent::RefreshCalendar).await;
    let _ = app.calendar_range(week_at(&app, -200), 7);
    assert!(app.take_calendar_widening());
    app.widen_calendar().await;

    app.refresh_calendar().await;
    let page = app.calendar_range(week_at(&app, -200), 7);
    assert!(page.is_materialized, "a refresh keeps the widened window");
    assert_eq!(page.grid.timed.len(), 1);

    app.dispatch(Intent::Events(EventIntent::Delete {
        event: evt("acct", "now"),
        occurrence: None,
    }))
    .await;
    let page = app.calendar_range(week_at(&app, -200), 7);
    assert!(page.is_materialized, "a write's rebuild keeps it too");
    assert_eq!(page.grid.timed.len(), 1);
    assert!(
        !app.take_calendar_widening(),
        "and nothing asks to widen it again"
    );
}

/// A page next to one already widened to is inside the step the widening took, so paging on a
/// week at a time does not re-expand the calendar for every week.
#[tokio::test]
async fn paging_on_from_a_widened_page_does_not_widen_again() {
    let surfaces = Arc::new(Mutex::new(Vec::new()));
    let provider = CalendarFake::with_events(vec![event_from_today("now", 0, 9, 60)]);
    let app = calendar_app(vec![calendar_account("acct", provider)], &surfaces);
    app.dispatch(Intent::RefreshCalendar).await;
    let _ = app.calendar_range(week_at(&app, -200), 7);
    assert!(app.take_calendar_widening());
    app.widen_calendar().await;

    for weeks_back in 1..=8 {
        let page = app.calendar_range(week_at(&app, -200 - 7 * weeks_back), 7);
        assert!(page.is_materialized);
    }
    assert!(!app.take_calendar_widening());
    let month = app.month_page(date_at(day_number(today()) - 230));
    assert!(month.is_materialized);
    assert!(!app.take_calendar_widening());
}

/// Widening a store nobody has read claims nothing: the window is claimed on evidence, and an
/// unreachable calendar is none, however far the window reaches (`docs/calendar.md` §4).
#[tokio::test]
async fn widening_an_unsynced_store_still_says_loading() {
    let surfaces = Arc::new(Mutex::new(Vec::new()));
    let app = calendar_app(
        vec![calendar_account("acct", CalendarFake::unreachable())],
        &surfaces,
    );
    app.refresh_calendar().await;

    let page = app.calendar_range(week_at(&app, -200), 7);
    assert!(!page.is_materialized);
    assert!(app.take_calendar_widening());
    app.widen_calendar().await;

    let page = app.calendar_range(week_at(&app, -200), 7);
    assert!(
        !page.is_materialized,
        "a calendar nobody could read is unknown, not empty, however far it is widened"
    );
}
