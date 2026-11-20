//! Refusing to build a development core that is about to be packaged.
//!
//! A development core (debug assertions on, or the `dev-harness` feature) offers the writing
//! assistant to everyone and carries the harness's trust path, so no package may carry one
//! (`docs/ai.md` rule 5). Every packager sets [`PACKAGED_CORE_VAR`] on the cargo line that builds
//! the core it ships, and the core's build script refuses to compile one that is a development
//! core. Only that line: a packager's other cargo runs (a debug core a binding generator reads) are
//! not shipped and must not be refused.
//!
//! [`crate::REQUIRE_VAR`] is not the signal, for two reasons. It means "carries Allodia's injected
//! values", which a from-source package rightly does not; and a packager sets it for its whole
//! run, debug builds included.

use crate::truthy;

/// Set, on the cargo line that builds a core a packager ships, and a development core fails to
/// compile. Read from the environment only, never the repo's `.env`, so it cannot leak into a
/// developer's own builds.
pub const PACKAGED_CORE_VAR: &str = "MAILCAL_PACKAGED_CORE";

/// Why the core being built must not be, or `None` when it may: `packaged` is
/// [`PACKAGED_CORE_VAR`]'s value, and the two flags say whether this is a development core.
#[must_use]
pub fn development_core_refusal(
    packaged: Option<&str>,
    debug_assertions: bool,
    dev_harness: bool,
) -> Option<String> {
    if !truthy(packaged) || !(debug_assertions || dev_harness) {
        return None;
    }
    let why = match (debug_assertions, dev_harness) {
        (true, true) => "debug assertions and the dev-harness feature are on",
        (true, false) => "debug assertions are on",
        (false, _) => "the dev-harness feature is on",
    };
    Some(format!(
        "{PACKAGED_CORE_VAR} is set, so this core is to be packaged, but {why}: a development \
         core offers the writing assistant to everyone (docs/ai.md rule 5). Build it with \
         --release and without dev-harness, or unset {PACKAGED_CORE_VAR} for a build nobody \
         installs."
    ))
}

#[cfg(test)]
mod tests {
    use super::development_core_refusal;

    #[test]
    fn a_packaged_core_is_refused_when_it_is_a_development_one() {
        for (debug, harness) in [(true, false), (false, true), (true, true)] {
            let refusal = development_core_refusal(Some("1"), debug, harness)
                .expect("a development core is refused");
            assert!(refusal.contains("MAILCAL_PACKAGED_CORE"), "{refusal}");
            assert!(!refusal.contains('\n'), "one line: {refusal}");
        }
    }

    #[test]
    fn a_release_core_or_an_unpackaged_one_is_built() {
        assert_eq!(development_core_refusal(Some("1"), false, false), None);
        for packaged in [None, Some(""), Some("0"), Some("false")] {
            assert_eq!(development_core_refusal(packaged, true, true), None);
        }
    }
}
