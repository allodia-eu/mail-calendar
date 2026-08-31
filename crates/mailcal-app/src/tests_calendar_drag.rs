//! What a drag on the grid actually sends, and what it refuses to send.
//!
//! The pure arithmetic is pinned next to it, in `mailcal_account::calendar_drag`. What can only
//! be asserted *here* is the wiring: that the intent reaches the patch path in the event's own
//! form, that one occurrence of a series is targeted as an instance rather than as the series,
//! and (the one that matters most) that a meeting somebody else called is refused by the
//! **core**, not merely hidden by the client.
//!
//! Split out of `tests_calendar_actions.rs`, which is near the 500-line limit.

use std::sync::{Arc, Mutex};

use engine_api::{
    CalendarDateTime, LocalDateTime, ParticipantRole, ParticipationStatus, TimeZoneId,
    resolve_instant,
};
use engine_provider::{Occurrence, PatchTarget};
use fakes::{CalendarFake, calendar_account, calendar_app, evt};
use mailcal_account::{EventDrag, EventEdge};

use super::Intent;

#[allow(clippy::duplicate_mod)]
#[path = "tests_fakes.rs"]
mod fakes;

/// Amsterdam at a wall clock: the form `stored_event`'s `DTSTART;TZID=Europe/Amsterdam` has, and
/// the form a patch must come back in.
fn amsterdam(hour: u8, minute: u8) -> CalendarDateTime {
    CalendarDateTime::Zoned {
        local: LocalDateTime::new(2026, 1, 5, hour, minute, 0).unwrap(),
        zone: TimeZoneId::iana("Europe/Amsterdam").unwrap(),
    }
}

fn drag(edge: EventEdge, days: i32, minutes: i32) -> EventDrag {
    EventDrag {
        edge,
        days,
        minutes,
        occurrence: None,
    }
}

#[tokio::test]
async fn a_drag_patches_a_wall_clock_in_the_events_own_zone() {
    // The fixture is `DTSTART;TZID=Europe/Amsterdam:20260105T093000`. Dragged down half an hour,
    // what must reach the provider is **10:00 Amsterdam**: not a UTC instant, and not the clock
    // of whatever zone the grid happened to be drawn in. A UTC value here would move the meeting
    // for every other attendee while looking right on this screen.
    let provider = CalendarFake::with_events(vec![fakes::stored_event("standup", "\"v7\"")]);
    let patches = provider.patches();
    let surfaces = Arc::new(Mutex::new(Vec::new()));
    let app = calendar_app(vec![calendar_account("acct-a", provider)], &surfaces);
    app.dispatch(Intent::RefreshCalendar).await;

    app.dispatch(Intent::MoveEvent {
        event: evt("acct-a", "standup"),
        drag: drag(EventEdge::Whole, 0, 30),
    })
    .await;

    let sent = patches.lock().unwrap();
    let patch = sent.first().expect("the provider received a patch");
    assert_eq!(patch.start, Some(amsterdam(10, 0)));
    assert_eq!(patch.end, Some(amsterdam(10, 30)));
    assert_eq!(
        patch.summary, None,
        "a drag moves the times and touches nothing else"
    );
}

#[tokio::test]
async fn a_resize_moves_only_the_edge_that_was_dragged() {
    let provider = CalendarFake::with_events(vec![fakes::stored_event("standup", "\"v7\"")]);
    let patches = provider.patches();
    let surfaces = Arc::new(Mutex::new(Vec::new()));
    let app = calendar_app(vec![calendar_account("acct-a", provider)], &surfaces);
    app.dispatch(Intent::RefreshCalendar).await;

    app.dispatch(Intent::MoveEvent {
        event: evt("acct-a", "standup"),
        drag: drag(EventEdge::End, 0, 45),
    })
    .await;

    let sent = patches.lock().unwrap();
    let patch = sent.first().expect("the provider received a patch");
    assert_eq!(
        patch.start,
        Some(amsterdam(9, 30)),
        "the untouched edge is re-sent unchanged, never shifted"
    );
    assert_eq!(patch.end, Some(amsterdam(10, 45)));
}

