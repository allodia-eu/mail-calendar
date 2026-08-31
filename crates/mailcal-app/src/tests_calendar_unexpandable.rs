//! What a rule the engine cannot expand costs the rest of the calendar.
//!
//! The answer has to be **itself and nothing else**. Such an event is stored, materializes no
//! occurrences and is drawn nowhere; the reason travels out on the sync report, which
//! `calendar_refresh` turns into one diagnostic line. Everything else in the account syncs.
//!
//! One rule did not follow that contract: a repeat whose step is a span of days too large to
//! build **aborted the process** instead of failing, so a single `RRULE` from any calendar
//! server took every account on the device down with it. It needed no write of ours: only a
//! server with a bad generator. Fixed upstream; this is the core-side lock, and it lives here
//! rather than beside the write guards because nothing about it involves a write.

use std::sync::{Arc, Mutex};

use engine_api::{Frequency, Recurrence, RecurrenceRule};
use fakes::{CalendarFake, calendar_account, calendar_app};

use super::Intent;

#[allow(clippy::duplicate_mod)]
#[path = "tests_fakes.rs"]
mod fakes;

/// The calendar date `offset` days from today.
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

#[tokio::test]
async fn a_rule_too_large_to_expand_costs_only_its_own_event() {
    // 1,043,498 weeks is 7,304,486 days, two past what a span can hold. The engine used to
    // panic building it (mid-sync, on a read) so this test would not have failed, it would
    // have killed the runner. That it reports at all is half the assertion; the other half is
    // that the standup beside it still arrives.
    let mut hostile = fakes::weekly_event_from_today("hostile", 0, 14, 30);
    let mut rule = RecurrenceRule::new(Frequency::Weekly);
    rule.interval = std::num::NonZeroU32::new(1_043_498).expect("non-zero");
    hostile.recurrence = Some(Recurrence::from_rule(rule));

    let provider = CalendarFake::with_events(vec![
        hostile,
        fakes::weekly_event_from_today("standup", 0, 9, 30),
    ]);
    let surfaces = Arc::new(Mutex::new(Vec::new()));
    let app = calendar_app(vec![calendar_account("acct-a", provider)], &surfaces);

    app.dispatch(Intent::RefreshCalendar).await;

    let drawn = app.calendar_range(day_from_today(-30), 400).grid.timed;
    assert!(
        drawn.iter().all(|segment| segment.event != "hostile"),
        "the rule cannot be drawn, so the event is not on the grid"
    );
    assert!(
        drawn.iter().filter(|s| s.event == "standup").count() > 12,
        "the rest of the account synced; one unexpandable event costs one event"
    );
}
