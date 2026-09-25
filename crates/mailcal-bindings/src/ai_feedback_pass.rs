//! Sending the feedback that waits in the outbox (`docs/ai.md`, "Feedback"), in the background: at
//! launch and after each rating.
//!
//! The sender asks the jurisdiction gate before each post, and does nothing at all, not even mint
//! a token, while `allodia_license::AI_FEEDBACK_ROUTE_LIVE` is false. A delivered item leaves the
//! outbox; anything else leaves it for the next pass.

use allodia_license::{AccountService, Feature, FeedbackSender};

use crate::{MailcalApp, ai_transport::AiTransport};

impl MailcalApp {
    /// Starts a pass over the outbox off the caller's thread.
    pub(crate) fn send_ai_feedback_in_background(&self) {
        let this = self.this.clone();
        self.runtime.spawn_blocking(move || {
            if let Some(app) = this.upgrade() {
                app.send_ai_feedback();
            }
        });
    }

    /// One pass. **Blocking.**
    fn send_ai_feedback(&self) {
        let waiting = self.app.ai_feedback_waiting();
        let signed_in = self.allodia.lock().expect("allodia account lock").is_some();
        if waiting.is_empty() || !signed_in {
            return;
        }
        let Ok(transport) = AiTransport::new(self.runtime.handle().clone()) else {
            return;
        };
        let sender = FeedbackSender::new(
            &AccountService::new(allodia_license::host()),
            Box::new(transport),
            self.app.jurisdiction_mode_source(),
        );
        let delivery = sender.deliver(
            || {
                self.allodia_grant_permits(Feature::UseAi)
                    .then(|| self.allodia_access_token("feedback").ok())
                    .flatten()
            },
            waiting
                .iter()
                .map(|item| (item.id.as_str(), item.body.as_str())),
        );
        for id in &delivery.delivered {
            self.app.ai_feedback_delivered(id);
        }
        if !delivery.delivered.is_empty() {
            log::info!(
                "ai: {} piece(s) of feedback delivered",
                delivery.delivered.len()
            );
        }
        if let Some(stopped) = delivery.stopped {
            log::info!("ai: feedback stays waiting; {stopped}");
        }
    }
}
