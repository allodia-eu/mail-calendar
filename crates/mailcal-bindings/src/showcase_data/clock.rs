//! The clock the showcase dataset is dated from, and the one conversion every seed shares.
//!
//! Split out of the seeds themselves because it answers a different question: not *what* the
//! sample mail says, but *when* it claims to have arrived — which is what decides whether two
//! captures of an unchanged app produce the same pixels.

use engine_api::{resolve_instant_in, to_local};
use engine_core::time::{CalendarDateTime, LocalDateTime, TimeZoneId, UtcDateTime};
use time::{Date, Month, OffsetDateTime};

/// The hour of the wall clock the showcase seeds itself from — **today's date, at a fixed time of
/// day**, deliberately not the moment of capture.
///
/// Every screenshot is content-addressed, so a capture that differs only because the clock moved
/// republishes the whole set and makes a real interface change indistinguishable from the hour it
/// was photographed. Seeding from the real `now` did exactly that: the mailbox renders a same-day
/// message as its *time*, so three consecutive captures of an unchanged app produced three
/// different images (`09:03`, `09:10`, `09:13`).
///
/// The **date** stays today's on purpose. A client formats a timestamp against the *device's*
/// clock, so a seed pinned to some fixed calendar date would render the newest mail as a weekday
/// name rather than a time, and the calendar grid — which opens on the real current week — would
/// come up empty. Rows older than today therefore still carry a weekday that tracks the capture
/// date; none of those is visible in the documentation set, whose screens sit under a sheet.
///
/// `09:41` is the same wall clock the iOS simulator's status bar and Android's demo-mode status
/// bar are pinned to in `scripts/dev/showcase.sh`, so a screenshot answers "what time is it?"
/// once.
const PINNED_HOUR: u8 = 9;
/// The minute half of the pinned wall clock — see `PINNED_HOUR`.
const PINNED_MINUTE: u8 = 41;

/// Today at the pinned wall clock in `zone`, as the instant every seed counts back from.
///
/// Falls back to the real clock if the zone cannot be resolved: a screenshot dataset is not worth
/// refusing to launch over, and the only thing lost is the churn this exists to pin down.
pub(crate) fn seeded_now(zone: &TimeZoneId) -> OffsetDateTime {
    let real = OffsetDateTime::now_utc();
    pin_to_wall_clock(real, zone).unwrap_or(real)
}

/// The conversion behind `seeded_now`: today's local date in `zone` → the pinned wall clock →
/// back to a UTC instant. `None` if any step is unrepresentable, which the caller reads as
/// "leave the real clock alone".
fn pin_to_wall_clock(real: OffsetDateTime, zone: &TimeZoneId) -> Option<OffsetDateTime> {
    let today = to_local(ago(real, 0), zone).ok()?;
    let wall = LocalDateTime::new(
        today.year(),
        today.month(),
        today.day(),
        PINNED_HOUR,
        PINNED_MINUTE,
        0,
    )
    .ok()?;
    // Floating, not zoned: the wall clock is already expressed in `zone`, and resolving it there
    // is what makes the pin survive a DST boundary instead of drifting an hour twice a year.
    let instant = resolve_instant_in(&CalendarDateTime::Floating(wall), zone).ok()?;
    let date = Date::from_calendar_date(
        instant.year(),
        Month::try_from(instant.month()).ok()?,
        instant.day(),
    )
    .ok()?;
    Some(
        date.with_hms(instant.hour(), instant.minute(), instant.second())
            .ok()?
            .assume_utc(),
    )
}

/// An instant `minutes` before `now`, as the engine's `UtcDateTime` (via its RFC 3339 parse).
pub(super) fn ago(now: OffsetDateTime, minutes: i64) -> UtcDateTime {
    let dt = now - time::Duration::minutes(minutes);
    format!(
        "{:04}-{:02}-{:02}T{:02}:{:02}:{:02}Z",
        dt.year(),
        u8::from(dt.month()),
        dt.day(),
        dt.hour(),
        dt.minute(),
        dt.second(),
    )
    .parse()
    .expect("a formatted instant parses")
}
