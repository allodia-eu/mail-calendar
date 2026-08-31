//! Small, locale-aware calendar date helpers.

use jiff::Timestamp;
use time::{Date, Duration, Month, OffsetDateTime, PrimitiveDateTime, Time};

pub(super) fn today() -> Date {
    OffsetDateTime::now_local()
        .unwrap_or_else(|_| OffsetDateTime::now_utc())
        .date()
}

pub(super) fn today_in(zone: &str) -> Date {
    now_in(zone).map_or_else(today, |(date, _)| date)
}

pub(super) fn now_in(zone: &str) -> Option<(Date, u32)> {
    let now = Timestamp::now().in_tz(zone).ok()?;
    let year = i32::from(now.year());
    let month = Month::try_from(u8::try_from(now.month()).ok()?).ok()?;
    let day = u8::try_from(now.day()).ok()?;
    let date = Date::from_calendar_date(year, month, day).ok()?;
    let hour = u32::try_from(now.hour()).ok()?;
    let minute = u32::try_from(now.minute()).ok()?;
    Some((date, hour * 60 + minute))
}

pub(super) fn now_wall() -> PrimitiveDateTime {
    let now = OffsetDateTime::now_local().unwrap_or_else(|_| OffsetDateTime::now_utc());
    PrimitiveDateTime::new(now.date(), now.time())
}

pub(in crate::ui) fn parse_date(value: &str) -> Option<Date> {
    let mut parts = value.split('-');
    let year = parts.next()?.parse().ok()?;
    let month = Month::try_from(parts.next()?.parse::<u8>().ok()?).ok()?;
    let day = parts.next()?.parse().ok()?;
    (parts.next().is_none()).then_some(())?;
    Date::from_calendar_date(year, month, day).ok()
}

pub(super) fn parse_wall(value: &str) -> Option<PrimitiveDateTime> {
    let (date, clock) = value.split_once('T')?;
    let mut parts = clock.split(':');
    let hour = parts.next()?.parse().ok()?;
    let minute = parts.next()?.parse().ok()?;
    let second = parts.next().map_or(Some(0), |part| part.parse().ok())?;
    (parts.next().is_none()).then_some(())?;
    Some(PrimitiveDateTime::new(
        parse_date(date)?,
        Time::from_hms(hour, minute, second).ok()?,
    ))
}

pub(super) fn wall_string(value: PrimitiveDateTime) -> String {
    format!(
        "{:04}-{:02}-{:02}T{:02}:{:02}:{:02}",
        value.year(),
        u8::from(value.month()),
        value.day(),
        value.hour(),
        value.minute(),
        value.second()
    )
}

pub(super) fn add_days(date: Date, days: i64) -> Date {
    date.checked_add(Duration::days(days)).unwrap_or(date)
}

pub(super) fn month_start(date: Date) -> Date {
    Date::from_calendar_date(date.year(), date.month(), 1).unwrap_or(date)
}

pub(super) fn add_months(date: Date, delta: i32) -> Date {
    let zero_based = i32::from(u8::from(date.month())) - 1 + delta;
    let year = date.year() + zero_based.div_euclid(12);
    let month_number = u8::try_from(zero_based.rem_euclid(12) + 1).unwrap_or(1);
    let month = Month::try_from(month_number).unwrap_or(Month::January);
    Date::from_calendar_date(year, month, 1).unwrap_or(date)
}

pub(super) fn weekday_short(date: Date) -> String {
    native_format(date, "%a")
        .unwrap_or_else(|| (date.weekday().number_days_from_monday() + 1).to_string())
}

pub(super) fn month_title(date: Date) -> String {
    native_format(date, "%OB %Y").unwrap_or_else(|| numeric_month(date))
}

pub(super) fn period_title(first: Date, last: Date) -> String {
    if first.year() == last.year() && first.month() == last.month() {
        return native_format(first, "%Ob %Y").unwrap_or_else(|| numeric_month(first));
    }
    let first_month = abbreviated_month(first);
    let last_month = abbreviated_month(last);
    if first.year() == last.year() {
        return format!("{first_month} – {last_month} {}", localized_year(last));
    }
    format!(
        "{first_month} {} – {last_month} {}",
        localized_year(first),
        localized_year(last)
    )
}

