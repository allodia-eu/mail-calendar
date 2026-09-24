//! Mail timestamp formatting in the active display zone and app language.

use jiff::Timestamp;
use time::{Date, Month};

use crate::l10n;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum RelativeDatePattern {
    Time,
    Weekday,
    Date,
    DateWithYear,
}

fn relative_date_pattern(day_diff: i64, same_year: bool) -> RelativeDatePattern {
    match day_diff {
        0 => RelativeDatePattern::Time,
        1..=6 => RelativeDatePattern::Weekday,
        _ if same_year => RelativeDatePattern::Date,
        _ => RelativeDatePattern::DateWithYear,
    }
}

pub(super) fn relative_date(raw: &str, zone: &str) -> String {
    relative_date_at(raw, zone, Timestamp::now(), l10n::active_locale())
}

fn relative_date_at(raw: &str, zone: &str, now: Timestamp, locale: &str) -> String {
    if raw.is_empty() || !raw.ends_with('Z') {
        return local_date_time(raw, zone);
    }
    let Ok(instant) = raw.parse::<Timestamp>() else {
        return local_date_time(raw, zone);
    };
    let (Ok(message), Ok(current)) = (instant.in_tz(zone), now.in_tz(zone)) else {
        return local_date_time(raw, zone);
    };
    let (Some(message_date), Some(current_date)) = (date_of(&message), date_of(&current)) else {
        return local_date_time(raw, zone);
    };
    let day_diff = (current_date - message_date).whole_days();
    let names = date_names(locale);
    match relative_date_pattern(day_diff, message.year() == current.year()) {
        RelativeDatePattern::Time => format!("{:02}:{:02}", message.hour(), message.minute()),
        RelativeDatePattern::Weekday => {
            names.weekdays[usize::from(message_date.weekday().number_days_from_monday())].to_owned()
        }
        RelativeDatePattern::Date => format!(
            "{} {}",
            message.day(),
            names.months[usize::from(u8::from(message_date.month()) - 1)]
        ),
        RelativeDatePattern::DateWithYear => format!(
            "{} {} {}",
            message.day(),
            names.months[usize::from(u8::from(message_date.month()) - 1)],
            message.year()
        ),
    }
}

pub(super) fn local_date_time(raw: &str, zone: &str) -> String {
    if raw.is_empty() {
        return String::new();
    }
    if raw.ends_with('Z')
        && let Ok(instant) = raw.parse::<Timestamp>()
        && let Ok(value) = instant.in_tz(zone)
    {
        return format!(
            "{:04}-{:02}-{:02} {:02}:{:02}",
            value.year(),
            value.month(),
            value.day(),
            value.hour(),
            value.minute()
        );
    }
    if raw.contains('T')
        && let Some(value) = raw.get(..16)
    {
        return value.replace('T', " ");
    }
    raw.get(..10).unwrap_or(raw).to_owned()
}

/// A date the **account service** sent, as a day a reader can check against a bank statement.
///
/// Day precision on purpose: a renewal is a date somebody's statement will agree with, and an hour
/// and minute in that sentence would invite a comparison with a clock that means nothing there.
///
/// ⚠️ The account service is not the sync engine and does not send the engine's `Z`-suffixed
/// instants alone: an offset like `+02:00`, and a plain calendar day for a period that ends on
/// one, both arrive. An instant is read in UTC, which is the day the service means; a plain day is
/// taken as it stands, because reading one as a local instant loses a day everywhere east of
/// Greenwich. A shape this build cannot read at all still has the right day in its leading ten
/// characters, so the sentence stays true and only stops being localised.
///
/// The shape is this client's own, the one every mail row older than a year already carries. The
/// Apple, Android and Windows twins each use their platform's long date instead, which is what
/// client-side formatting means: the sentence comes from the catalog and the date is the host's.
pub(super) fn account_date(raw: &str, locale: &str) -> Option<String> {
    if raw.is_empty() {
        return None;
    }
    let Some(day) = instant_day(raw).or_else(|| civil_day(raw)) else {
        return Some(raw.get(..10).unwrap_or(raw).to_owned());
    };
    let names = date_names(locale);
    Some(format!(
        "{} {} {}",
        day.day(),
        names.months[usize::from(u8::from(day.month()) - 1)],
        day.year()
    ))
}

/// The day an instant falls on in `zone`, the reader's own calendar ("3 Jul 2025"): when a style
/// was learned, and how far back the device's sent mail reaches.
pub(super) fn local_day(seconds: i64, zone: &str, locale: &str) -> Option<String> {
    let value = Timestamp::from_second(seconds).ok()?.in_tz(zone).ok()?;
    let day = date_of(&value)?;
    let names = date_names(locale);
    Some(format!(
        "{} {} {}",
        day.day(),
        names.months[usize::from(u8::from(day.month()) - 1)],
        day.year()
    ))
}

