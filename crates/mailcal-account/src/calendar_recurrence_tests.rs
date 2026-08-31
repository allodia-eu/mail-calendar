//! What the write builders do with a repeat rule.
//!
//! Two assertions here are the ones that matter. `an_until_reaches_the_engine_as_an_instant`
//! covers the conversion no adapter can do for itself, and
//! `a_rule_we_could_not_describe_is_never_rewritten` is the guard that keeps a client's
//! partial picture from becoming the user's whole series.
//!
//! Split into their own `#[path]` file to keep `calendar_tests.rs` under the 500-line limit.

use std::num::{NonZeroI32, NonZeroU32};

use engine_api::{Frequency, NDay, Recurrence, RecurrenceBound, RecurrenceEdit, Weekday};
use engine_core::{
    calendar::Event,
    ids::{CalendarId, EventId, Uid},
    membership::Memberships,
    time::{CalendarDateTime, Duration, LocalDateTime, TimeZoneId, UtcDateTime},
    version::{ETag, RevisionTokens},
};

use super::*;
use crate::{RecurrenceDay, RecurrenceEnd, RecurrenceFrequency, RecurrenceWeekday};

fn now() -> UtcDateTime {
    UtcDateTime::new(2026, 2, 10, 11, 30, 0).unwrap()
}

fn local(text: &str) -> LocalDateTime {
    text.parse().unwrap()
}

fn amsterdam() -> TimeZoneId {
    TimeZoneId::iana("Europe/Amsterdam").unwrap()
}

/// A plain weekly rule, with whatever end the caller wants.
fn weekly(end: RecurrenceEnd) -> SimpleRecurrence {
    SimpleRecurrence {
        frequency: RecurrenceFrequency::Weekly,
        interval: 1,
        days: Vec::new(),
        month_days: Vec::new(),
        months: Vec::new(),
        end,
    }
}

/// A stored series in Amsterdam, repeating on `rule`.
fn stored_series(rule: engine_api::RecurrenceRule) -> Event {
    let mut event = Event::new(
        EventId::try_from("/cal/standup.ics").unwrap(),
        Uid::new("standup@test.local").unwrap(),
        Memberships::of_one(CalendarId::try_from("/cal/").unwrap()),
        CalendarDateTime::Zoned {
            local: local("2026-01-05T09:30:00"),
            zone: amsterdam(),
        },
    );
    event.title = "Standup".to_owned();
    event.duration = Duration::from_parts(0, 0, 0, 30, 0, 0).unwrap();
    event.revisions = RevisionTokens::from_etag(ETag::new("\"v7\""));
    event.recurrence = Some(Recurrence::from_rule(rule));
    event
}

/// A create draft for a series repeating on `rule`, zoned in Amsterdam or all-day.
fn draft_repeating(rule: &SimpleRecurrence, all_day: bool) -> EventDraft {
    let (start, end) = if all_day {
        ("2026-08-01", "2026-08-02")
    } else {
        ("2026-08-01T09:00:00", "2026-08-01T09:30:00")
    };
    build_event_draft(
        CalendarId::try_from("/cal/").unwrap(),
        "new-event@test.local",
        "Standup",
        start,
        end,
        all_day,
        (!all_day).then_some("Europe/Amsterdam"),
        None,
        None,
        Some(rule),
        now(),
    )
    .expect("a well-formed series builds")
}

#[test]
fn a_create_carries_the_rule_it_was_given() {
    let draft = draft_repeating(&weekly(RecurrenceEnd::Never), false);

    let repeat = draft.recurrence.expect("the draft repeats");
    assert_eq!(repeat.rule.frequency, Frequency::Weekly);
    assert_eq!(repeat.rule.bound, RecurrenceBound::Unbounded);
    assert_eq!(repeat.until, None, "an unbounded rule has no UNTIL");
}

