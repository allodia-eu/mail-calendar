//! How a showcase calendar and its events are built — the shared constructors the per-locale
//! seeds call, kept apart from [`super`]'s locale dispatch so neither file grows past the
//! 500-line limit.
//!
//! The three calendars (Work, Personal, Family) and their ids, palette hues and weekday
//! anchors are identical in every locale; only the names and event titles are translated. So
//! everything that decides *shape* lives here, and everything that decides *language* lives in
//! the locale seeds.

use engine_core::{
    calendar::{
        Calendar, Conference, ConferenceKey, ConferenceOrigin, ConferenceService, ConferenceUri,
        EntryPoint, Event, NativeState,
    },
    ids::{CalendarId, EventId, Uid},
    membership::Memberships,
    time::{CalendarDate, CalendarDateTime, Duration, LocalDateTime, TimeZoneId},
};
use time::OffsetDateTime;

/// Builds one showcase calendar with its fixed id, palette hue, and default flag.
pub(super) fn showcase_calendar(kind: Cal, name: &str, is_default: bool) -> Calendar {
    let mut calendar = Calendar::new(kind.id(), name);
    // A server colour, snapped to the nearest palette entry by the core's colour-defaults rule
    // (`docs/calendar.md`). These are exact palette hexes, so each lands on itself.
    calendar.color = Some(kind.color().to_owned());
    calendar.is_default = is_default;
    calendar
}

/// One of the three showcase calendars. The seed splits its events across them so the grid shows
/// a real work-and-private-life mix in distinct colours; each maps to a stable id and a fixed
/// palette hue, identical across both locales.
#[derive(Clone, Copy)]
pub(super) enum Cal {
    /// Work — standups, reviews, interviews, the board. The default calendar for new events.
    Work,
    /// Personal — gym, lunches, appointments, evenings out.
    Personal,
    /// Family — school runs, kids' activities, visits, birthdays.
    Family,
}

impl Cal {
    /// The calendar's stable provider id, shared across both locales.
    pub(super) fn id(self) -> CalendarId {
        let key = match self {
            Cal::Work => "showcase-work",
            Cal::Personal => "showcase-personal",
            Cal::Family => "showcase-family",
        };
        CalendarId::try_from(key).expect("valid calendar id")
    }

    /// The palette hue this calendar paints in: Work blue, Personal green, Family magenta — three
    /// well-separated hues from the shared palette (`mailcal-viewmodel::calendar::color`).
    fn color(self) -> &'static str {
        match self {
            Cal::Work => "#2f6fa8",     // palette: blue
            Cal::Personal => "#3f8f55", // palette: green
            Cal::Family => "#a64f8e",   // palette: magenta
        }
    }
}

/// The three showcase calendars' display names in one locale.
pub(super) struct CalendarNames {
    /// The work calendar's name ("Work" / "Werk").
    pub(super) work: &'static str,
    /// The personal calendar's name ("Personal" / "Persoonlijk").
    pub(super) personal: &'static str,
    /// The family calendar's name ("Family" / "Familie").
    pub(super) family: &'static str,
}

/// Weekday anchors, Monday = 0 … Sunday = 6. The event helpers place work *within the current
/// Monday-started week* rather than N days from today, so the visible week is full whatever day a
/// screenshot is taken — a Sunday capture would otherwise leave Monday–Saturday empty.
pub(super) const MON: i64 = 0;
/// Tuesday.
pub(super) const TUE: i64 = 1;
/// Wednesday.
pub(super) const WED: i64 = 2;
/// Thursday.
pub(super) const THU: i64 = 3;
/// Friday.
pub(super) const FRI: i64 = 4;
/// Saturday.
pub(super) const SAT: i64 = 5;
/// Sunday.
pub(super) const SUN: i64 = 6;

/// A timed event of `minutes`.
///
/// The duration is **not** optional here, though [`Event`] defaults it to zero: a zero-length event
/// is floored to the grid's minimum segment, so every showcase meeting used to render as an
/// identical 15-minute sliver regardless of what it was. A calendar screenshot whose every event is
/// the same height is not showing the product.
pub(super) fn event(
    key: &str,
    title: &str,
    cal: Cal,
    start: CalendarDateTime,
    minutes: u64,
) -> Event {
    let mut event = Event::new(
        EventId::try_from(key).expect("valid event id"),
        Uid::new(format!("{key}@allodia.local")).expect("valid uid"),
        Memberships::of_one(cal.id()),
        start,
    );
    event.title = title.to_owned();
    event.duration =
        Duration::from_parts(0, 0, 0, minutes, 0, 0).expect("a valid showcase duration");
    event
}