/// The reveal's "Since" figure: the day and month within this year, the month and year before it,
/// because it stands at the size of the counts beside it and a full date does not fit there.
pub(super) fn since_figure(
    seconds: i64,
    now: Timestamp,
    zone: &str,
    locale: &str,
) -> Option<String> {
    let value = Timestamp::from_second(seconds).ok()?.in_tz(zone).ok()?;
    let current = now.in_tz(zone).ok()?;
    let day = date_of(&value)?;
    let month = date_names(locale).months[usize::from(u8::from(day.month()) - 1)];
    Some(if value.year() == current.year() {
        format!("{} {month}", day.day())
    } else {
        format!("{month} {}", day.year())
    })
}

/// The day of an instant that carries its own offset, read in UTC.
fn instant_day(raw: &str) -> Option<Date> {
    let instant = raw.parse::<Timestamp>().ok()?;
    date_of(&instant.in_tz("UTC").ok()?)
}

/// The day of a plain `YYYY-MM-DD`, taken as it stands.
fn civil_day(raw: &str) -> Option<Date> {
    let mut parts = raw.get(..10)?.split('-');
    let year = parts.next()?.parse().ok()?;
    let month = Month::try_from(parts.next()?.parse::<u8>().ok()?).ok()?;
    let day = parts.next()?.parse().ok()?;
    Date::from_calendar_date(year, month, day).ok()
}

fn date_of(value: &jiff::Zoned) -> Option<Date> {
    Date::from_calendar_date(
        i32::from(value.year()),
        Month::try_from(u8::try_from(value.month()).ok()?).ok()?,
        u8::try_from(value.day()).ok()?,
    )
    .ok()
}

/// GLib formats against the process locale, which the app's language override does not change.
struct DateNames {
    weekdays: [&'static str; 7],
    months: [&'static str; 12],
}

fn date_names(locale: &str) -> &'static DateNames {
    date_names_for(locale).unwrap_or(&EN)
}

fn date_names_for(locale: &str) -> Option<&'static DateNames> {
    Some(match locale {
        "en" => &EN,
        "nl" => &NL,
        "de" => &DE,
        "fr" => &FR,
        "es" => &ES,
        "it" => &IT,
        "pt" => &PT,
        _ => return None,
    })
}

const EN: DateNames = DateNames {
    weekdays: ["Mon", "Tue", "Wed", "Thu", "Fri", "Sat", "Sun"],
    months: [
        "Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec",
    ],
};
const NL: DateNames = DateNames {
    weekdays: ["ma", "di", "wo", "do", "vr", "za", "zo"],
    months: [
        "jan", "feb", "mrt", "apr", "mei", "jun", "jul", "aug", "sep", "okt", "nov", "dec",
    ],
};
const DE: DateNames = DateNames {
    weekdays: ["Mo", "Di", "Mi", "Do", "Fr", "Sa", "So"],
    months: [
        "Jan", "Feb", "Mär", "Apr", "Mai", "Jun", "Jul", "Aug", "Sep", "Okt", "Nov", "Dez",
    ],
};
const FR: DateNames = DateNames {
    weekdays: ["lun.", "mar.", "mer.", "jeu.", "ven.", "sam.", "dim."],
    months: [
        "janv.", "févr.", "mars", "avr.", "mai", "juin", "juil.", "août", "sept.", "oct.", "nov.",
        "déc.",
    ],
};
const ES: DateNames = DateNames {
    weekdays: ["lun", "mar", "mié", "jue", "vie", "sáb", "dom"],
    months: [
        "ene", "feb", "mar", "abr", "may", "jun", "jul", "ago", "sept", "oct", "nov", "dic",
    ],
};
const IT: DateNames = DateNames {
    weekdays: ["lun", "mar", "mer", "gio", "ven", "sab", "dom"],
    months: [
        "gen", "feb", "mar", "apr", "mag", "giu", "lug", "ago", "set", "ott", "nov", "dic",
    ],
};
const PT: DateNames = DateNames {
    weekdays: ["seg", "ter", "qua", "qui", "sex", "sáb", "dom"],
    months: [
        "jan", "fev", "mar", "abr", "mai", "jun", "jul", "ago", "set", "out", "nov", "dez",
    ],
};

#[cfg(test)]
mod tests {
    use jiff::Timestamp;

    use super::{
        RelativeDatePattern, account_date, date_names_for, local_date_time, local_day,
        relative_date_at, relative_date_pattern, since_figure,
    };

