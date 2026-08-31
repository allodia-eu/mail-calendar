//! The account, app and event builders the calendar tests construct their world from.
//!
//! Split from `calendar.rs` (which holds [`super::calendar::CalendarFake`] itself) when the two
//! together crossed the 500-line limit. The seam is the provider versus the fixtures built on it.

use std::sync::{Arc, Mutex};

use engine_api::{AccountId, EmailAddress, Engine, TimeZoneId};
use engine_core::{
    calendar::Event,
    ids::{CalendarId, EventId, Uid},
    membership::Memberships,
    raw::RawIcal,
    time::{CalendarDateTime, LocalDateTime},
    version::ETag,
};

use super::{RecordingObserver, calendar::CalendarFake};
use crate::{Account, App, Surface, Telemetry, TimeZoneInit};

/// Wraps `provider` as a calendar-only account `id` (no mail providers).
pub(crate) fn calendar_account(id: &str, provider: CalendarFake) -> Account<CalendarFake> {
    Account {
        id: AccountId::try_from(id).unwrap(),
        providers: Vec::new(),
        calendar_providers: vec![provider],
        contact_providers: Vec::new(),
        identity: EmailAddress::new(format!("me@{id}.local")),
    }
}

/// Builds an in-memory app over calendar-only `accounts`, recording signalled surfaces.
pub(crate) fn calendar_app(
    accounts: Vec<Account<CalendarFake>>,
    surfaces: &Arc<Mutex<Vec<Surface>>>,
) -> App<CalendarFake> {
    calendar_app_on(Engine::open_in_memory().unwrap(), accounts, surfaces)
}

/// The same, over a caller-supplied engine: so a test can open **two** apps over one on-disk
/// store and see what the second one's boot actually paints. An in-memory engine cannot show that:
/// the store dies with the app, and "the launch after this one" is the whole question.
pub(crate) fn calendar_app_on(
    engine: Engine,
    accounts: Vec<Account<CalendarFake>>,
    surfaces: &Arc<Mutex<Vec<Surface>>>,
) -> App<CalendarFake> {
    App::new(
        engine,
        accounts,
        TimeZoneInit {
            device_zone: TimeZoneId::utc(),
            prefs_path: None,
        },
        None,
        Arc::new(RecordingObserver {
            surfaces: Arc::clone(surfaces),
        }),
        Telemetry::off(None),
    )
}

/// A **patchable** stored event: one carrying the `raw_ical` an update patches and the
/// `ETag` that guards it: a recurring, alarmed, zoned event with an `X-` property, i.e.
/// everything the six-property create builder would delete if an update went through it.
///
/// [`event_from_today`] and friends build an `Event` straight from the projection, with no
/// raw document behind it; fine for reads, but an update over one is precisely the thing
/// that cannot work, so an edit test needs this instead.
///
/// Built from `engine_core` types rather than parsed: `mailcal-app` has no `provider-caldav`
/// dependency and must not grow one (the iCal glue lives in `mailcal-account`).
pub(crate) fn stored_event(key: &str, etag: &str) -> Event {
    let ical = format!(
        "BEGIN:VCALENDAR\r\nBEGIN:VEVENT\r\nUID:{key}@h\r\nDTSTAMP:20260101T080000Z\r\n\
         DTSTART;TZID=Europe/Amsterdam:20260105T093000\r\n\
         DTEND;TZID=Europe/Amsterdam:20260105T100000\r\n\
         RRULE:FREQ=WEEKLY;BYDAY=MO;COUNT=20\r\nSUMMARY:Standup\r\nX-KEEP:me\r\n\
         BEGIN:VALARM\r\nACTION:DISPLAY\r\nTRIGGER:-PT15M\r\nEND:VALARM\r\n\
         END:VEVENT\r\nEND:VCALENDAR\r\n"
    );
    let mut event = Event::new(
        EventId::try_from(key).unwrap(),
        Uid::new(format!("{key}@h")).unwrap(),
        Memberships::of_one(CalendarId::try_from("cal").unwrap()),
        // The projection's start must agree with the document's DTSTART: the patcher
        // renders a move in *this* value's form.
        CalendarDateTime::Zoned {
            local: LocalDateTime::new(2026, 1, 5, 9, 30, 0).unwrap(),
            zone: TimeZoneId::iana("Europe/Amsterdam").unwrap(),
        },
    );
    // ...and its **end** must agree with the document's `DTEND` for the same reason: a resize is
    // rendered against the projected duration, so a fixture that projects a zero-length event
    // would silently make every resize test assert the wrong wall clock.
    event.duration = engine_core::time::Duration::from_parts(0, 0, 0, 30, 0, 0).unwrap();
    event.raw_ical = Some(engine_core::raw::RawIcal::new(ical));
    event.revisions.etag = Some(engine_core::version::ETag::new(etag));
    event
}

