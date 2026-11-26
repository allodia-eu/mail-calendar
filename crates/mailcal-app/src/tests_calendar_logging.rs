//! What a calendar refresh writes to the diagnostic log when a source fails.
//!
//! A refresh is a *sync* pass, so a failed source is counted and logged rather than surfaced:
//! the calendars that did sync are still worth showing. That makes the log line the only trace
//! of the failure, and a refresh that dropped it left an account whose every calendar sync
//! failed looking exactly like an account with an empty calendar.

use std::sync::{Arc, Mutex};

use fakes::{CalendarFake, calendar_account, calendar_app};

use super::{EventIntent, Intent};
use crate::tests_fakes as fakes;

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

    assert_logged("calendar[a1]: calendar list failed in ");
    assert_logged("the server could not be reached");
    assert_logged("refresh_calendar: 1 calendar(s); 1 synced, 0 unchanged, 0 busy, 1 failed;");
}

#[tokio::test]
async fn a_failed_reconcile_is_logged_by_its_class_not_the_servers_words() {
    // A server's error body can name the resource the write made, and on CalDAV that address
    // carries the event's `UID`; the log keeps the failure's class only.
    crate::tests_log_capture::install();
    let provider = CalendarFake::failing_reconcile(vec![fakes::stored_event("standup", "\"v7\"")]);
    let surfaces = Arc::new(Mutex::new(Vec::new()));
    let app = calendar_app(vec![calendar_account("acct-a", provider)], &surfaces);
    app.dispatch(Intent::RefreshCalendar).await;
    app.dispatch(Intent::Events(EventIntent::Create {
        title: "Kickoff".to_owned(),
        start: "2026-09-01T09:00:00Z".to_owned(),
        end: "2026-09-01T09:30:00Z".to_owned(),
        account: None,
        calendar: None,
        all_day: false,
        timezone: None,
        notes: None,
        location: None,
        recurrence: None,
        invitees: None,
        meeting: None,
    }))
    .await;

    let lines = crate::tests_log_capture::lines_from(&["mailcal_app::calendar_status"]);
    assert!(
        lines
            .iter()
            .any(|line| line.contains("reconciliation failed")
                && line.ends_with("the server cannot do this")),
        "captured:\n{}",
        lines.join("\n"),
    );
    assert!(
        lines.iter().all(|line| !line.contains("/cal/")),
        "captured:\n{}",
        lines.join("\n"),
    );
}