/// The full name of an ISO weekday (Monday 1 … Sunday 7), in the process locale.
///
/// Stepped from a fixed date rather than looked up in a table, so the platform's own words are
/// what a repeat sentence reads; and so the anchor's own weekday never has to be asserted here.
pub(super) fn weekday_full(iso: u8) -> String {
    let Ok(anchor) = Date::from_calendar_date(2026, Month::January, 5) else {
        return iso.to_string();
    };
    let from_monday = i64::from(anchor.weekday().number_days_from_monday());
    let date = add_days(anchor, i64::from(iso.saturating_sub(1)) - from_monday);
    native_format(date, "%A").unwrap_or_else(|| iso.to_string())
}

/// The full name of a month number (1–12), in the process locale. Standalone form (`%OB`), the
/// one a sentence names a month in rather than dates it with.
pub(super) fn month_full(month: u32) -> String {
    let named = u8::try_from(month)
        .ok()
        .and_then(|value| Month::try_from(value).ok())
        .and_then(|value| Date::from_calendar_date(2026, value, 1).ok())
        .and_then(|date| native_format(date, "%OB"));
    named.unwrap_or_else(|| month.to_string())
}

/// A date in the locale's own short form: what a repeat sentence's "until" clause reads.
pub(super) fn short_date(date: Date) -> String {
    native_format(date, "%x").unwrap_or_else(|| date.to_string())
}

fn abbreviated_month(date: Date) -> String {
    native_format(date, "%Ob").unwrap_or_else(|| format!("{:02}", u8::from(date.month())))
}

fn localized_year(date: Date) -> String {
    native_format(date, "%Y").unwrap_or_else(|| date.year().to_string())
}

fn numeric_month(date: Date) -> String {
    format!("{:04}-{:02}", date.year(), u8::from(date.month()))
}

/// GLib delegates these names to the process locale installed by GTK at startup.
fn native_format(date: Date, pattern: &str) -> Option<String> {
    gtk::glib::DateTime::from_local(
        date.year(),
        i32::from(u8::from(date.month())),
        i32::from(date.day()),
        12,
        0,
        0.0,
    )
    .ok()?
    .format(pattern)
    .ok()
    .map(|value| value.to_string())
}

pub(super) fn date_heading(date: Date) -> String {
    format!("{} {}", weekday_short(date), date.day())
}

pub(in crate::ui) fn clock(minutes: u32, use_24_hour: bool) -> String {
    let hour = minutes / 60;
    let minute = minutes % 60;
    if use_24_hour {
        format!("{hour:02}:{minute:02}")
    } else {
        let suffix = if hour < 12 { "AM" } else { "PM" };
        let display_hour = match hour % 12 {
            0 => 12,
            value => value,
        };
        if minute == 0 {
            format!("{display_hour} {suffix}")
        } else {
            format!("{display_hour}:{minute:02} {suffix}")
        }
    }
}

pub(super) fn local_date_time(raw: &str, zone: &str, use_24_hour: bool) -> String {
    if raw.is_empty() {
        return String::new();
    }
    if raw.ends_with('Z')
        && let Ok(instant) = raw.parse::<Timestamp>()
        && let Ok(value) = instant.in_tz(zone)
    {
        let minutes = u32::try_from(value.hour()).unwrap_or(0) * 60
            + u32::try_from(value.minute()).unwrap_or(0);
        return format!(
            "{:04}-{:02}-{:02} · {}",
            value.year(),
            value.month(),
            value.day(),
            clock(minutes, use_24_hour)
        );
    }
    if let Some(value) = raw
        .strip_suffix('Z')
        .and_then(parse_wall)
        .or_else(|| parse_wall(raw))
    {
        let minutes = u32::from(value.hour()) * 60 + u32::from(value.minute());
        return format!("{} · {}", value.date(), clock(minutes, use_24_hour));
    }
    raw.get(..10).unwrap_or(raw).to_owned()
}

/// A UTC RFC 3339 instant as a wall-clock date and minutes-from-midnight in `zone`.
///
/// The core ships no display tzdata and emits instants (`docs/timestamps.md`), so every surface
/// that shows one converts here rather than carrying its own arithmetic. `None` when the instant
/// or the zone will not parse; a caller then draws what it was given rather than inventing a time.
pub(in crate::ui) fn instant_in(raw: &str, zone: &str) -> Option<(Date, u32)> {
    let instant = raw.parse::<Timestamp>().ok()?.in_tz(zone).ok()?;
    let year = i32::from(instant.year());
    let month = Month::try_from(u8::try_from(instant.month()).ok()?).ok()?;
    let day = u8::try_from(instant.day()).ok()?;
    let date = Date::from_calendar_date(year, month, day).ok()?;
    let hour = u32::try_from(instant.hour()).ok()?;
    let minute = u32::try_from(instant.minute()).ok()?;
    Some((date, hour * 60 + minute))
}