#[tokio::test]
async fn dragging_one_occurrence_targets_that_occurrence_not_the_series() {
    // Sending the occurrence token must split a `RECURRENCE-ID` override out of the series; a
    // `PatchTarget::Series` here would rewrite every Monday to eternity, which is the failure
    // the "this event / all events" question exists to prevent.
    //
    // The series is **zoned and today-relative**, and both halves are load-bearing. Zoned,
    // because the token is written in the series' own zone and a UTC fixture cannot tell a
    // correct implementation from one that ignores the zone at all. Today-relative, because a
    // fixture on a fixed date falls out of the rolling horizon, so the store materializes no
    // occurrence for it, and the token is then one the core never minted, which is exactly
    // what `calendar_scope` refuses.
    let zone = TimeZoneId::iana("Europe/Amsterdam").unwrap();
    let mut series = fakes::weekly_event_from_today("standup", 0, 9, 30);
    series.start = CalendarDateTime::Zoned {
        local: series.start.local().expect("the fixture is timed"),
        zone: zone.clone(),
    };
    let provider = CalendarFake::with_events(vec![series]);
    let patches = provider.patches();
    let surfaces = Arc::new(Mutex::new(Vec::new()));
    let app = calendar_app(vec![calendar_account("acct-a", provider)], &surfaces);
    app.dispatch(Intent::RefreshCalendar).await;

    // Read off the grid rather than constructed: a client hands back the token it was drawn,
    // so a test that mints its own is testing a path no client takes.
    let occurrence: LocalDateTime = segment_on_page(&app, 7, "standup")
        .expect("next week's occurrence is drawn")
        .occurrence_start
        .parse()
        .expect("the grid's token is a wall clock");
    app.dispatch(Intent::MoveEvent {
        event: evt("acct-a", "standup"),
        drag: EventDrag {
            edge: EventEdge::Whole,
            days: 0,
            minutes: 30,
            occurrence: Some(occurrence),
        },
    })
    .await;

    let sent = patches.lock().unwrap();
    let patch = sent.first().expect("the provider received a patch");
    assert_eq!(
        patch.target,
        PatchTarget::Instance(Occurrence::at(
            CalendarDateTime::Zoned {
                local: occurrence,
                zone: zone.clone(),
            },
            resolve_instant(&CalendarDateTime::Zoned {
                local: occurrence,
                zone,
            })
            .unwrap()
            .expect("a zoned occurrence resolves to an instant"),
        )),
        "the occurrence is named in the event's own zone, as a RECURRENCE-ID must be, and by \
         the instant Google builds its occurrence id from"
    );
}

#[tokio::test]
async fn omitting_the_occurrence_moves_the_whole_series() {
    let provider = CalendarFake::with_events(vec![fakes::stored_event("standup", "\"v7\"")]);
    let patches = provider.patches();
    let surfaces = Arc::new(Mutex::new(Vec::new()));
    let app = calendar_app(vec![calendar_account("acct-a", provider)], &surfaces);
    app.dispatch(Intent::RefreshCalendar).await;

    app.dispatch(Intent::MoveEvent {
        event: evt("acct-a", "standup"),
        drag: drag(EventEdge::Whole, 1, 0),
    })
    .await;

    let sent = patches.lock().unwrap();
    assert_eq!(
        sent.first().expect("a patch").target,
        PatchTarget::Series,
        "no occurrence named means the series: the client asked the user which"
    );
}

