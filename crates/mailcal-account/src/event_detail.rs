//! The event-detail read: a rich projection of a single stored event, for the detail view and
//! for the editor to prefill from: the read counterpart of the [`build_event_draft`] /
//! [`build_event_patch`] write builders in [`crate::calendar`].
//!
//! It lives here, the provider-format glue layer, because it reaches into `engine-core` domain
//! types (a reminder's trigger, a recurrence frequency) the same way the write builders do.
//! The app layer (`mailcal-app`) stays free of that: it reads the stored event through the
//! `engine-api` facade and hands it here, and only the flat [`EventDetail`] (plain strings and
//! options) crosses back to it and on to the client.
//!
//! [`build_event_draft`]: crate::build_event_draft
//! [`build_event_patch`]: crate::build_event_patch

use engine_api::{resolve_instant, to_local};
use engine_core::{
    calendar::{Event, RelativeTo, Trigger},
    time::{CalendarDate, CalendarDateTime, LocalDateTime, TimeZoneId, UtcDateTime},
};
use mailcal_viewmodel::{
    EventAttendee,
    calendar::days::{date_at, from_civil},
    event_attendees,
};

use crate::{
    recurrence_shape::{EventRecurrence, describe_recurrence},
    repeat_summary::{RepeatSummary, summarize_repeat},
};

/// Which occurrence of a series a detail is being asked for.
///
/// The times are the ones the **expander** produced for that instant, not the master's: a
/// series' own start is the *first* occurrence's, so projecting it for every occurrence is how
/// a September Tuesday comes to read as an August one.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DetailOccurrence {
    /// The token that named it, echoed back onto the detail unchanged.
    pub token: String,
    /// The occurrence's own start, as a wall clock in the event's own terms.
    pub start: LocalDateTime,
    /// Its own end, same terms; **exclusive** for an all-day event, like the master's.
    pub end: LocalDateTime,
}

/// A single event's full detail, for the detail view and to prefill the editor.
///
/// Times are the event's **own wall clock**: the form the editor edits and
/// `Intent::UpdateEvent` expects, so a save cannot silently convert a zoned or all-day event. A
/// client localises the display; a same-zone event reads identically.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EventDetail {
    /// The owning account's id.
    pub account: String,
    /// The event's provider key.
    pub key: String,
    /// The event's calendar key (`CalendarRow.id`); resolve its name and colour from the page
    /// snapshot's `calendars` the client already holds.
    pub calendar: String,
    /// The title (may be empty: the client shows its own placeholder).
    pub title: String,
    /// Whether this is an all-day event.
    pub all_day: bool,
    /// The event's own IANA zone, or empty for a floating or all-day event. A display hint; the
    /// wall clock in `start`/`end` is already in this zone.
    pub timezone: String,
    /// The start as the event's own wall clock: `YYYY-MM-DDTHH:MM:SS`, or a bare date
    /// `YYYY-MM-DD` when all-day.
    pub start: String,
    /// The end, same terms as `start`, for an all-day event the date is **exclusive**.
    pub end: String,
    /// The location, if any.
    pub location: Option<String>,
    /// The notes/description, if any.
    pub notes: Option<String>,
    /// Minutes before the start of the first "N before start" reminder, or `None` if the event
    /// has none we summarise. Read-only: no client offers to change a reminder yet.
    pub reminder_minutes: Option<i32>,
    /// The repeat rule as a client can show and edit it, or `None` if the event does not
    /// repeat. [`EventRecurrence::Complex`] means it repeats on a rule the editor does not
    /// model; say so, and do not offer to change it.
    pub recurrence: Option<EventRecurrence>,
    /// The rule as the parts a **sentence** needs (the rhythm and what ends it) or `None` when
    /// the event has no rule, or one too rich to state exactly (then say only that it repeats).
    ///
    /// Decided here so four clients cannot disagree about what a rule means; the words stay each
    /// client's, because localisation is.
    pub repeat_summary: Option<RepeatSummary>,
    /// Whether the event recurs (a master with a rule, or an overridden instance): so the editor
    /// can tell the user an edit applies to the whole series.
    pub is_recurring: bool,
    /// Whether the owning account's calendar can be written; gates the edit/delete affordances.
    pub can_write: bool,
    /// The occurrence this detail describes, as the token that named it, or empty when it
    /// describes the **series**, which is what an agenda row and a one-off event always do.
    ///
    /// Echoed from what the core **resolved**, never from what the client sent, and that is the
    /// point: a token that no longer names an occurrence (the series changed underneath it)
    /// comes back empty, and the times above are the series' again. So a client puts its scope
    /// question exactly when this is non-empty, and never offers *This event* against times
    /// that belong to another one.
    pub occurrence_start: String,
    /// Everyone on the event, organiser first; empty for an appointment nobody was invited to.
    ///
    /// **Attacker-controlled plain text**, projected by
    /// [`mailcal_viewmodel::event_attendees`]: one row per address, an unanswered organizer
    /// counted as attending, and every name and address sanitised. Attendees are **read-only**
    /// throughout the product; editing them means sending iTIP updates, which is a separate
    /// feature: so an editor shows this list without offering to change it.
    pub attendees: Vec<EventAttendee>,
}

