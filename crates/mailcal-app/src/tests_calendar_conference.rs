//! An event's meeting, end to end through the app: the detail offers it, and every grid row
//! the event draws says it has one (`docs/conferencing.md`).

use std::sync::{Arc, Mutex};

use engine_core::calendar::{ConferenceUri, FlatEntry, group_flat};

use super::fakes::{CalendarFake, calendar_account, calendar_app, event_from_today, evt};
use crate::Intent;

#[tokio::test]
async fn a_meeting_reaches_the_detail_and_marks_its_block() {
    let surfaces = Arc::new(Mutex::new(Vec::new()));
    let mut call = event_from_today("call", 0, 9, 30);
    call.conferences = group_flat(vec![
        FlatEntry::new(ConferenceUri::parse("https://zoom.us/j/987?pwd=x").unwrap()),
        FlatEntry::new(ConferenceUri::parse("tel:+31201234567").unwrap()),
    ]);
    let rule = "_".repeat(80);
    call.description = Some(format!(
        "Agenda first.\n\n{rule}\nMicrosoft Teams meeting\nJoin\n{rule}\n"
    ));
    let lunch = event_from_today("lunch", 0, 12, 60);
    let app = calendar_app(
        vec![calendar_account(
            "acct",
            CalendarFake::with_events(vec![call, lunch]),
        )],
        &surfaces,
    );
    app.dispatch(Intent::RefreshCalendar).await;

    let detail = app
        .event_detail(&evt("acct", "call"), None)
        .await
        .expect("the event resolves");
    assert_eq!(detail.conferences.len(), 1);
    let zoom = &detail.conferences[0];
    assert_eq!(zoom.service.as_deref(), Some("Zoom"));
    assert_eq!(
        zoom.join_url.as_deref(),
        Some("https://zoom.us/j/987?pwd=x")
    );
    assert_eq!(zoom.dial_in[0].uri.as_deref(), Some("tel:+31201234567"));
    assert_eq!(detail.display_notes.as_deref(), Some("Agenda first."));

    let page = app.calendar_range(app.week_start_date(super::today()), 7);
    let marked: Vec<(&str, bool)> = page
        .grid
        .timed
        .iter()
        .map(|block| (block.event.as_str(), block.has_conference))
        .collect();
    assert!(marked.contains(&("call", true)), "{marked:?}");
    assert!(marked.contains(&("lunch", false)), "{marked:?}");
}
