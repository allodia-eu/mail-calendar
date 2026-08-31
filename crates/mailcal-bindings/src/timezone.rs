//! Shared time-zone helpers exposed over UniFFI.

use engine_api::{TimeZoneId, is_supported_zone};

/// Parses a host-reported IANA timezone id, falling back to UTC if it is empty (the app's
/// `TimeZoneState` further validates it against the bundled tzdb).
pub(crate) fn device_zone(id: String) -> TimeZoneId {
    TimeZoneId::iana(id).unwrap_or_else(|_| TimeZoneId::utc())
}

/// The device's current IANA time zone, detected from the OS in shared Rust.
///
/// Region-aware, so on Windows it returns the real city (e.g. `Europe/Amsterdam`) rather
/// than the Windows-zone primary (`Europe/Berlin`) that a host's own `TimeZoneInfo` would
/// collapse to. One implementation for every client keeps detection consistent across
/// platforms. The result is validated against the engine's bundled tzdb (so the host
/// adopts a zone the engine can resolve), falling back to `Etc/UTC` if detection fails or
/// the zone is unknown. A host passes this as `device_timezone` to
/// [`crate::MailcalApp::new_accounts`] and re-reads it when the OS signals a zone change.
#[uniffi::export]
pub fn device_time_zone() -> String {
    iana_time_zone::get_timezone()
        .ok()
        .filter(|name| {
            TimeZoneId::iana(name)
                .map(|zone| is_supported_zone(&zone))
                .unwrap_or(false)
        })
        .unwrap_or_else(|| TimeZoneId::utc().as_str().to_owned())
}

/// Every IANA time-zone id the engine's bundled tzdb can resolve, sorted: the list a host
/// fills its time-zone picker with, so it only ever offers a zone the engine can localize
/// against. One authoritative list shared by every client, instead of each host's OS zone
/// set (which on Windows collapses cities like `Europe/Amsterdam` into a single zone).
#[uniffi::export]
pub fn available_time_zones() -> Vec<String> {
    mailcal_app::available_time_zones()
}
