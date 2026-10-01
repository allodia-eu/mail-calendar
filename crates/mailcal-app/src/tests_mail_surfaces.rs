//! An account used for its calendar or contacts alone: out of every mail surface, and still able
//! to say its server is unreachable.

use std::sync::{Arc, Mutex};

use fakes::{CalendarFake, calendar_account, calendar_app};

#[allow(clippy::duplicate_mod)]
#[path = "tests_fakes.rs"]
mod fakes;

#[tokio::test]
async fn an_account_without_mail_is_in_no_mail_surface() {
    let surfaces = Arc::new(Mutex::new(Vec::new()));
    let mailbox = calendar_account("mailbox", CalendarFake::with_events(Vec::new()));
    let mut calendar = calendar_account("calendar", CalendarFake::with_events(Vec::new()));
    calendar.uses_mail = false;
    let app = calendar_app(vec![mailbox, calendar], &surfaces);

    // The switcher, the folder pane, the From picker and the assistant's account list all read
    // these rows.
    let rows: Vec<String> = app
        .account_rows()
        .await
        .into_iter()
        .map(|row| row.id)
        .collect();
    assert_eq!(rows, ["mailbox"]);

    let sync: Vec<String> = app
        .sync_settings()
        .await
        .accounts
        .into_iter()
        .map(|row| row.account_id)
        .collect();
    assert_eq!(
        sync,
        ["mailbox"],
        "no mail to sync, so no mail sync settings"
    );

    let signatures: Vec<String> = app
        .signatures()
        .await
        .accounts
        .into_iter()
        .map(|row| row.account_id)
        .collect();
    assert_eq!(signatures, ["mailbox"], "no mail to sign");
}

/// With no mail pass to learn it from, an account without mail learns from its calendar whether
/// its server answers; otherwise it could never say it is unreachable.
#[tokio::test]
async fn an_account_without_mail_is_unreachable_when_its_calendar_is() {
    let surfaces = Arc::new(Mutex::new(Vec::new()));
    let mut calendar = calendar_account("calendar", CalendarFake::unreachable());
    calendar.uses_mail = false;
    let app = calendar_app(vec![calendar], &surfaces);

    app.refresh_calendar().await;

    assert_eq!(app.connectivity().unreachable_accounts, ["calendar"]);
}

/// An account with mail keeps learning it from its mail: a calendar that fails beside a mailbox
/// empties the calendar and does not badge the account.
#[tokio::test]
async fn an_account_with_mail_is_not_badged_for_its_calendar_alone() {
    let surfaces = Arc::new(Mutex::new(Vec::new()));
    let mailbox = calendar_account("mailbox", CalendarFake::unreachable());
    let app = calendar_app(vec![mailbox], &surfaces);

    app.refresh_calendar().await;

    assert!(app.connectivity().unreachable_accounts.is_empty());
}
