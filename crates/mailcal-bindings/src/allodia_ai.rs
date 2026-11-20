//! The Allodia half of the writing assistant (`docs/ai.md` rules 2 and 5): the relay as the core's
//! [`AiRelay`], and what the stored entitlement says about `ai`, fed to the core's offer.
//!
//! Both are read from memory. The entitlement is read from the service in the background
//! (`allodia_entitlement.rs`); what reaches the offer is the stored answer inside its grace,
//! published again whenever it may have moved: at launch, after each read, on each refresh
//! trigger (so a grace that runs out takes the offer with it) and at sign-out.

use std::sync::{Arc, Weak};

use allodia_license::{AccountService, Capability, Feature, Relay};
use mailcal_ai::{AiBackend, AiError, HttpTransport, wire};
use mailcal_app::ai::{AiEntitlement, AiRelay, RelayStatus};

use crate::{AllodiaGrantHealth, MailcalApp, allodia::StoredAccount};

/// Whether the relay can be asked, from who is signed in and what is known of the grant.
///
/// A grant the service refused, or one recorded without `mailcal:ai:use`, has to be made again; a
/// grant whose scopes were never recorded concludes nothing, and the relay's own answer decides.
pub(crate) fn relay_status(
    signed_in: Option<&StoredAccount>,
    health: AllodiaGrantHealth,
) -> RelayStatus {
    let Some(stored) = signed_in else {
        return RelayStatus::SignedOut;
    };
    if health == AllodiaGrantHealth::SignedOut
        || !crate::allodia_health::grant_permits(stored.granted_scopes.as_ref(), Feature::UseAi)
    {
        RelayStatus::SignInNeeded
    } else {
        RelayStatus::Ready
    }
}

/// The relay as the core reaches it. Holds the app weakly: the core holds this.
struct AllodiaRelay {
    app: Weak<MailcalApp>,
}

impl AiRelay for AllodiaRelay {
    fn status(&self) -> RelayStatus {
        self.app
            .upgrade()
            .map_or(RelayStatus::SignedOut, |app| app.allodia_relay_status())
    }

    fn backend(&self, transport: Box<dyn HttpTransport>) -> Box<dyn AiBackend> {
        let app = self.app.clone();
        let relay = Relay::new(
            &AccountService::new(allodia_license::host()),
            Box::new(move || {
                app.upgrade()
                    .ok_or(AiError::Unauthorized)?
                    .allodia_relay_token()
            }),
            transport,
        );
        Box::new(Heeded {
            relay,
            app: self.app.clone(),
        })
    }
}

/// The relay, acting on what its refusals say about this device's own state.
struct Heeded {
    relay: Relay,
    app: Weak<MailcalApp>,
}

impl AiBackend for Heeded {
    fn chat(&self, request: &wire::ChatRequest) -> Result<wire::ChatResponse, AiError> {
        let answered = self.relay.chat(request);
        if let (Err(error), Some(app)) = (&answered, self.app.upgrade()) {
            match error {
                // The token held for the process is not one the relay takes; the next request
                // mints another rather than presenting it again.
                AiError::Unauthorized => app.forget_allodia_access_token(),
                // The relay is the server's word that `ai` is gone, so the stored answer is out of
                // date: read it now, and the offer follows when the answer lands.
                AiError::NotEntitled => app.read_allodia_entitlement_in_background(),
                _ => {}
            }
        }
        answered
    }
}

impl MailcalApp {
    /// Installs the relay in the core and tells it what the stored entitlement says. Called once,
    /// as the app is built.
    pub(crate) fn install_allodia_relay(self: &Arc<Self>) {
        let relay = AllodiaRelay {
            app: Arc::downgrade(self),
        };
        self.app.install_ai_relay(Arc::new(relay));
        self.publish_ai_entitlement();
    }

    /// Whether the relay can be asked now. Read from memory.
    pub(crate) fn allodia_relay_status(&self) -> RelayStatus {
        let signed_in = self.allodia.lock().expect("allodia account lock");
        relay_status(signed_in.as_ref(), self.allodia_health())
    }

    /// An access token for the relay. **Blocking**: it may refresh the grant, on the assistant's
    /// own thread.
    fn allodia_relay_token(&self) -> Result<String, AiError> {
        if !self.allodia_grant_permits(Feature::UseAi) {
            return Err(AiError::Unauthorized);
        }
        self.allodia_access_token("the writing assistant")
            .map_err(|_| match self.allodia_relay_status() {
                // Nothing was learned about the grant: the service could not be asked.
                RelayStatus::Ready => AiError::Unreachable,
                RelayStatus::SignInNeeded | RelayStatus::SignedOut => AiError::Unauthorized,
            })
    }

    /// Tells the core what the stored entitlement grants now.
    pub(crate) fn publish_ai_entitlement(&self) {
        self.publish_ai_entitlement_at(time::OffsetDateTime::now_utc().unix_timestamp());
    }

    /// As [`Self::publish_ai_entitlement`], at `now` in Unix seconds.
    ///
    /// A sign-in the service refused grants nothing here, unlike everywhere else the stored answer
    /// is read: the grace exists because a paid capability cannot work without Allodia, and the
    /// own endpoint can.
    pub(crate) fn publish_ai_entitlement_at(&self, now: i64) {
        let signed_in = self.allodia.lock().expect("allodia account lock").is_some();
        let refused = self.allodia_health() == AllodiaGrantHealth::SignedOut;
        let granted =
            signed_in && !refused && self.allodia_entitlement.grants(&Capability::Ai, now);
        self.app.set_ai_entitlement(if granted {
            AiEntitlement::Granted
        } else {
            AiEntitlement::NotGranted
        });
    }
}

#[cfg(test)]
#[path = "allodia_ai_tests.rs"]
mod tests;
