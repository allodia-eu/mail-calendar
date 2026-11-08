//! Device-time-zone monitoring. A change is reported to the core, which asks whether to adopt it as
//! the `TimeZoneChanged` notice (`docs/notices.md` rule 8).

use std::time::Duration;

use mailcal_bindings::device_time_zone;

#[derive(Debug)]
pub(super) struct DeviceZoneMonitor {
    current: String,
}

impl DeviceZoneMonitor {
    pub(super) const fn new(current: String) -> Self {
        Self { current }
    }

    pub(super) fn changed(&mut self, next: String) -> Option<String> {
        if next == self.current {
            return None;
        }
        self.current.clone_from(&next);
        Some(next)
    }
}

pub(super) fn device_zone() -> String {
    #[cfg(debug_assertions)]
    if let Ok(zone) = std::env::var("MAILCAL_FAKE_DEVICE_TIMEZONE")
        && !zone.is_empty()
    {
        return zone;
    }
    device_time_zone()
}

pub(super) fn poll_interval() -> Duration {
    #[cfg(debug_assertions)]
    if std::env::var("MAILCAL_FAKE_DEVICE_TIMEZONE").is_ok_and(|zone| !zone.is_empty()) {
        return Duration::from_secs(1);
    }
    Duration::from_mins(1)
}

#[cfg(test)]
mod tests {
    use super::DeviceZoneMonitor;

    #[test]
    fn the_monitor_reports_only_a_real_device_zone_change() {
        let mut monitor = DeviceZoneMonitor::new("Europe/Amsterdam".to_owned());
        assert_eq!(monitor.changed("Europe/Amsterdam".to_owned()), None);
        assert_eq!(
            monitor.changed("America/New_York".to_owned()),
            Some("America/New_York".to_owned())
        );
        assert_eq!(monitor.changed("America/New_York".to_owned()), None);
    }
}