#[tokio::test]
async fn a_meeting_somebody_else_called_is_refused_by_the_core() {
    // The client hides the gesture on a block whose `can_move` is false. That is the right thing
    // for the user and **not** the check: the intent crosses an FFI, and a write that trusts its
    // caller is not a check at all. Nothing may reach the provider.
    let mut theirs = fakes::stored_event("review", "\"v1\"");
    let mut organizer = engine_api::Participant::attendee("boss@elsewhere.test");
    organizer.roles.insert(ParticipantRole::Owner);
    organizer.participation_status = ParticipationStatus::Accepted;
    let mut me = engine_api::Participant::attendee("me@acct-a.local");
    me.participation_status = ParticipationStatus::Accepted;
    theirs.participants = vec![organizer, me];

    let provider = CalendarFake::with_events(vec![theirs]);
    let patches = provider.patches();
    let surfaces = Arc::new(Mutex::new(Vec::new()));
    let app = calendar_app(vec![calendar_account("acct-a", provider)], &surfaces);
    app.dispatch(Intent::RefreshCalendar).await;

    let outcome = app
        .move_event(&evt("acct-a", "review"), &drag(EventEdge::Whole, 0, 30))
        .await;

    assert!(outcome.is_err(), "somebody else's meeting was re-timed");
    assert!(
        patches.lock().unwrap().is_empty(),
        "nothing reached the provider"
    );
}

#[tokio::test]
async fn a_meeting_we_organize_is_ours_to_drag() {
    // The other side of the same rule: an organiser moving their own meeting is the normal way a
    // meeting gets moved, and the server's scheduling layer tells the attendees.
    let mut ours = fakes::stored_event("review", "\"v1\"");
    let mut organizer = engine_api::Participant::attendee("me@acct-a.local");
    organizer.roles.insert(ParticipantRole::Owner);
    organizer.participation_status = ParticipationStatus::Accepted;
    let mut them = engine_api::Participant::attendee("boss@elsewhere.test");
    them.participation_status = ParticipationStatus::Accepted;
    ours.participants = vec![organizer, them];

    let provider = CalendarFake::with_events(vec![ours]);
    let patches = provider.patches();
    let surfaces = Arc::new(Mutex::new(Vec::new()));
    let app = calendar_app(vec![calendar_account("acct-a", provider)], &surfaces);
    app.dispatch(Intent::RefreshCalendar).await;

    app.move_event(&evt("acct-a", "review"), &drag(EventEdge::Whole, 0, 30))
        .await
        .expect("our own meeting is ours to move");
    assert_eq!(patches.lock().unwrap().len(), 1);
}

/// The date `offset` days from today, as the grid names it: the anchor a page query takes.
///
/// Relative to today because the rolling horizon is: a fixture on a fixed date drops out of the
/// materialized window and the test starts failing on a date nobody chose.
fn day_from_today(offset: i64) -> engine_api::CalendarDate {
    mailcal_viewmodel::calendar::days::date_at(
        i64::try_from(
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("after the epoch")
                .as_secs()
                / 86_400,
        )
        .expect("in range")
            + offset,
    )
}

/// The one segment on the page anchored `offset` days out that belongs to `event`.
fn segment_on_page(
    app: &super::App<CalendarFake>,
    offset: i64,
    event: &str,
) -> Option<mailcal_viewmodel::calendar::grid::TimedSegment> {
    app.calendar_range(day_from_today(offset), 1)
        .grid
        .timed
        .into_iter()
        .find(|segment| segment.event == event)
}