/// An all-day event on calendar `cal`, covering `days` whole days from `start_weekday`
/// (Monday = 0 … Sunday = 6) of the current week.
///
/// All-day is **zoneless** — a bare calendar date, the same date in every zone — so it is built
/// from [`CalendarDateTime::Date`] and never from a midnight instant. Localising it would drag it a
/// day either way (see `calendar-semantics.md`).
pub(super) fn all_day_event(
    key: &str,
    title: &str,
    cal: Cal,
    now: OffsetDateTime,
    start_weekday: i64,
    days: u64,
) -> Event {
    let dt = now + time::Duration::days(weekday_delta(now, start_weekday));
    let date = CalendarDate::new(dt.year(), u8::from(dt.month()), dt.day())
        .expect("a valid showcase date");
    let mut event = Event::new(
        EventId::try_from(key).expect("valid event id"),
        Uid::new(format!("{key}@allodia.local")).expect("valid uid"),
        Memberships::of_one(cal.id()),
        CalendarDateTime::Date(date),
    );
    event.title = title.to_owned();
    event.duration = Duration::from_parts(0, days, 0, 0, 0, 0).expect("a valid showcase duration");
    event
}

/// The day offset from *today* to `weekday` (Monday = 0 … Sunday = 6) in the current
/// Monday-started week — negative earlier in the week, positive later. Anchoring events to a
/// weekday (not an offset-from-today) keeps the whole visible week populated whatever day the
/// screenshot is taken.
fn weekday_delta(now: OffsetDateTime, weekday: i64) -> i64 {
    weekday - i64::from(now.weekday().number_days_from_monday())
}

/// A wall-clock at `hour:minute` on `weekday` (Monday = 0 … Sunday = 6) of the current week,
/// Europe/Amsterdam.
pub(super) fn zoned_wd(
    now: OffsetDateTime,
    weekday: i64,
    hour: u8,
    minute: u8,
) -> CalendarDateTime {
    zoned(now, weekday_delta(now, weekday), hour, minute)
}

/// A `day_offset`-days-from-now wall-clock at `hour:minute`, in Europe/Amsterdam.
fn zoned(now: OffsetDateTime, day_offset: i64, hour: u8, minute: u8) -> CalendarDateTime {
    let dt = now + time::Duration::days(day_offset);
    let local = LocalDateTime::new(dt.year(), u8::from(dt.month()), dt.day(), hour, minute, 0)
        .expect("valid local datetime");
    CalendarDateTime::Zoned {
        local,
        zone: TimeZoneId::iana("Europe/Amsterdam").expect("valid zone"),
    }
}

/// The Monday design sync's provider key, the same in every locale's seed.
const MEETING_EVENT_KEY: &str = "ev-mon-designsync";

/// Which showcase event a screenshot run opens to show a video meeting in the event detail: Join
/// first, then the meeting ID and the number to dial in on (`docs/conferencing.md`).
///
/// The twin of [`super::ShowcaseInvitation`], for the same reason: the target is one fact, held in
/// the core, so every client opens the *same* event and a reworded seed cannot leave one of them
/// opening nothing.
#[derive(uniffi::Record, Clone, Debug, PartialEq, Eq)]
pub struct ShowcaseMeeting {
    /// The id of the account whose calendar holds the event (its owner address).
    pub account: String,
    /// The event's provider key, as it appears on a grid block and an agenda row.
    pub event_key: String,
}

/// The [`ShowcaseMeeting`] a host opens for the meeting screenshot. Takes no locale: only the
/// event's title is translated, never its key or its account.
#[uniffi::export]
#[must_use]
pub fn showcase_meeting() -> ShowcaseMeeting {
    ShowcaseMeeting {
        account: super::REPLY_ACCOUNT.to_owned(),
        event_key: MEETING_EVENT_KEY.to_owned(),
    }
}

/// Gives the Monday design sync, the same meeting in every locale, a Teams meeting with its
/// meeting ID and a dial-in number, so the detail shows Join and the grid its camera glyph
/// (`docs/conferencing.md`). Reserved numbers and a made-up meeting: nothing here dials anyone.
pub(super) fn with_meeting(events: &mut [Event]) {
    let Some(sync) = events
        .iter_mut()
        .find(|event| event.id.key().as_str() == MEETING_EVENT_KEY)
    else {
        return;
    };
    let join = ConferenceUri::parse("https://teams.microsoft.com/meet/990000000001?p=ShowcasePass")
        .expect("a valid join link");
    let mut phone = EntryPoint::at(ConferenceUri::parse("tel:+31205550100").expect("a number"));
    phone.label = Some("+31 20 555 0100".to_owned());
    phone.access.access_code = Some("990 000 000#".to_owned());
    sync.conferences = vec![Conference {
        key: ConferenceKey::new("onlineMeeting"),
        service: Some(ConferenceService::Teams),
        origin: ConferenceOrigin::Native {
            removable: false,
            state: NativeState::Ready,
        },
        entry_points: vec![EntryPoint::at(join), phone],
        label: None,
        meeting_id: Some("990 000 000 001".to_owned()),
        notes: None,
    }];
}