/// Projects a stored [`Event`] into the flat [`EventDetail`] a client renders. Pure: the async
/// store read that supplies `event` lives in the app layer, so this is unit-testable without an
/// engine.
///
/// `occurrence` is the instance the user opened, when they opened one. `None` projects the
/// series, which is what an agenda row and a one-off event ask for.
#[must_use]
pub fn project_event_detail(
    account: &str,
    event: &Event,
    can_write: bool,
    occurrence: Option<&DetailOccurrence>,
) -> EventDetail {
    let (timezone, start, end) = wall_clock_bounds(event, occurrence);
    let recurrence = event.recurrence.as_ref().and_then(describe_recurrence);
    EventDetail {
        account: account.to_owned(),
        key: event.id.key().as_str().to_owned(),
        calendar: event
            .calendars
            .iter()
            .next()
            .map(|id| id.key().as_str().to_owned())
            .unwrap_or_default(),
        title: event.title.clone(),
        all_day: event.start.is_all_day(),
        timezone,
        start,
        end,
        location: event
            .locations
            .iter()
            .find_map(|location| location.name.clone()),
        notes: event.description.clone(),
        reminder_minutes: reminder_minutes(event),
        repeat_summary: match &recurrence {
            Some(EventRecurrence::Simple(rule)) => summarize_repeat(rule, start_date(event)),
            Some(EventRecurrence::Complex) | None => None,
        },
        recurrence,
        is_recurring: event.recurrence.is_some() || event.recurrence_id.is_some(),
        can_write,
        occurrence_start: occurrence.map(|at| at.token.clone()).unwrap_or_default(),
        attendees: event_attendees(event),
    }
}

/// The event's `(timezone, start, end)` as wall-clock strings in the event's own form: a bare
/// date for an all-day event (with an **exclusive** end), a `YYYY-MM-DDTHH:MM:SS` wall clock
/// otherwise. The zone is empty for a floating or all-day event.
///
/// With an `occurrence`, the times are that instance's; the **zone is still the series'**,
/// because an occurrence keeps the clock its master is read in.
fn wall_clock_bounds(
    event: &Event,
    occurrence: Option<&DetailOccurrence>,
) -> (String, String, String) {
    let zone = match &event.start {
        CalendarDateTime::Date(_) | CalendarDateTime::Floating(_) => String::new(),
        CalendarDateTime::Zoned { zone, .. } => zone.as_str().to_owned(),
    };
    if let Some(at) = occurrence {
        // The expander already applied whatever the user did to this instance, so both edges
        // come from it rather than from the master plus arithmetic.
        return if event.start.is_all_day() {
            (zone, date_of(at.start), date_of(at.end))
        } else {
            (zone, datetime_str(at.start), datetime_str(at.end))
        };
    }
    match &event.start {
        CalendarDateTime::Date(date) => (
            zone,
            date_str(*date),
            all_day_end(*date, event.duration.days()),
        ),
        CalendarDateTime::Floating(local) | CalendarDateTime::Zoned { local, .. } => {
            let start = datetime_str(*local);
            let end = end_wall_clock(event).map_or_else(|| start.clone(), datetime_str);
            (zone, start, end)
        }
    }
}

/// A wall clock's date half, for an all-day event whose bounds are bare dates.
fn date_of(local: LocalDateTime) -> String {
    format!(
        "{:04}-{:02}-{:02}",
        local.year(),
        local.month(),
        local.day()
    )
}

/// The civil date an event starts on; what a repeat rule falls back to for every part it does
/// not name itself: its weekday, its day of the month, its month.
fn start_date(event: &Event) -> CalendarDate {
    match &event.start {
        CalendarDateTime::Date(date) => *date,
        CalendarDateTime::Floating(local) | CalendarDateTime::Zoned { local, .. } => {
            date_at(from_civil(local.year(), local.month(), local.day()))
        }
    }
}

