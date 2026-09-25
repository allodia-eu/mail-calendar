//! Allodia's relay as the AI backend, in a build that carries the Allodia sign-in.
//!
//! The relay is offered when somebody is signed in and the last entitlement answer, inside its
//! grace, grants `ai` (`entitlement.md`). That answer is read in the background and
//! never on a path anyone waits for: at launch when it is due, and after a sign-in. What it says is
//! kept in the preferences so a launch without a network draws what it was last told. An own
//! endpoint, when one is set up, wins over the relay (`crate::ai_endpoint`).
//!
//! The balance is display only: every relay answer carries it and the core records it, and
//! [`MailcalApp::refresh_ai_balance`] asks for it outright.

use allodia_license::{AccountService, Cache, Capability, Error, Feature, Outcome, Relay, Stored};
use mailcal_ai::{AiError, Destination, GatedBackend};

use crate::{MailcalApp, ai_transport::AiTransport, allodia_transport::HttpsTransport};

impl MailcalApp {
    /// Whether somebody is signed in and the last entitlement answer, inside its grace, grants
    /// `ai`.
    pub(crate) fn ai_entitled(&self) -> bool {
        if self.allodia.lock().expect("allodia account lock").is_none() {
            return false;
        }
        let now = time::OffsetDateTime::now_utc().unix_timestamp();
        self.entitlement_cache()
            .effective(now)
            .grants(&Capability::Ai)
    }

    /// The relay behind the gate, when the person is signed in and entitled to `ai`.
    pub(crate) fn relay_backend(&self) -> Option<GatedBackend> {
        if !self.ai_entitled() {
            return None;
        }
        let transport = AiTransport::new(self.runtime.handle().clone()).ok()?;
        let this = self.this.clone();
        let relay = Relay::new(
            &AccountService::new(allodia_license::host()),
            Box::new(move || {
                let app = this.upgrade().ok_or(AiError::Unauthorized)?;
                // A grant from before the scope existed cannot be used for this, and asking would
                // only be refused: the Writing style screen offers to sign in again instead.
                if !app.allodia_grant_permits(Feature::UseAi) {
                    return Err(AiError::Unauthorized);
                }
                app.allodia_access_token("writing style")
                    .map_err(|error| match error {
                        crate::MailcalError::Connect(_) => AiError::Unreachable,
                        _ => AiError::Unauthorized,
                    })
            }),
            Box::new(transport),
        );
        Some(GatedBackend::new(
            Box::new(relay),
            Destination::AllodiaRelay,
            self.app.jurisdiction_mode_source(),
        ))
    }

    /// The stored entitlement answer, or nothing when none is stored or it does not read.
    fn entitlement_cache(&self) -> Cache {
        Cache::restore(
            self.app
                .entitlement_answer()
                .and_then(|json| serde_json::from_str::<Stored>(&json).ok()),
        )
    }

    /// Asks for the entitlement in the background when it is due, then rebuilds the backend.
    pub(crate) fn refresh_entitlement_in_background(&self) {
        let now = time::OffsetDateTime::now_utc().unix_timestamp();
        let signed_in = self.allodia.lock().expect("allodia account lock").is_some();
        if !signed_in || !self.entitlement_cache().should_refresh(now) {
            return;
        }
        let this = self.this.clone();
        self.runtime.spawn_blocking(move || {
            if let Some(app) = this.upgrade() {
                app.refresh_entitlement();
            }
        });
    }

    /// Asks the service what the account is entitled to, keeps the answer, and rebuilds the
    /// backend. **Blocking.** An unreachable service changes nothing; an answer replaces what was
    /// stored whatever it says.
    fn refresh_entitlement(&self) {
        if !self.allodia_grant_permits(Feature::Entitlement) {
            return;
        }
        let Ok(token) = self.allodia_access_token("the plan") else {
            return;
        };
        let Ok(transport) = HttpsTransport::new(self.runtime.handle().clone()) else {
            return;
        };
        let service = AccountService::new(allodia_license::host());
        let outcome = match service.entitlement(&transport, &token) {
            Ok(answer) => Outcome::Answered(answer),
            Err(Error::Unauthorized) => {
                self.forget_allodia_access_token();
                return;
            }
            Err(_) => Outcome::Unreachable,
        };
        let now = time::OffsetDateTime::now_utc().unix_timestamp();
        let mut cache = self.entitlement_cache();
        cache.apply(outcome, now);
        let stored = cache
            .stored()
            .and_then(|stored| serde_json::to_string(stored).ok());
        self.app.set_entitlement_answer(stored);
        log::info!("allodia: the entitlement was read");
        self.refresh_ai_backend();
    }

    /// Asks the relay how many credits are left and records the answer. **Blocking.**
    pub(crate) fn fetch_ai_balance(&self) -> Result<(), crate::WritingStyleFailure> {
        use crate::WritingStyleFailure;
        if !self.allodia_grant_permits(Feature::UseAi) {
            return Err(WritingStyleFailure::Unauthorized);
        }
        let token = self
            .allodia_access_token("the credits")
            .map_err(|_| WritingStyleFailure::Unreachable)?;
        let transport = HttpsTransport::new(self.runtime.handle().clone())
            .map_err(|_| WritingStyleFailure::Unreachable)?;
        let balance = AccountService::new(allodia_license::host())
            .ai_balance(&transport, &token)
            .map_err(|error| match error {
                Error::Unauthorized => WritingStyleFailure::Unauthorized,
                Error::Transport(_) => WritingStyleFailure::Unreachable,
                _ => WritingStyleFailure::Malformed,
            })?;
        self.app.set_ai_balance(balance.balance_credits);
        Ok(())
    }
}
