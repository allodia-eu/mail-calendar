//! Who is offered writing style at all (`docs/ai.md`, "Early access"): every development build,
//! and a production build only while the signed-in Allodia account's entitlement carries `ai`,
//! which Allodia assigns person by person while the feature is tested.
//!
//! Not offered, the app installs no backend whatever is set up, so every surface that needs one
//! stays hidden; the snapshot's `offered` hides the own endpoint's settings as well.

use std::sync::atomic::Ordering;

use crate::MailcalApp;

/// A debug build, or the Android development build, whose core is optimised but carries the
/// harness's `dev-harness` feature. A production build is neither.
pub(crate) const DEVELOPMENT_BUILD: bool = cfg!(any(debug_assertions, feature = "dev-harness"));

impl MailcalApp {
    /// Whether writing style is offered in this build, to this person, now.
    pub(crate) fn ai_offered(&self) -> bool {
        self.development_build.load(Ordering::Relaxed) || self.ai_entitled()
    }

    /// A build without the Allodia sign-in has no entitlement to read.
    #[cfg(not(feature = "allodia-license"))]
    #[expect(clippy::unused_self, reason = "the same call as the licensed build's")]
    fn ai_entitled(&self) -> bool {
        false
    }

    /// Makes this app answer as a production build does, for a test of the gate.
    #[cfg(test)]
    pub(crate) fn as_production_build(&self) {
        self.development_build.store(false, Ordering::Relaxed);
        self.refresh_ai_backend();
    }
}