    fn stamp(raw: &str) -> Timestamp {
        raw.parse().expect("fixture is an RFC 3339 instant")
    }

    #[test]
    fn relative_patterns_keep_day_seven_unambiguous() {
        assert_eq!(relative_date_pattern(0, true), RelativeDatePattern::Time);
        for day in 1..=6 {
            assert_eq!(
                relative_date_pattern(day, true),
                RelativeDatePattern::Weekday
            );
        }
        assert_eq!(relative_date_pattern(7, true), RelativeDatePattern::Date);
        assert_eq!(
            relative_date_pattern(400, false),
            RelativeDatePattern::DateWithYear
        );
    }

    #[test]
    fn relative_labels_use_the_display_zone_and_app_language() {
        let now = stamp("2026-07-20T20:00:00Z");
        assert_eq!(
            relative_date_at("2026-07-20T09:05:00Z", "UTC", now, "en"),
            "09:05"
        );
        assert_eq!(
            relative_date_at("2026-07-17T09:05:00Z", "UTC", now, "nl"),
            "vr"
        );
        assert_eq!(
            relative_date_at("2026-07-13T09:05:00Z", "UTC", now, "nl"),
            "13 jul"
        );
        assert_eq!(
            relative_date_at("2025-07-03T09:05:00Z", "UTC", now, "nl"),
            "3 jul 2025"
        );
        assert_eq!(
            relative_date_at("2026-07-19T22:30:00Z", "Europe/Amsterdam", now, "en"),
            "00:30"
        );
        assert_eq!(
            relative_date_at("2026-07-20T09:05:00", "UTC", now, "nl"),
            "2026-07-20 09:05"
        );
    }

    #[test]
    fn every_catalog_language_has_date_names() {
        for locale in crate::l10n::LOCALES {
            assert!(
                date_names_for(locale).is_some(),
                "{locale} needs weekday and month names"
            );
        }
        assert!(date_names_for("not-a-locale").is_none());
    }

    /// ⚠️ A plain calendar day read as a local instant and rendered in UTC is the day before,
    /// everywhere east of Greenwich, and the sentence it lands in says when somebody's access
    /// stops. An instant carrying its own offset is the other way about: UTC is what the service
    /// means by it.
    #[test]
    fn an_account_service_date_keeps_the_day_the_service_meant() {
        assert_eq!(
            account_date("2026-11-01", "nl").as_deref(),
            Some("1 nov 2026")
        );
        assert_eq!(
            account_date("2026-11-01T00:30:00+02:00", "nl").as_deref(),
            Some("31 okt 2026")
        );
        assert_eq!(
            account_date("2026-11-01T09:05:00Z", "en").as_deref(),
            Some("1 Nov 2026")
        );
    }

    /// An empty string is no date at all, and a shape nothing can read still has the right day.
    #[test]
    fn an_unreadable_account_date_keeps_its_day_and_only_stops_being_localised() {
        assert_eq!(account_date("", "en"), None);
        assert_eq!(
            account_date("01/11/2026 ish", "en").as_deref(),
            Some("01/11/2026")
        );
    }

    /// The day is the reader's: an instant late on the 31st in UTC is already the 1st in
    /// Amsterdam.
    #[test]
    fn a_local_day_is_the_day_in_the_display_zone() {
        let late = stamp("2024-12-31T23:30:00Z").as_second();
        assert_eq!(local_day(late, "UTC", "en").as_deref(), Some("31 Dec 2024"));
        assert_eq!(
            local_day(late, "Europe/Amsterdam", "nl").as_deref(),
            Some("1 jan 2025")
        );
        assert_eq!(local_day(late, "Not/A_Zone", "en"), None);
    }

    #[test]
    fn since_is_the_day_this_year_and_the_month_before_it() {
        let now = stamp("2026-09-24T12:00:00Z");
        let june = stamp("2026-06-24T12:00:00Z").as_second();
        let march = stamp("2025-03-03T00:00:00Z").as_second();
        assert_eq!(
            since_figure(june, now, "UTC", "en").as_deref(),
            Some("24 Jun")
        );
        assert_eq!(
            since_figure(march, now, "UTC", "nl").as_deref(),
            Some("mrt 2025")
        );
    }

    #[test]
    fn the_reading_header_is_an_absolute_local_date_and_time() {
        assert_eq!(
            local_date_time("2026-07-20T09:05:00Z", "Europe/Amsterdam"),
            "2026-07-20 11:05"
        );
        assert_eq!(
            local_date_time("2026-07-20T09:05:00", "Europe/Amsterdam"),
            "2026-07-20 09:05"
        );
        assert_eq!(local_date_time("2026-07-20", "UTC"), "2026-07-20");
    }
}
