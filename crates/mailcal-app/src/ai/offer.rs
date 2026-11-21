//! Whether the writing assistant is offered (`docs/ai.md` rules 4 and 5): one decision, which
//! every surface of the feature reads, so the three appear and disappear together.

/// What kind of build this core is.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum BuildKind {
    /// A debug build, or a release core built for development (`dev-harness`).
    Development,
    /// Everything else; what a store or a download ships.
    #[default]
    Production,
}

/// What the Allodia entitlement says about the assistant, as the licence side last read it.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum AiEntitlement {
    /// The entitlement in force carries `ai`.
    Granted,
    /// It does not, or there is none: every build from source, and every install nobody signed
    /// in to.
    #[default]
    NotGranted,
}

/// Whether the feature is offered: to everyone in a development build, and in a production build
/// only while the entitlement carries `ai`.
#[must_use]
pub const fn ai_offered(build: BuildKind, entitlement: AiEntitlement) -> bool {
    matches!(build, BuildKind::Development) || matches!(entitlement, AiEntitlement::Granted)
}

#[cfg(test)]
mod tests {
    use super::{AiEntitlement, BuildKind, ai_offered};

    #[test]
    fn a_development_build_offers_it_to_everyone() {
        assert!(ai_offered(
            BuildKind::Development,
            AiEntitlement::NotGranted
        ));
        assert!(ai_offered(BuildKind::Development, AiEntitlement::Granted));
    }

    #[test]
    fn a_production_build_offers_it_only_with_the_entitlement() {
        assert!(ai_offered(BuildKind::Production, AiEntitlement::Granted));
        assert!(!ai_offered(
            BuildKind::Production,
            AiEntitlement::NotGranted
        ));
    }

    #[test]
    fn nothing_known_is_a_production_build_without_the_entitlement() {
        assert!(!ai_offered(BuildKind::default(), AiEntitlement::default()));
    }
}