/// A date written out the way the desktop's locale writes one: "Thursday 20 August 2026".
///
/// The long form rather than `%x`: this is the line a person reads before agreeing to a meeting,
/// and `20-08-2026` is the form that gets misread as a different day.
pub(in crate::ui) fn long_date(date: Date) -> String {
    native_format(date, "%A %e %B %Y").map_or_else(
        || date.to_string(),
        |value| value.split_whitespace().collect::<Vec<_>>().join(" "),
    )
}

pub(super) fn date_from_wall(value: &str) -> Option<Date> {
    value
        .split_once('T')
        .map_or_else(|| parse_date(value), |(date, _)| parse_date(date))
}

#[cfg(test)]
mod tests {
    use time::{Date, Month};

    use super::{
        add_days, add_months, clock, local_date_time, month_full, month_title, parse_date,
        parse_wall, period_title, wall_string, weekday_full, weekday_short,
    };

    #[test]
    fn iso_dates_and_wall_clocks_round_trip_without_a_timezone_conversion() {
        let wall = parse_wall("2026-07-21T10:15:30").expect("valid wall clock");
        assert_eq!(wall_string(wall), "2026-07-21T10:15:30");
        assert_eq!(parse_date("2026-02-30"), None);
    }

    #[test]
    fn month_navigation_is_anchored_on_the_first() {
        let january = Date::from_calendar_date(2026, Month::January, 31).unwrap();
        assert_eq!(add_months(january, 1).to_string(), "2026-02-01");
        assert_eq!(add_months(january, -1).to_string(), "2025-12-01");
    }

    #[test]
    fn period_titles_use_the_platforms_native_abbreviated_months() {
        let first = Date::from_calendar_date(2026, Month::June, 29).unwrap();
        let last = Date::from_calendar_date(2026, Month::July, 5).unwrap();
        assert_eq!(
            period_title(first, last),
            format!(
                "{} – {} {}",
                native(first, "%Ob"),
                native(last, "%Ob"),
                native(last, "%Y")
            )
        );
    }

    #[test]
    fn weekday_names_are_the_platforms_own_and_counted_from_monday() {
        // Indexed from the wrong end this renames every day of the week and still reads like a
        // real sentence, so the whole week is walked rather than one sample: the anchor's own
        // weekday is read from `time` rather than asserted, so no date here has to be trusted.
        let anchor = Date::from_calendar_date(2026, Month::January, 5).unwrap();
        for offset in 0..7 {
            let date = add_days(anchor, offset);
            let iso = date.weekday().number_days_from_monday() + 1;
            assert_eq!(weekday_full(iso), native(date, "%A"));
        }
    }

    #[test]
    fn month_names_are_the_platforms_own_standalone_form() {
        for month in 1..=12u32 {
            let date = Date::from_calendar_date(
                2026,
                Month::try_from(u8::try_from(month).unwrap()).unwrap(),
                1,
            )
            .unwrap();
            assert_eq!(month_full(month), native(date, "%OB"));
        }
    }

    #[test]
    fn headings_and_month_titles_use_the_platform_locale() {
        let date = Date::from_calendar_date(2026, Month::July, 20).unwrap();
        assert_eq!(weekday_short(date), native(date, "%a"));
        assert_eq!(month_title(date), native(date, "%OB %Y"));
    }

    #[test]
    fn clock_format_has_identical_bucket_decisions() {
        assert_eq!(clock(0, true), "00:00");
        assert_eq!(clock(0, false), "12 AM");
        assert_eq!(clock(13 * 60 + 5, false), "1:05 PM");
    }

    #[test]
    fn utc_agenda_instants_are_localized_but_floating_clocks_are_not() {
        assert_eq!(
            local_date_time("2026-07-21T12:05:00Z", "Europe/Amsterdam", true),
            "2026-07-21 · 14:05"
        );
        assert_eq!(
            local_date_time("2026-07-21T13:05:00", "Europe/Amsterdam", false),
            "2026-07-21 · 1:05 PM"
        );
    }

    fn native(date: Date, pattern: &str) -> String {
        gtk::glib::DateTime::from_local(
            date.year(),
            i32::from(u8::from(date.month())),
            i32::from(date.day()),
            12,
            0,
            0.0,
        )
        .unwrap()
        .format(pattern)
        .unwrap()
        .to_string()
    }
}