/// A **weekly** event starting `day_offset` days from today at `hour`:00 UTC, lasting `minutes`.
///
/// Relative to today for the same reason as [`event_from_today`]: a recurring fixture pinned to
/// a fixed date drops out of the rolling horizon and the test starts failing on a date nobody
/// chose. Deliberately UTC-zoned: the *zone* rule for an occurrence token is pinned in
/// `mailcal_account::calendar_drag`, and what this fixture is for is the wiring.
pub(crate) fn weekly_event_from_today(key: &str, day_offset: i64, hour: u8, minutes: u64) -> Event {
    let mut event = event_from_today(key, day_offset, hour, minutes);
    event.recurrence = Some(engine_core::calendar::Recurrence::from_rule(
        engine_core::calendar::RecurrenceRule::new(engine_core::calendar::Frequency::Weekly),
    ));
    event
}

/// A weekly event one of whose occurrences the user has **already moved**: the shape that
/// tells an occurrence's *identity* apart from where it currently sits.
///
/// The occurrence `weeks_out` weeks from the series start is overridden to `moved_to_hour`,
/// so its recurrence id and its start are different instants. A fixture where they agree
/// cannot fail the test it exists for.
pub(crate) fn weekly_event_with_a_moved_occurrence(
    key: &str,
    day_offset: i64,
    hour: u8,
    minutes: u64,
    weeks_out: i64,
    moved_to_hour: u8,
) -> Event {
    use engine_core::{calendar::RecurrenceOverride, patch::PatchObject};

    let mut event = weekly_event_from_today(key, day_offset, hour, minutes);
    let original = occurrence_wall_clock_of(&event, weeks_out);
    let moved = LocalDateTime::new(
        original.year(),
        original.month(),
        original.day(),
        moved_to_hour,
        0,
        0,
    )
    .unwrap();
    let patch = PatchObject::new(vec![(
        "start".to_owned(),
        serde_json::Value::String(datetime_text(moved)),
    )])
    .unwrap();
    event
        .recurrence
        .as_mut()
        .expect("weekly")
        .overrides
        .insert(original, RecurrenceOverride::Patch(patch));
    event
}

/// The wall clock of the occurrence `weeks_out` weeks after `event`'s start: the recurrence
/// id a weekly series generates for it.
pub(crate) fn occurrence_wall_clock_of(event: &Event, weeks_out: i64) -> LocalDateTime {
    let start = match &event.start {
        CalendarDateTime::Floating(local) | CalendarDateTime::Zoned { local, .. } => *local,
        CalendarDateTime::Date(date) => {
            LocalDateTime::new(date.year(), date.month(), date.day(), 0, 0, 0).unwrap()
        }
    };
    let day = mailcal_viewmodel::calendar::days::date_at(
        mailcal_viewmodel::calendar::days::from_civil(start.year(), start.month(), start.day())
            + weeks_out * 7,
    );
    LocalDateTime::new(
        day.year(),
        day.month(),
        day.day(),
        start.hour(),
        start.minute(),
        start.second(),
    )
    .unwrap()
}

/// `YYYY-MM-DDTHH:MM:SS`, the form a JSCalendar override patch states a start in.
fn datetime_text(local: LocalDateTime) -> String {
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

/// An event on `day_offset` days from today, at `hour`:00 UTC, lasting `minutes`.
///
/// Positioned relative to **today** on purpose: the rolling horizon is, so a fixture pinned
/// to a fixed date would drift out of the materialized window and the test would start
/// failing on a date nobody chose.
pub(crate) fn event_from_today(key: &str, day_offset: i64, hour: u8, minutes: u64) -> Event {
    let today = mailcal_viewmodel::calendar::days::date_at(
        i64::try_from(
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("after the epoch")
                .as_secs()
                / 86_400,
        )
        .expect("in range")
            + day_offset,
    );
    let mut event = Event::new(
        EventId::try_from(key).unwrap(),
        Uid::new(format!("{key}@h")).unwrap(),
        Memberships::of_one(CalendarId::try_from("cal").unwrap()),
        CalendarDateTime::utc(
            LocalDateTime::new(today.year(), today.month(), today.day(), hour, 0, 0).unwrap(),
        ),
    );
    event.duration = engine_core::time::Duration::from_parts(0, 0, 0, minutes, 0, 0).unwrap();
    event
}