#[test]
fn an_until_reaches_the_engine_as_an_instant() {
    // RFC 5545 §3.3.10 requires `UNTIL` in UTC once the event is zoned, and no adapter carries
    // the tzdata to convert one. Amsterdam is UTC+2 in August, so 17:00 local is 15:00Z; the
    // resolution is what this asserts, not merely that a field is populated.
    let rule = weekly(RecurrenceEnd::OnDate {
        date: "2026-08-29T17:00:00".to_owned(),
    });

    let repeat = draft_repeating(&rule, false)
        .recurrence
        .expect("the draft repeats");

    assert_eq!(
        repeat.until,
        Some(UtcDateTime::new(2026, 8, 29, 15, 0, 0).unwrap()),
        "the wall clock resolved through the event's own zone"
    );
}

#[test]
fn an_all_day_series_needs_no_instant() {
    // A zoneless series renders its own `UNTIL` from the wall clock, so there is nothing to
    // resolve, and resolving one would be inventing a zone the event does not have.
    let rule = weekly(RecurrenceEnd::OnDate {
        date: "2026-08-29T00:00:00".to_owned(),
    });

    let repeat = draft_repeating(&rule, true)
        .recurrence
        .expect("the draft repeats");

    assert_eq!(repeat.until, None);
    assert!(matches!(repeat.rule.bound, RecurrenceBound::Until(_)));
}

#[test]
fn a_rule_that_describes_no_series_is_refused() {
    let nonsense = SimpleRecurrence {
        interval: 0,
        ..weekly(RecurrenceEnd::Never)
    };

    let refused = build_event_draft(
        CalendarId::try_from("/cal/").unwrap(),
        "new-event@test.local",
        "Standup",
        "2026-08-01T09:00:00",
        "2026-08-01T09:30:00",
        false,
        Some("Europe/Amsterdam"),
        None,
        None,
        Some(&nonsense),
        now(),
    );

    assert!(
        refused.is_err(),
        "a rule repeating every zero weeks is not a series"
    );
}

#[test]
fn an_edit_replaces_the_rule_and_can_take_it_away() {
    let stored = stored_series(engine_api::RecurrenceRule::new(Frequency::Weekly));
    let mut fortnightly = weekly(RecurrenceEnd::Never);
    fortnightly.interval = 2;

    let (_, set) = build_event_patch(
        &stored,
        &EventEdit {
            recurrence: Some(RecurrenceChange::Set(fortnightly)),
            ..EventEdit::default()
        },
        now(),
    )
    .expect("replacing a simple rule is allowed");
    let (_, cleared) = build_event_patch(
        &stored,
        &EventEdit {
            recurrence: Some(RecurrenceChange::Clear),
            ..EventEdit::default()
        },
        now(),
    )
    .expect("a series can stop repeating");
    let (_, untouched) = build_event_patch(&stored, &EventEdit::default(), now())
        .expect("an edit that says nothing about recurrence is allowed");

    let Some(RecurrenceEdit::Set(rule)) = set.recurrence_edit() else {
        panic!("expected the rule to be replaced");
    };
    assert_eq!(rule.rule.interval, NonZeroU32::new(2).unwrap());
    assert!(matches!(
        cleared.recurrence_edit(),
        Some(RecurrenceEdit::Clear)
    ));
    assert!(
        untouched.recurrence_edit().is_none(),
        "not mentioning recurrence leaves the series exactly as it was"
    );
}

#[test]
fn a_rule_we_could_not_describe_is_never_rewritten() {
    // The client saw "it repeats" and nothing more, because `bySetPosition` is not in the
    // shape the FFI carries. Whatever its editor holds is missing that part, so writing it
    // back would turn "the fourth Wednesday" into "every Wednesday"; silently, on a real
    // series. A client is gated on the same answer; this is the write refusing to trust it.
    let mut rich = engine_api::RecurrenceRule::new(Frequency::Monthly);
    rich.by_day = vec![NDay {
        day: Weekday::We,
        nth_of_period: None,
    }];
    rich.by_set_position = vec![4];
    let stored = stored_series(rich);

    let refused = build_event_patch(
        &stored,
        &EventEdit {
            recurrence: Some(RecurrenceChange::Set(weekly(RecurrenceEnd::Never))),
            ..EventEdit::default()
        },
        now(),
    );
    let allowed = build_event_patch(
        &stored,
        &EventEdit {
            recurrence: Some(RecurrenceChange::Clear),
            ..EventEdit::default()
        },
        now(),
    );

    assert!(
        refused.is_err(),
        "the rule we cannot see is not overwritten"
    );
    assert!(
        allowed.is_ok(),
        "stopping the repeat needs no knowledge of the rule: the user asked for one event"
    );
}

