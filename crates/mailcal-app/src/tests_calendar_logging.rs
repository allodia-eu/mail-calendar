//! What a calendar refresh writes to the diagnostic log when a source fails.
//!
//! A refresh is a *sync* pass, so a failed source is counted and logged rather than surfaced:
//! the calendars that did sync are still worth showing. That makes the log line the only trace
//! of the failure, and a refresh that dropped it left an account whose every calendar sync
//! failed looking exactly like an account with an empty calendar.

use std::sync::{Arc, Mutex};

use fakes::{CalendarFake, calendar_account, calendar_app};

use super::Intent;

#[allow(clippy::duplicate_mod)]
#[path = "tests_fakes.rs"]
mod fakes;

fn lines() -> Vec<String> {
    crate::tests_log_capture::lines_from(&["mailcal_app::calendar_refresh"])
}

fn assert_logged(needle: &str) {
    let lines = lines();
    assert!(
        lines.iter().any(|line| line.contains(needle)),
        "no calendar refresh line contains {needle:?}; captured:\n{}",
        lines.join("\n"),
    );
}

#[tokio::test]
async fn a_calendar_source_that_fails_is_logged_with_its_reason() {
    crate::tests_log_capture::install();
    let surfaces = Arc::new(Mutex::new(Vec::new()));
    let app = calendar_app(
        vec![
            calendar_account("reached", CalendarFake::with_events(Vec::new())),
            calendar_account("unreached", CalendarFake::unreachable()),
        ],
        &surfaces,
    );

    app.dispatch(Intent::RefreshCalendar).await;

    assert_logged("calendar[a1]: source[0] failed in ");
    assert_logged("the server could not be reached");
    assert_logged("refresh_calendar: 2 source(s) on 2 account(s); 1 synced, 0 busy, 1 failed;");
}
