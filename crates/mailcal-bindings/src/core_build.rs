//! Which kind of core this is (`docs/ai.md` rule 5), and the marker that says so in the bytes of
//! every library and executable that links it.
//!
//! A development core offers the writing assistant to everyone, so no package may carry one. The
//! build refuses one when it is told it is packaging (`MAILCAL_PACKAGED_CORE`, `build.rs`); a
//! packager that takes a core it did not build reads the marker instead
//! (`scripts/dev/check-core-build.sh` and its counterparts in the Android and Windows builds), so
//! it needs no toolchain for the target. Exactly one marker is compiled in.

use mailcal_app::ai::BuildKind;

/// What a packager looks for in a development core.
#[cfg(any(debug_assertions, feature = "dev-harness"))]
static MARKER: &str = "mailcal-core-build:development";

/// What a packager looks for in a production core.
#[cfg(not(any(debug_assertions, feature = "dev-harness")))]
static MARKER: &str = "mailcal-core-build:production";

/// The build this core is, for the offer.
pub(crate) fn build_kind() -> BuildKind {
    // Read, so the linker keeps the marker wherever the core is linked: a packager that finds no
    // marker refuses the core.
    std::hint::black_box(MARKER);
    if cfg!(any(debug_assertions, feature = "dev-harness")) {
        BuildKind::Development
    } else {
        BuildKind::Production
    }
}

#[cfg(test)]
mod tests {
    use mailcal_app::ai::BuildKind;

    use super::{MARKER, build_kind};

    /// The marker a packager reads names the kind the offer is decided by.
    #[test]
    fn the_marker_names_the_kind_the_offer_reads() {
        let kind = match MARKER {
            "mailcal-core-build:development" => BuildKind::Development,
            "mailcal-core-build:production" => BuildKind::Production,
            other => panic!("an unknown marker: {other}"),
        };
        assert_eq!(kind, build_kind());
    }
}
