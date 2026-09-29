//! Feedback on a drafted reply, on [`MailcalApp`] (`docs/ai.md`, "Feedback").
//!
//! Offered only while somebody is signed in to an Allodia account, the one way it can reach
//! Allodia. A rating goes into the core's outbox at once; sending it is a background pass behind
//! the jurisdiction gate (`crate::ai_feedback_pass`), and nothing leaves until the service has the
//! route.

use crate::{MailcalApp, records_ai_feedback::DraftRating};

#[uniffi::export]
impl MailcalApp {
    /// Whether a client offers feedback on a draft: somebody is signed in to an Allodia account
    /// in a build that has the sign-in. Cheap and local.
    #[must_use]
    pub fn ai_feedback_available(&self) -> bool {
        #[cfg(debug_assertions)]
        if staged::feedback_offered() {
            return true;
        }
        cfg!(feature = "allodia-license")
            && self.allodia.lock().expect("allodia account lock").is_some()
    }

    /// Keeps `rating` of the draft `draft_id`, with the message it answered and what the draft
    /// said when `include_content`, and starts a pass that sends what is waiting. Returns whether
    /// it was kept: not when feedback is unavailable, nor for a draft this session did not issue.
    pub fn rate_draft(&self, draft_id: String, rating: DraftRating, include_content: bool) -> bool {
        if !self.ai_feedback_available() {
            return false;
        }
        let kept = self
            .app
            .rate_draft(&draft_id, rating.into(), include_content);
        #[cfg(feature = "allodia-license")]
        if kept {
            self.send_ai_feedback_in_background();
        }
        kept
    }
}

/// Debug builds only: `MAILCAL_FAKE_AI_FEEDBACK` offers feedback without an Allodia sign-in, so
/// the form can be driven against the harness. What is rated is kept in the outbox like any other
/// rating; the pass that would send it still needs a sign-in, so nothing leaves.
#[cfg(debug_assertions)]
mod staged {
    use std::sync::OnceLock;

    pub(super) fn feedback_offered() -> bool {
        static STAGED: OnceLock<bool> = OnceLock::new();
        *STAGED.get_or_init(|| {
            let staged = std::env::var_os("MAILCAL_FAKE_AI_FEEDBACK").is_some();
            if staged {
                log::warn!("ai: feedback on drafts is offered without an Allodia sign-in (staged)");
            }
            staged
        })
    }
}