#[tokio::test]
async fn the_grid_reports_whose_events_can_be_dragged() {
    // The flag the client gates the gesture on, read off the page the grid actually renders;
    // so a regression shows up as an undraggable event rather than as a failing unit test three
    // layers down. `can_write` is true for both, only `can_move` tells them apart.
    let mine = fakes::event_from_today("standup", 1, 9, 30);
    let mut theirs = fakes::event_from_today("review", 1, 14, 60);
    let mut organizer = engine_api::Participant::attendee("boss@elsewhere.test");
    organizer.roles.insert(ParticipantRole::Owner);
    organizer.participation_status = ParticipationStatus::Accepted;
    let mut me = engine_api::Participant::attendee("me@acct-a.local");
    me.participation_status = ParticipationStatus::Accepted;
    theirs.participants = vec![organizer, me];

    let provider = CalendarFake::with_events(vec![mine, theirs]);
    let surfaces = Arc::new(Mutex::new(Vec::new()));
    let app = calendar_app(vec![calendar_account("acct-a", provider)], &surfaces);
    app.dispatch(Intent::RefreshCalendar).await;

    let flag = |segment: Option<mailcal_viewmodel::calendar::grid::TimedSegment>| {
        segment.map(|segment| (segment.can_write, segment.can_move))
    };
    assert_eq!(
        flag(segment_on_page(&app, 1, "standup")),
        Some((true, true)),
        "our own appointment"
    );
    assert_eq!(
        flag(segment_on_page(&app, 1, "review")),
        Some((true, false)),
        "writable, and still not ours to re-time"
    );
}

#[tokio::test]
async fn a_recurring_block_carries_the_token_that_names_its_own_occurrence() {
    // Non-empty is how a client knows to ask "this event, or all of them?", and the value is
    // *this occurrence's* wall clock, not the series' first start. Handing back the series start
    // would split an override on the wrong week, silently, on a recurring event.
    let provider =
        CalendarFake::with_events(vec![fakes::weekly_event_from_today("standup", 0, 9, 30)]);
    let surfaces = Arc::new(Mutex::new(Vec::new()));
    let app = calendar_app(vec![calendar_account("acct-a", provider)], &surfaces);
    app.dispatch(Intent::RefreshCalendar).await;

    let next_week = day_from_today(7);
    let segment =
        segment_on_page(&app, 7, "standup").expect("next week's occurrence is on the grid");
    assert_eq!(
        segment.occurrence_start,
        format!(
            "{:04}-{:02}-{:02}T09:00:00",
            next_week.year(),
            next_week.month(),
            next_week.day()
        ),
        "the token names the occurrence that was drawn, not the series' first"
    );
}

#[tokio::test]
async fn an_already_moved_occurrence_is_still_named_by_where_it_started() {
    // An occurrence's identity is its **recurrence id**: the slot in the series it came from;
    // and stays that even after the user moves it. Naming it by where it now sits names no
    // occurrence at all: the second drag of the same Monday would either be refused or split a
    // *second* override at a time the rule never produces, leaving the first one behind.
    let provider = CalendarFake::with_events(vec![fakes::weekly_event_with_a_moved_occurrence(
        "standup", 0, 9, 30, 1, 14,
    )]);
    let surfaces = Arc::new(Mutex::new(Vec::new()));
    let app = calendar_app(vec![calendar_account("acct-a", provider)], &surfaces);
    app.dispatch(Intent::RefreshCalendar).await;

    let next_week = day_from_today(7);
    let segment = segment_on_page(&app, 7, "standup").expect("the moved occurrence is drawn");
    assert_eq!(
        segment.occurrence_start,
        format!(
            "{:04}-{:02}-{:02}T09:00:00",
            next_week.year(),
            next_week.month(),
            next_week.day()
        ),
        "the token is the occurrence's recurrence id (09:00), not the 14:00 it was moved to"
    );
}

#[tokio::test]
async fn a_one_off_carries_no_occurrence_token_so_nothing_is_asked() {
    // Empty is the signal *not* to ask. A one-off event that offered "this event / all events"
    // would be asking a question with one answer.
    let provider = CalendarFake::with_events(vec![fakes::event_from_today("dentist", 2, 11, 45)]);
    let surfaces = Arc::new(Mutex::new(Vec::new()));
    let app = calendar_app(vec![calendar_account("acct-a", provider)], &surfaces);
    app.dispatch(Intent::RefreshCalendar).await;

    let segment = segment_on_page(&app, 2, "dentist").expect("the event is on the grid");
    assert!(segment.occurrence_start.is_empty());
}
