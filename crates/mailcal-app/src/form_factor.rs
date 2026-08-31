//! Desktop or mobile, and the product defaults that differ between them.
//!
//! The core is compiled once per platform, so which one this is needs no host to report it and
//! no FFI to carry it; `cfg!` answers at build time, and a host cannot forget to say or say it
//! wrongly. What a host *does* report (the `Platform` the analytics payload carries) is a finer
//! question than this one: it distinguishes iPad from iPhone because a support answer wants
//! that, while a default only cares which side of this line the device is on.
//!
//! The decision and the defaults are kept apart on purpose. [`FormFactor::current`] is the one
//! line that varies by target; everything after it is a plain function of the value, so both
//! answers are tested on whatever host happens to run the suite rather than only the one that
//! compiled them.

/// Which kind of device this build runs on.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum FormFactor {
    /// macOS, Windows, Linux; mains power, a disk with room, an unmetered link more often
    /// than not.
    Desktop,
    /// iOS, iPadOS, Android: a battery, a storage tier the user paid for by the gigabyte, and
    /// a connection that may be charged by the megabyte.
    Mobile,
}

impl FormFactor {
    /// What this build targets.
    ///
    /// `cfg!` rather than `#[cfg]` so both arms compile on every host: a branch only one target
    /// can see is a branch only one target's CI can catch.
    #[must_use]
    pub(crate) const fn current() -> Self {
        if cfg!(any(target_os = "ios", target_os = "android")) {
            Self::Mobile
        } else {
            Self::Desktop
        }
    }

    /// The largest message the body warm pulls in full, in octets; `None` warms every size.
    ///
    /// A warm fetches whole raw sources, so what it costs is bytes rather than messages, and
    /// the tail holds most of them; one photo thread outweighs a thousand replies. A laptop
    /// spends disk it has to keep every message readable offline; a phone would spend a
    /// metered link and a storage tier its owner paid for by the gigabyte, on the few messages
    /// least likely to be reread. Above the cap the body waits for the open that asks for it,
    /// which fetches and caches it anyway.
    #[must_use]
    pub(crate) const fn default_prefetch_size_limit(self) -> Option<u64> {
        match self {
            Self::Desktop => None,
            Self::Mobile => Some(2 * 1024 * 1024),
        }
    }

    /// How much history a newly added account syncs, in months.
    ///
    /// A first sync is the longest wait the app ever asks for and the largest thing it ever
    /// writes, and the two sides of that trade are not the same on a phone as on a laptop. The
    /// user can move it either way afterwards; this is only where the slider starts.
    #[must_use]
    pub(crate) const fn default_sync_depth_months(self) -> u16 {
        match self {
            Self::Desktop => 6,
            Self::Mobile => 3,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::FormFactor;

    #[test]
    fn a_phone_starts_shallower_than_a_laptop() {
        // Asserted for both values on every host: the point of splitting the `cfg!` out of the
        // mapping is that neither answer depends on which target compiled the test.
        assert_eq!(FormFactor::Desktop.default_sync_depth_months(), 6);
        assert_eq!(FormFactor::Mobile.default_sync_depth_months(), 3);
        assert!(
            FormFactor::Mobile.default_sync_depth_months()
                < FormFactor::Desktop.default_sync_depth_months(),
            "the constrained device is the one that syncs less",
        );
    }

    #[test]
    fn only_the_metered_device_caps_what_the_warm_pulls() {
        assert_eq!(FormFactor::Desktop.default_prefetch_size_limit(), None);
        assert_eq!(
            FormFactor::Mobile.default_prefetch_size_limit(),
            Some(2 * 1024 * 1024),
        );
    }

    #[test]
    fn this_build_resolves_to_the_side_its_target_is_on() {
        let expected = if cfg!(any(target_os = "ios", target_os = "android")) {
            FormFactor::Mobile
        } else {
            FormFactor::Desktop
        };
        assert_eq!(FormFactor::current(), expected);
    }
}
