//! Sending the feedback that waits in the outbox (`docs/ai.md`, "Feedback"), in the background: at
//! launch and after each rating.
//!
//! The sender asks the jurisdiction gate before each post, and does nothing at all, not even mint
//! a token, while `allodia_license::AI_FEEDBACK_ROUTE_LIVE` is false. A delivered item leaves the
//! outbox; anything else leaves it for the next pass.

use std::sync::Mutex;

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

    /// Passes until nothing new was asked for meanwhile. **Blocking.** A call while a pass runs
    /// returns at once and has that pass go round once more, so a rating given during a pass is
    /// sent without any item being posted twice.
    fn send_ai_feedback(&self) {
        if !self.ai_feedback_flight.begin() {
            return;
        }
        loop {
            self.send_ai_feedback_once();
            if !self.ai_feedback_flight.again() {
                break;
            }
        }
    }

    /// One pass over what waits now.
    fn send_ai_feedback_once(&self) {
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

/// At most one pass at a time, and one more when another was asked for while it ran.
#[derive(Debug, Default)]
pub(crate) struct SingleFlight(Mutex<Flight>);

#[derive(Debug, Default)]
struct Flight {
    running: bool,
    again: bool,
}

impl SingleFlight {
    /// `true` when the caller is to run the pass; `false` when one runs already, which then goes
    /// round once more.
    fn begin(&self) -> bool {
        let mut flight = self.0.lock().expect("feedback flight lock");
        if flight.running {
            flight.again = true;
            return false;
        }
        flight.running = true;
        true
    }

    /// Called as a pass ends: `true` when it is to run again, `false` when it is over.
    fn again(&self) -> bool {
        let mut flight = self.0.lock().expect("feedback flight lock");
        if flight.again {
            flight.again = false;
            return true;
        }
        flight.running = false;
        false
    }
}

#[cfg(test)]
mod tests {
    use super::SingleFlight;

    #[test]
    fn a_pass_asked_for_while_one_runs_is_folded_into_it() {
        let flight = SingleFlight::default();
        assert!(flight.begin());
        assert!(!flight.begin(), "a second pass never runs beside the first");
        assert!(!flight.begin());
        assert!(flight.again(), "the running pass goes round once more");
        assert!(!flight.again());
        assert!(flight.begin(), "and once it is over the next one starts");
    }
}
