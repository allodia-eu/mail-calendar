//! The calendar pane through the app: the grouping in the account order, a shut group that
//! survives a relaunch, and where a mini-month press seats the grid.

use std::{
    sync::{Arc, Mutex},
    time::Duration,
};

use engine_api::CalendarDate;
use mailcal_account::CalendarLayout;
use mailcal_viewmodel::calendar::pane::ShownSpan;

use super::{CalendarPaneQuery, CalendarSeat};
use crate::{
    Intent, Surface,
    preferences_file::ScratchPreferences,
    tests_fakes::{CalendarFake, calendar_account, calendar_app, calendar_app_with_prefs},
};

fn date(raw: &str) -> CalendarDate {
    raw.parse().expect("a valid date")
}

/// Two accounts whose ids sort the other way round from the order they were added in.
fn accounts() -> Vec<crate::Account<CalendarFake>> {
    vec![
        calendar_account(
            "zulu",
            CalendarFake::on_dav(&[("home", "h-1"), ("family", "f-1")], Vec::new()),
        ),
        calendar_account(
            "alpha",
            CalendarFake::on_dav(&[("work", "w-1")], Vec::new()),
        ),
    ]
}

#[tokio::test]
async fn groups_follow_the_account_order_and_a_shut_group_signals_the_calendar() {
    let surfaces = Arc::new(Mutex::new(Vec::new()));
    let app = calendar_app(accounts(), &surfaces);
    app.dispatch(Intent::RefreshCalendar).await;

    let groups = app.calendar_groups();
    let accounts: Vec<&str> = groups.iter().map(|group| group.account.as_str()).collect();
    assert_eq!(accounts, ["zulu", "alpha"]);
    assert_eq!(groups[0].address, "me@zulu.local");
    assert_eq!(groups[0].calendars.len(), 2);
    assert!(groups.iter().all(|group| group.expanded));

    surfaces.lock().unwrap().clear();
    app.set_calendar_group_expanded("zulu", false);
    assert_eq!(surfaces.lock().unwrap().as_slice(), [Surface::Calendar]);
    assert!(!app.calendar_groups()[0].expanded);

    // Saying it again changes nothing, so it signals nothing.
    surfaces.lock().unwrap().clear();
    app.set_calendar_group_expanded("zulu", false);
    assert!(surfaces.lock().unwrap().is_empty());
}

#[tokio::test]
async fn a_shut_group_survives_a_relaunch() {
    let scratch = ScratchPreferences::new();
    let surfaces = Arc::new(Mutex::new(Vec::new()));
    let first = calendar_app_with_prefs(accounts(), scratch.path(), &surfaces);
    first.set_calendar_group_expanded("alpha", false);
    assert!(first.settle_preferences_within(Duration::from_secs(5)));
    drop(first);

    let second = calendar_app_with_prefs(accounts(), scratch.path(), &surfaces);
    second.dispatch(Intent::RefreshCalendar).await;
    let groups = second.calendar_groups();
    assert!(groups[0].expanded, "zulu was never shut");
    assert!(!groups[1].expanded, "alpha stays shut after the relaunch");
}

#[tokio::test]
async fn the_pane_reads_the_layout_it_is_given() {
    let surfaces = Arc::new(Mutex::new(Vec::new()));
    let app = calendar_app(accounts(), &surfaces);
    let query = CalendarPaneQuery {
        today: date("2026-11-18"),
        layout: CalendarLayout::Week,
        anchor: date("2026-11-16"),
        columns: 7,
        month: None,
    };

    let week = app.calendar_pane(query);
    assert_eq!(week.month.first, "2026-11-01");
    let shown = week
        .month
        .weeks
        .iter()
        .flat_map(|row| &row.days)
        .filter(|day| day.shown != ShownSpan::Outside)
        .count();
    assert_eq!(shown, 7);

    // The month layout follows its anchor and marks no days.
    let month = app.calendar_pane(CalendarPaneQuery {
        layout: CalendarLayout::Month,
        anchor: date("2027-02-10"),
        ..query
    });
    assert_eq!(month.month.first, "2027-02-01");
    assert!(
        month
            .month
            .weeks
            .iter()
            .flat_map(|row| &row.days)
            .all(|day| day.shown == ShownSpan::Outside)
    );
}

#[tokio::test]
async fn a_pressed_day_seats_each_layout() {
    let surfaces = Arc::new(Mutex::new(Vec::new()));
    let app = calendar_app(accounts(), &surfaces);
    // Wednesday 18 November 2026; the default week starts on Monday.
    let day = date("2026-11-18");
    let seat = |layout| app.calendar_seat(day, layout);

    for layout in [CalendarLayout::Week, CalendarLayout::WorkWeek] {
        assert_eq!(
            seat(layout),
            CalendarSeat {
                layout,
                anchor: date("2026-11-16"),
            }
        );
    }
    for layout in [
        CalendarLayout::Day,
        CalendarLayout::ThreeDay,
        CalendarLayout::Month,
    ] {
        assert_eq!(
            seat(layout),
            CalendarSeat {
                layout,
                anchor: day
            }
        );
    }
    assert_eq!(
        seat(CalendarLayout::Agenda),
        CalendarSeat {
            layout: CalendarLayout::Day,
            anchor: day,
        }
    );
}
