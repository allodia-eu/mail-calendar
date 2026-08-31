//! Foreground timer cadence and deterministic runtime-test bounds.

use std::time::Duration;

pub(super) fn calendar_refresh_interval() -> Duration {
    #[cfg(debug_assertions)]
    if let Ok(raw) = std::env::var("MAILCAL_CALENDAR_REFRESH_SECONDS")
        && let Ok(seconds) = raw.parse::<u64>()
    {
        return Duration::from_secs(seconds.max(1));
    }
    Duration::from_mins(5)
}

pub(super) fn calendar_refresh_limit() -> Option<u64> {
    #[cfg(debug_assertions)]
    {
        parse_calendar_refresh_limit(
            std::env::var("MAILCAL_CALENDAR_REFRESH_LIMIT")
                .ok()
                .as_deref(),
        )
    }
    #[cfg(not(debug_assertions))]
    {
        None
    }
}

#[cfg(debug_assertions)]
fn parse_calendar_refresh_limit(raw: Option<&str>) -> Option<u64> {
    raw.and_then(|value| value.parse::<u64>().ok())
        .filter(|limit| *limit > 0)
}

#[cfg(test)]
mod tests {
    use super::parse_calendar_refresh_limit;

    #[test]
    fn the_runtime_refresh_limit_accepts_only_a_positive_integer() {
        assert_eq!(parse_calendar_refresh_limit(Some("2")), Some(2));
        assert_eq!(parse_calendar_refresh_limit(Some("0")), None);
        assert_eq!(parse_calendar_refresh_limit(Some("many")), None);
        assert_eq!(parse_calendar_refresh_limit(None), None);
    }
}