/// The exclusive end date of an all-day event `days` long: midnight UTC plus that many whole
/// days is pure civil arithmetic (a bare date has no DST), so it lands on the right day.
fn all_day_end(start: CalendarDate, days: u64) -> String {
    UtcDateTime::new(start.year(), start.month(), start.day(), 0, 0, 0)
        .ok()
        .and_then(|midnight| {
            midnight.checked_add(core::time::Duration::from_secs(days.saturating_mul(86_400)))
        })
        .map_or_else(
            || date_str(start),
            |end| format!("{:04}-{:02}-{:02}", end.year(), end.month(), end.day()),
        )
}

/// The end of a **timed** event as a wall clock in its own zone: resolve the start to its
/// absolute instant, add the duration, and read it back in the event's zone: so it is correct
/// across a DST transition, unlike a naive wall-clock addition. A floating event has no zone, so
/// the arithmetic is done "as UTC", which is exactly a wall-clock add (no DST to cross). Returns
/// `None` for an all-day event (handled by [`wall_clock_bounds`]) or if the maths overflows.
///
/// The one approximation: a nominal-day duration component (RFC 5545 `P1D`) is treated as 24h
/// here, which can be off by an hour at a DST boundary for a rare multi-day *timed* event; the
/// common case (a seconds-only duration) is exact.
///
/// Shared with [`crate::calendar_drag`], which needs the same end in the same terms: a drag
/// that resized against a *differently* derived end would move an edge the user did not touch.
pub(crate) fn end_wall_clock(event: &Event) -> Option<LocalDateTime> {
    let duration = event.duration;
    let span = core::time::Duration::new(
        duration
            .days()
            .saturating_mul(86_400)
            .saturating_add(duration.seconds()),
        duration.nanoseconds(),
    );
    match &event.start {
        CalendarDateTime::Zoned { zone, .. } => {
            let end = resolve_instant(&event.start)
                .ok()
                .flatten()?
                .checked_add(span)?;
            to_local(end, zone).ok()
        }
        CalendarDateTime::Floating(local) => {
            let start = UtcDateTime::new(
                local.year(),
                local.month(),
                local.day(),
                local.hour(),
                local.minute(),
                local.second(),
            )
            .ok()?;
            to_local(start.checked_add(span)?, &TimeZoneId::utc()).ok()
        }
        CalendarDateTime::Date(_) => None,
    }
}

/// Minutes before the start of the first "N before start" display reminder, or `None`.
fn reminder_minutes(event: &Event) -> Option<i32> {
    event.alerts.iter().find_map(|alert| match &alert.trigger {
        Trigger::Offset {
            offset,
            relative_to: RelativeTo::Start,
        } if offset.is_before() => {
            let magnitude = offset.magnitude();
            let minutes = magnitude
                .days()
                .saturating_mul(1440)
                .saturating_add(magnitude.seconds() / 60);
            i32::try_from(minutes).ok()
        }
        _ => None,
    })
}

/// `YYYY-MM-DDTHH:MM:SS` for a wall clock.
pub(crate) fn datetime_str(local: LocalDateTime) -> String {
    format!(
        "{:04}-{:02}-{:02}T{:02}:{:02}:{:02}",
        local.year(),
        local.month(),
        local.day(),
        local.hour(),
        local.minute(),
        local.second(),
    )
}

/// `YYYY-MM-DD` for a calendar date.
pub(crate) fn date_str(date: CalendarDate) -> String {
    format!("{:04}-{:02}-{:02}", date.year(), date.month(), date.day())
}

#[cfg(test)]
mod tests {
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

    use super::project_event_detail;
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

        let detail = project_event_detail("acct", &event, true, None);
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
        let detail = project_event_detail("acct", &event, true, None);
        assert_eq!(detail.end, "2026-03-29T03:30:00");
    }

    #[test]
    fn an_all_day_event_projects_dates_with_an_exclusive_end() {
        let mut event = base(CalendarDateTime::Date(
            CalendarDate::new(2026, 4, 1).unwrap(),
        ));
        event.title = "Vrij".to_owned();
        event.duration = Duration::from_parts(0, 1, 0, 0, 0, 0).unwrap();

        let detail = project_event_detail("acct", &event, true, None);
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
        let detail = project_event_detail("acct", &event, true, None);
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

        let detail = project_event_detail("acct", &event, true, None);
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
        let detail = project_event_detail("acct", &event, true, None);
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

        let detail = project_event_detail("acct", &event, true, None);
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
        let detail = project_event_detail("acct", &event, false, None);
        assert_eq!(detail.timezone, "", "a floating event has no zone");
        assert_eq!(detail.start, "2026-05-01T08:00:00");
        assert_eq!(detail.end, "2026-05-01T09:30:00");
        assert!(detail.reminder_minutes.is_none());
        assert!(detail.recurrence.is_none());
        assert!(!detail.is_recurring);
        assert!(detail.location.is_none() && detail.notes.is_none());
        assert!(!detail.can_write);
    }
}
