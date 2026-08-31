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
        RelativeDatePattern, date_names_for, local_date_time, relative_date_at,
        relative_date_pattern,
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
