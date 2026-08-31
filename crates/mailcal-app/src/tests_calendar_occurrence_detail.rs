//! Which occurrence's times a detail reports.
//!
//! A series' own start is its **first** occurrence's. Project that for every occurrence and a
//! detail opened on any later one reads the wrong date: a plain read, with no editing
//! involved, and an editor prefilled from it writes that date back. So the token the tapped
//! surface carried has to reach the read, and the times have to come from what the expander
//! produced for that instant rather than from the master.
//!
//! The fixture is a weekly standup whose second occurrence the user has already moved to
//! another hour, which is the shape that tells the three candidate answers apart: the master's
//! time, the slot's time, and the time the occurrence actually keeps.

use std::sync::{Arc, Mutex};

use fakes::{CalendarFake, calendar_account, calendar_app, evt};

use super::Intent;

#[allow(clippy::duplicate_mod)]
#[path = "tests_fakes.rs"]
mod fakes;

/// The standup: 09:00 weekly from today (30 minutes long), with the occurrence a week out
/// moved to 14:00.
fn moved_standup() -> engine_api::Event {
    fakes::weekly_event_with_a_moved_occurrence("standup", 0, 9, 30, 1, 14)
}

/// The token naming the occurrence `weeks_out` weeks from the series start, as the grid mints
/// it: from the **recurrence id**, which is the slot the occurrence came from rather than where
/// it now sits.
fn token_for(event: &engine_api::Event, weeks_out: i64) -> String {
    fakes::occurrence_wall_clock_of(event, weeks_out).to_string()
}

/// The detail of `key`, opened either on the series or on one occurrence.
async fn detail_for(
    event: engine_api::Event,
    occurrence: Option<&str>,
) -> mailcal_account::EventDetail {
    let surfaces = Arc::new(Mutex::new(Vec::new()));
    let app = calendar_app(
        vec![calendar_account(
            "acct-a",
            CalendarFake::with_events(vec![event]),
        )],
        &surfaces,
    );
    app.dispatch(Intent::RefreshCalendar).await;
    app.event_detail(&evt("acct-a", "standup"), occurrence)
        .await
        .expect("the event is in the store")
}

#[tokio::test]
async fn an_occurrence_reports_its_own_times_not_the_series() {
    let event = moved_standup();
    let series = detail_for(event.clone(), None).await;
    let second = detail_for(event.clone(), Some(&token_for(&event, 1))).await;

    assert_ne!(
        second.start, series.start,
        "the second occurrence is a week after the first, so its detail cannot read the same"
    );
    assert!(
        second.start.ends_with("T14:00:00"),
        "it reports the hour the user moved it to, not the series' 09:00 and not the slot's: \
         {}",
        second.start
    );
}

#[tokio::test]
async fn an_occurrence_names_itself_so_a_client_knows_what_it_may_ask() {
    let event = moved_standup();
    let token = token_for(&event, 1);
    let detail = detail_for(event, Some(&token)).await;

    assert_eq!(
        detail.occurrence_start, token,
        "handed back unchanged, so it can go straight into the write that follows"
    );
}

#[tokio::test]
async fn the_series_names_no_occurrence() {
    // What an agenda row opens, and the only thing a one-off event can be. A client reads this
    // to decide whether to put its scope question at all.
    assert!(
        detail_for(moved_standup(), None)
            .await
            .occurrence_start
            .is_empty()
    );
}

#[tokio::test]
async fn a_token_naming_no_occurrence_falls_back_to_the_series() {
    // A token goes stale when the series changes underneath the view it was drawn in. The
    // detail still opens (a closed sheet would be worse) and it says it describes the
    // series, so no client offers *This event* against times that belong to another one.
    let series = detail_for(moved_standup(), None).await;
    let stale = detail_for(moved_standup(), Some("2019-01-01T09:30:00")).await;

    assert!(stale.occurrence_start.is_empty());
    assert_eq!(stale.start, series.start);
    assert_eq!(stale.end, series.end);
}

#[tokio::test]
async fn nonsense_in_the_token_is_not_a_crash() {
    // It crosses the FFI as a string, so it can hold anything a client sends.
    let detail = detail_for(moved_standup(), Some("not-a-time")).await;
    assert!(detail.occurrence_start.is_empty());
}

#[tokio::test]
async fn an_occurrence_keeps_the_series_duration_when_only_its_start_moved() {
    // The end comes from the expander too, so a moved occurrence's end follows its start
    // rather than staying on the master's clock.
    let event = moved_standup();
    let second = detail_for(event.clone(), Some(&token_for(&event, 1))).await;
    assert!(
        second.end > second.start,
        "an occurrence's end is after its own start: {} → {}",
        second.start,
        second.end
    );
    assert!(
        second.end.ends_with("T14:30:00"),
        "the override moved the start and left the duration, so the end follows it rather \
         than staying on the series' clock: {} → {}",
        second.start,
        second.end
    );
}

#[tokio::test]
async fn an_untouched_occurrence_reports_the_slot_it_sits_in() {
    // The case with no override at all: the third occurrence is where the rule puts it, which
    // is still not where the master is.
    let event = moved_standup();
    let third = detail_for(event.clone(), Some(&token_for(&event, 2))).await;
    let series = detail_for(event, None).await;

    assert_ne!(third.start, series.start);
    assert!(
        third.start.ends_with("T09:00:00"),
        "an occurrence nobody moved keeps the series' clock on its own day: {}",
        third.start
    );
}
