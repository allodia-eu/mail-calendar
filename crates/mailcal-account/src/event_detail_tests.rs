use std::collections::BTreeSet;

use engine_core::{
    calendar::{
        Alert, Event, Frequency, Location, Participant, ParticipantRole, ParticipationStatus,
        Recurrence, RecurrenceRule, Trigger,
    },
    ids::{CalendarId, EventId, Uid},
    membership::Memberships,
    time::{CalendarDate, CalendarDateTime, Duration, LocalDateTime, TimeZoneId},
};

use super::{InviteeEditability, project_event_detail};

const READ_ONLY: InviteeEditability = InviteeEditability::ReadOnly;
use crate::recurrence_shape::{EventRecurrence, RecurrenceFrequency};

fn base(start: CalendarDateTime) -> Event {
    Event::new(
        EventId::try_from("/cal/e.ics").unwrap(),
        Uid::new("e@h").unwrap(),
        Memberships::of_one(CalendarId::try_from("work").unwrap()),
        start,
    )
}

fn amsterdam(local: &str) -> CalendarDateTime {
    CalendarDateTime::Zoned {
        local: local.parse().unwrap(),
        zone: TimeZoneId::iana("Europe/Amsterdam").unwrap(),
    }
}

#[test]
fn a_zoned_timed_event_projects_its_own_wall_clock_and_zone() {
    let mut event = base(amsterdam("2026-01-05T09:30:00"));
    event.title = "Standup".to_owned();
    event.duration = Duration::from_parts(0, 0, 0, 30, 0, 0).unwrap();

    let detail = project_event_detail("acct", &event, true, None, READ_ONLY);
    assert!(!detail.all_day);
    assert_eq!(detail.timezone, "Europe/Amsterdam");
    assert_eq!(detail.start, "2026-01-05T09:30:00");
    assert_eq!(detail.end, "2026-01-05T10:00:00");
    assert_eq!(detail.calendar, "work");
    assert_eq!(detail.account, "acct");
    assert!(detail.can_write);
}

#[test]
fn end_stays_correct_across_a_spring_forward_transition() {
    // 2026-03-29 the Netherlands springs forward at 02:00 → 03:00. An event 01:30 + 1h ends
    // at 03:30 wall clock, not 02:30 (which does not exist). A naive wall-clock add is wrong;
    // resolving to the instant and back is not.
    let mut event = base(amsterdam("2026-03-29T01:30:00"));
    event.duration = Duration::from_parts(0, 0, 1, 0, 0, 0).unwrap();
    let detail = project_event_detail("acct", &event, true, None, READ_ONLY);
    assert_eq!(detail.end, "2026-03-29T03:30:00");
}

#[test]
fn an_all_day_event_projects_dates_with_an_exclusive_end() {
    let mut event = base(CalendarDateTime::Date(
        CalendarDate::new(2026, 4, 1).unwrap(),
    ));
    event.title = "Vrij".to_owned();
    event.duration = Duration::from_parts(0, 1, 0, 0, 0, 0).unwrap();

    let detail = project_event_detail("acct", &event, true, None, READ_ONLY);
    assert!(detail.all_day);
    assert_eq!(detail.timezone, "");
    assert_eq!(detail.start, "2026-04-01");
    assert_eq!(detail.end, "2026-04-02", "a one-day event ends on the 2nd");
}

#[test]
fn an_all_day_end_crosses_a_month_boundary() {
    // Midnight-UTC + N days must roll the month/year, not clamp at 31.
    let mut event = base(CalendarDateTime::Date(
        CalendarDate::new(2026, 1, 30).unwrap(),
    ));
    event.duration = Duration::from_parts(0, 3, 0, 0, 0, 0).unwrap();
    let detail = project_event_detail("acct", &event, true, None, READ_ONLY);
    assert_eq!(detail.end, "2026-02-02");
}

#[test]
fn a_reminder_recurrence_location_and_notes_are_summarized() {
    let mut event = base(amsterdam("2026-01-05T09:30:00"));
    event.duration = Duration::from_parts(0, 0, 0, 30, 0, 0).unwrap();
    event.description = Some("bring the roadmap".to_owned());
    event.locations = vec![Location::named("Room 2")];
    event.alerts = vec![Alert::display(Trigger::before_start(
        Duration::from_parts(0, 0, 0, 15, 0, 0).unwrap(),
    ))];
    event.recurrence = Some(Recurrence::from_rule(RecurrenceRule::new(
        Frequency::Weekly,
    )));

    let detail = project_event_detail("acct", &event, true, None, READ_ONLY);
    assert_eq!(detail.reminder_minutes, Some(15));
    assert!(matches!(
        detail.recurrence,
        Some(EventRecurrence::Simple(ref simple))
            if simple.frequency == RecurrenceFrequency::Weekly
    ));
    assert!(detail.is_recurring);
    assert_eq!(detail.location.as_deref(), Some("Room 2"));
    assert_eq!(detail.notes.as_deref(), Some("bring the roadmap"));
}

#[test]
fn a_reminder_of_a_day_before_is_reported_in_minutes() {
    let mut event = base(amsterdam("2026-01-05T09:30:00"));
    event.alerts = vec![Alert::display(Trigger::before_start(
        Duration::from_parts(0, 1, 0, 0, 0, 0).unwrap(),
    ))];
    let detail = project_event_detail("acct", &event, true, None, READ_ONLY);
    assert_eq!(detail.reminder_minutes, Some(1440), "one day before");
}

#[test]
fn the_events_participants_reach_the_detail_as_an_attendee_list() {
    // The join: `event_attendees` has its own tests in `mailcal-viewmodel`; this pins that the
    // projection actually calls it, which no test there can see.
    let mut event = base(amsterdam("2026-01-05T09:30:00"));
    let mut owner = Participant::attendee("chair@example.com");
    owner.roles = BTreeSet::from([ParticipantRole::Owner]);
    let mut guest = Participant::attendee("guest@example.com");
    guest.participation_status = ParticipationStatus::Accepted;
    event.participants = vec![guest, owner];

    let detail = project_event_detail("acct", &event, true, None, READ_ONLY);
    let addresses: Vec<_> = detail
        .attendees
        .iter()
        .map(|attendee| attendee.email.as_str())
        .collect();
    assert_eq!(addresses, ["chair@example.com", "guest@example.com"]);
}

#[test]
fn a_plain_floating_event_has_no_reminder_recurrence_or_extras() {
    let mut event = base(CalendarDateTime::Floating(
        LocalDateTime::new(2026, 5, 1, 8, 0, 0).unwrap(),
    ));
    event.duration = Duration::from_parts(0, 0, 1, 30, 0, 0).unwrap();
    let detail = project_event_detail("acct", &event, false, None, READ_ONLY);
    assert_eq!(detail.timezone, "", "a floating event has no zone");
    assert_eq!(detail.start, "2026-05-01T08:00:00");
    assert_eq!(detail.end, "2026-05-01T09:30:00");
    assert!(detail.reminder_minutes.is_none());
    assert!(detail.recurrence.is_none());
    assert!(!detail.is_recurring);
    assert!(detail.location.is_none() && detail.notes.is_none());
    assert!(!detail.can_write);
}
