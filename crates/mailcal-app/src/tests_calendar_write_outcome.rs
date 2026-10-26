//! What the calendar tells a person after a change that did not succeed: never that it may have
//! been saved when nothing was sent, and never that nothing was saved when it may have been.

use std::sync::{Arc, Mutex};

use super::{
    CalendarWriteStatus, EventIntent, Intent,
    fakes::{self, CalendarFake, calendar_account, calendar_app, evt},
};

fn kickoff() -> Intent {
    Intent::Events(EventIntent::Create {
        title: "Kickoff".to_owned(),
        start: "2026-09-01T09:00:00Z".to_owned(),
        end: "2026-09-01T09:30:00Z".to_owned(),
        account: Some("acct-a".to_owned()),
        calendar: None,
        all_day: false,
        timezone: None,
        notes: None,
        location: None,
        recurrence: None,
        invitees: None,
    })
}

/// A calendar listed again without its adapter (it no longer connects) still has what was last
/// synced on the grid. A change made there reaches no server, and says so rather than nothing.
#[tokio::test]
async fn a_change_to_a_calendar_that_no_longer_connects_is_not_saved() {
    let surfaces = Arc::new(Mutex::new(Vec::new()));
    let app = calendar_app(
        vec![calendar_account(
            "acct-a",
            CalendarFake::with_events(vec![fakes::stored_event("standup", "\"v7\"")]),
        )],
        &surfaces,
    );
    app.dispatch(Intent::RefreshCalendar).await;
    let mut disconnected = calendar_account("acct-a", CalendarFake::with_events(Vec::new()));
    disconnected.calendar_providers.clear();
    app.add_account_deferred(disconnected).await;

    app.dispatch(kickoff()).await;
    assert_eq!(app.calendar_write_status(), CalendarWriteStatus::NotSaved);

    let edit = app
        .update_event(
            &evt("acct-a", "standup"),
            &mailcal_account::EventEdit {
                title: Some("Standup (kort)".to_owned()),
                ..mailcal_account::EventEdit::default()
            },
        )
        .await;
    assert!(edit.is_err());
    assert_eq!(app.calendar_write_status(), CalendarWriteStatus::NotSaved);
}

/// An edit whose answer was lost after it was sent may have landed, so it is not reported as
/// lost.
#[tokio::test]
async fn an_edit_whose_answer_was_lost_is_unconfirmed() {
    let surfaces = Arc::new(Mutex::new(Vec::new()));
    let app = calendar_app(
        vec![calendar_account(
            "acct-a",
            CalendarFake::losing_patch_answers(vec![fakes::stored_event("standup", "\"v7\"")]),
        )],
        &surfaces,
    );
    app.dispatch(Intent::RefreshCalendar).await;

    let edit = app
        .update_event(
            &evt("acct-a", "standup"),
            &mailcal_account::EventEdit {
                title: Some("Standup (kort)".to_owned()),
                ..mailcal_account::EventEdit::default()
            },
        )
        .await;
    assert!(edit.is_err());
    assert_eq!(
        app.calendar_write_status(),
        CalendarWriteStatus::Unconfirmed
    );
}
