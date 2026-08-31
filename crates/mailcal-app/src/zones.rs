//! The host-facing time-zone list ([`available_time_zones`]). Split out of `lib.rs` to
//! keep it under the 500-line limit; a thin forward to the engine's bundled tzdb.

/// Every IANA time-zone id the engine can localise against, sorted: the authoritative
/// list a host fills its time-zone picker from.
///
/// Delegates to the engine ([`engine_api::available_zones`]) so the list comes from the
/// exact bundled tzdb the engine resolves and migrates the store against (the recorded
/// `tzdata_version`): no second tzdb copy in the product core, and no version skew. This
/// replaces each host's own OS zone set, which on Windows collapses distinct IANA cities
/// like `Europe/Amsterdam` into a single zone.
#[must_use]
pub fn available_time_zones() -> Vec<String> {
    engine_api::available_zones()
}

#[cfg(test)]
mod tests {
    use engine_api::{TimeZoneId, is_supported_zone};

    use super::available_time_zones;

    #[test]
    fn lists_the_full_resolvable_tzdb_sorted() {
        let zones = available_time_zones();
        // The bundled tzdb has hundreds of zones, far more than a Windows host's ~140.
        assert!(
            zones.len() > 100,
            "expected the full tzdb, got {}",
            zones.len()
        );
        // Including the cities a Windows OS list collapses away (the bug that motivated this).
        for expected in [
            "Europe/Amsterdam",
            "Europe/Berlin",
            "America/New_York",
            "Etc/UTC",
        ] {
            assert!(
                zones.iter().any(|zone| zone == expected),
                "missing {expected}"
            );
        }
        // Sorted and de-duplicated.
        assert!(
            zones.windows(2).all(|pair| pair[0] < pair[1]),
            "not sorted/unique"
        );
        // Every entry is a zone the engine can actually resolve.
        assert!(
            zones
                .iter()
                .all(|zone| is_supported_zone(&TimeZoneId::iana(zone.clone()).unwrap())),
            "an offered zone is not engine-resolvable"
        );
    }
}