#[test]
fn a_rule_cannot_be_changed_on_one_occurrence() {
    // An occurrence is an instance *of* a rule, not a holder of one. The adapters refuse the
    // pairing; refusing it here means the user is told before a round trip, and the reason is
    // the real one rather than a transport's.
    let stored = stored_series(engine_api::RecurrenceRule::new(Frequency::Weekly));

    let refused = build_event_patch(
        &stored,
        &EventEdit {
            recurrence: Some(RecurrenceChange::Clear),
            occurrence: Some(local("2026-01-12T09:30:00")),
            ..EventEdit::default()
        },
        now(),
    );

    assert!(refused.is_err());
}

#[test]
fn deleting_one_occurrence_names_it_by_its_original_start() {
    // The same resolution the patch target does, and for the same reason: Google addresses an
    // occurrence by that start **in UTC**. January in Amsterdam is UTC+1, so 09:30 is 08:30Z.
    let stored = stored_series(engine_api::RecurrenceRule::new(Frequency::Weekly));

    let deletion = build_event_deletion(&stored, Some(local("2026-01-12T09:30:00")), now())
        .expect("a well-formed occurrence delete builds");

    let occurrence = deletion
        .occurrence_target()
        .expect("the delete removes one occurrence");
    assert_eq!(
        occurrence.start,
        CalendarDateTime::Zoned {
            local: local("2026-01-12T09:30:00"),
            zone: amsterdam(),
        },
        "named in the series' own form, never converted"
    );
    assert_eq!(
        occurrence.instant,
        Some(UtcDateTime::new(2026, 1, 12, 8, 30, 0).unwrap()),
        "resolved for the one transport that addresses an occurrence in UTC"
    );
}

#[test]
fn deleting_the_series_keeps_the_guard_it_was_read_at() {
    let stored = stored_series(engine_api::RecurrenceRule::new(Frequency::Weekly));

    let deletion = build_event_deletion(&stored, None, now()).expect("a series delete builds");

    assert!(
        deletion.occurrence_target().is_none(),
        "no occurrence named means the whole event goes"
    );
    assert_eq!(
        deletion.guard,
        Some(stored.revisions.clone()),
        "so the delete cannot discard someone else's newer edit"
    );
}

#[test]
fn every_part_of_the_shape_reaches_the_engine_rule() {
    // "The fourth Monday of every third month, twelve times": the whole shape at once, so a
    // field dropped between the editor's record and the engine rule shows up here.
    let rule = SimpleRecurrence {
        frequency: RecurrenceFrequency::Monthly,
        interval: 3,
        days: vec![RecurrenceDay {
            day: RecurrenceWeekday::Monday,
            nth: Some(4),
        }],
        month_days: vec![24],
        months: Vec::new(),
        end: RecurrenceEnd::AfterCount { count: 12 },
    };

    let repeat = draft_repeating(&rule, false)
        .recurrence
        .expect("the draft repeats");

    assert_eq!(repeat.rule.frequency, Frequency::Monthly);
    assert_eq!(repeat.rule.interval, NonZeroU32::new(3).unwrap());
    assert_eq!(
        repeat.rule.by_day,
        vec![NDay {
            day: Weekday::Mo,
            nth_of_period: Some(NonZeroI32::new(4).unwrap()),
        }]
    );
    assert_eq!(repeat.rule.by_month_day, vec![24]);
    assert_eq!(
        repeat.rule.bound,
        RecurrenceBound::Count(NonZeroU32::new(12).unwrap())
    );
}
