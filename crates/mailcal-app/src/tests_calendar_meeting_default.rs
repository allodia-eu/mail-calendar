//! Each account's default video meeting (`docs/conferencing.md`, rule 10).

use std::sync::{Arc, Mutex};

use engine_api::ConferenceService;

use super::fakes::{CalendarFake, calendar_account, calendar_app};
use crate::Intent;

/// Each calendar row's account and default meeting.
fn defaults(app: &crate::App<CalendarFake>) -> Vec<(String, Option<String>)> {
    app.calendars()
        .into_iter()
        .map(|row| (row.account, row.default_meeting))
        .collect()
}

async fn two_accounts(surfaces: &Arc<Mutex<Vec<crate::Surface>>>) -> crate::App<CalendarFake> {
    let work = CalendarFake::writing_meetings(vec![], &[ConferenceService::Teams]);
    let home = CalendarFake::writing_meetings(vec![], &[]);
    let app = calendar_app(
        vec![
            calendar_account("work", work),
            calendar_account("home", home),
        ],
        surfaces,
    );
    app.set_jitsi_server(Some("https://meet.test.local"))
        .unwrap();
    app.dispatch(Intent::RefreshCalendar).await;
    app
}

#[tokio::test]
async fn an_account_has_no_default_meeting_until_one_is_chosen() {
    let surfaces = Arc::new(Mutex::new(Vec::new()));
    let app = two_accounts(&surfaces).await;
    assert!(defaults(&app).iter().all(|(_, default)| default.is_none()));
    assert!(
        app.meeting_defaults()
            .iter()
            .all(|row| row.chosen.is_none())
    );
}

#[tokio::test]
async fn a_default_is_its_accounts_alone() {
    let surfaces = Arc::new(Mutex::new(Vec::new()));
    let app = two_accounts(&surfaces).await;
    app.set_default_meeting("work", Some("server:teams"));
    app.set_default_meeting("home", Some("jitsi"));
    assert_eq!(
        defaults(&app),
        [
            ("work".to_owned(), Some("server:teams".to_owned())),
            ("home".to_owned(), Some("jitsi".to_owned())),
        ]
    );
    let listed: Vec<_> = app
        .meeting_defaults()
        .into_iter()
        .map(|row| {
            let ids: Vec<_> = row.choices.into_iter().map(|choice| choice.id).collect();
            (row.account, ids, row.chosen)
        })
        .collect();
    assert_eq!(
        listed,
        [
            (
                "work".to_owned(),
                vec!["server:teams".to_owned(), "jitsi".to_owned()],
                Some("server:teams".to_owned())
            ),
            (
                "home".to_owned(),
                vec!["jitsi".to_owned()],
                Some("jitsi".to_owned())
            ),
        ]
    );

    app.set_default_meeting("work", None);
    assert_eq!(defaults(&app)[0].1, None);
    assert_eq!(defaults(&app)[1].1.as_deref(), Some("jitsi"));
}

#[tokio::test]
async fn a_default_no_calendar_offers_any_more_is_not_applied_and_comes_back() {
    let surfaces = Arc::new(Mutex::new(Vec::new()));
    let app = two_accounts(&surfaces).await;
    app.set_default_meeting("home", Some("jitsi"));
    // A choice the account's calendars never offered is none.
    app.set_default_meeting("work", Some("server:meet"));
    assert_eq!(defaults(&app)[0].1, None);

    app.set_jitsi_server(None).unwrap();
    assert_eq!(defaults(&app)[1].1, None);
    assert!(
        app.meeting_defaults()
            .iter()
            .all(|row| row.account != "home"),
        "an account whose calendars offer no meeting is not listed"
    );

    app.set_jitsi_server(Some("https://meet.test.local"))
        .unwrap();
    assert_eq!(defaults(&app)[1].1.as_deref(), Some("jitsi"));
}

#[tokio::test]
async fn choosing_a_default_redraws_the_calendar() {
    let surfaces = Arc::new(Mutex::new(Vec::new()));
    let app = two_accounts(&surfaces).await;
    surfaces.lock().unwrap().clear();
    app.set_default_meeting("work", Some("server:teams"));
    assert!(surfaces.lock().unwrap().contains(&crate::Surface::Calendar));
}

#[tokio::test]
async fn the_jitsi_field_is_kept_refused_or_left_by_the_core_and_unchanged_text_writes_nothing() {
    let surfaces = Arc::new(Mutex::new(Vec::new()));
    let app = two_accounts(&surfaces).await;
    let page = app.videoconferencing_page();
    assert_eq!(
        page.jitsi_server.as_deref(),
        Some("https://meet.test.local")
    );
    assert_eq!(page.jitsi_refused, None);
    assert_eq!(page.defaults.len(), 2);

    // The same server, as a field left with a trailing `/`, is no change: no write, no redraw.
    surfaces.lock().unwrap().clear();
    app.set_jitsi_server(Some("https://meet.test.local/"))
        .unwrap();
    assert!(surfaces.lock().unwrap().is_empty());

    // A refusal is the page's until the next text is sent, exactly as it was typed.
    assert!(app.set_jitsi_server(Some(" meet.test.local ")).is_err());
    let page = app.videoconferencing_page();
    assert_eq!(page.jitsi_refused.as_deref(), Some(" meet.test.local "));
    assert_eq!(
        page.jitsi_server.as_deref(),
        Some("https://meet.test.local")
    );
    assert_eq!(
        surfaces.lock().unwrap().as_slice(),
        [crate::Surface::Settings]
    );

    // Sending the stored text again takes the refusal away without writing anything.
    surfaces.lock().unwrap().clear();
    app.set_jitsi_server(Some("https://meet.test.local"))
        .unwrap();
    assert_eq!(app.videoconferencing_page().jitsi_refused, None);
    assert_eq!(
        surfaces.lock().unwrap().as_slice(),
        [crate::Surface::Settings]
    );

    // A new server is stored and redraws the calendars, which the page is read again on.
    surfaces.lock().unwrap().clear();
    app.set_jitsi_server(Some("https://video.test.local"))
        .unwrap();
    assert_eq!(
        app.videoconferencing_page().jitsi_server.as_deref(),
        Some("https://video.test.local")
    );
    assert!(surfaces.lock().unwrap().contains(&crate::Surface::Calendar));
}
