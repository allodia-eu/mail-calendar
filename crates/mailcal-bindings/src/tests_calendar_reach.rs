//! A page outside the rolling horizon, asked for exactly as a client asks for it, loads rather
//! than saying "Loading this period…" for good (`docs/calendar.md` §4).

use std::{sync::mpsc, time::Duration};

use super::{ZONE, today};
use crate::{
    LogLevel, MailcalApp, ShowcaseLocale, Surface,
    tests::{ChannelObserver, NullLogger},
};

/// The `YYYY-MM-DD` `offset` days from the showcase's today.
fn day_from_today(offset: i64) -> String {
    let today: engine_api::CalendarDate = today().parse().expect("a date");
    let days = mailcal_viewmodel::calendar::days::day_number(today) + offset;
    mailcal_viewmodel::calendar::days::date_at(days).to_string()
}

/// Waits for the next `Surface::Calendar`, the signal a client re-pulls its pages on.
fn calendar_signalled(rx: &mpsc::Receiver<Surface>) -> bool {
    while let Ok(surface) = rx.recv_timeout(Duration::from_secs(5)) {
        if matches!(surface, Surface::Calendar) {
            return true;
        }
    }
    false
}

/// Pulls again on every `Surface::Calendar`, as a client does, until `covered` answers true.
/// False when the signals stop first.
fn covered_after_signals(rx: &mpsc::Receiver<Surface>, covered: impl Fn() -> bool) -> bool {
    while calendar_signalled(rx) {
        if covered() {
            return true;
        }
    }
    false
}

#[test]
fn a_week_and_a_month_outside_the_horizon_load_after_the_client_pulls_them() {
    let (tx, rx) = mpsc::channel();
    let app = MailcalApp::new_showcase(
        Box::new(ChannelObserver { tx }),
        Box::new(NullLogger),
        LogLevel::Info,
        ZONE.to_owned(),
        ShowcaseLocale::En,
    );
    app.dispatch(crate::Intent::RefreshCalendar);
    assert!(
        calendar_signalled(&rx),
        "the refresh signalled the calendar"
    );

    let back = app.week_start_date(day_from_today(-200));
    assert!(
        !app.calendar_range(back.clone(), 7).is_materialized,
        "200 days back has not been expanded yet"
    );
    assert!(
        covered_after_signals(&rx, || app.calendar_range(back.clone(), 7).is_materialized),
        "the same week is covered once the client pulls it again"
    );

    let ahead = day_from_today(500);
    assert!(!app.month_page(ahead.clone()).is_materialized);
    assert!(covered_after_signals(&rx, || app
        .month_page(ahead.clone())
        .is_materialized));
}
