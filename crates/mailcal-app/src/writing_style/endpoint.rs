//! The backend slot, the jurisdiction mode, and the own endpoint's stored settings.
//!
//! The binding layer decides which backend exists and hands it over; the core only holds it and
//! reports whether there is one. An own endpoint's address, model and declaration are ordinary
//! preferences; its key is a secret and lives in the platform keystore, which the binding layer
//! owns.

use std::sync::Arc;

use engine_api::Provider;
use mailcal_account::{AiEndpoint, StoredBalance, load_preferences};
use mailcal_ai::{Destination, GatedBackend, Mode, ModeSource, wire::Metering};
use mailcal_viewmodel::CreditBalance;

use crate::{App, Surface};

impl<P: Provider> App<P> {
    /// Installs the backend AI requests go through, or removes it with `None`. Signals
    /// [`Surface::WritingStyle`], whose snapshot says whether AI is available.
    pub fn set_ai_backend(&self, backend: Option<GatedBackend>) {
        *self
            .writing_style
            .backend
            .lock()
            .expect("ai backend poisoned") = backend.map(Arc::new);
        self.observer.surface_changed(Surface::WritingStyle);
    }

    /// Whether a backend is installed.
    #[must_use]
    pub fn ai_available(&self) -> bool {
        self.writing_style.backend().is_some()
    }

    /// The mode the gate applies, read from the preferences at the moment of each request, so a
    /// change applies to the next one. Captures the file's path, not the app, so a backend holding
    /// it keeps nothing else alive.
    #[must_use]
    pub fn jurisdiction_mode_source(&self) -> ModeSource {
        let path = self.prefs_path.clone();
        Arc::new(move || {
            path.as_ref()
                .map(|path| load_preferences(path).jurisdiction_mode)
                .unwrap_or_default()
        })
    }

    /// The mode in force now.
    #[must_use]
    pub fn jurisdiction_mode(&self) -> Mode {
        (self.jurisdiction_mode_source())()
    }

    /// The own endpoint's stored address, model and declaration.
    #[must_use]
    pub fn own_ai_endpoint(&self) -> Option<AiEndpoint> {
        self.writing_style
            .edit_prefs(|prefs| prefs.ai.endpoint.clone())
    }

    /// Stores the own endpoint's address, model and declaration, or clears them with `None`. The
    /// caller validates first (`mailcal_ai::OwnEndpoint::new`) and keeps the key.
    pub fn set_own_ai_endpoint(&self, endpoint: Option<AiEndpoint>) {
        self.writing_style
            .edit_prefs(|prefs| prefs.ai.endpoint = endpoint);
    }

    /// The Allodia account service's last entitlement answer, as the licence half stored it.
    #[must_use]
    pub fn entitlement_answer(&self) -> Option<String> {
        self.writing_style
            .edit_prefs(|prefs| prefs.ai.entitlement_answer.clone())
    }

    /// Stores the entitlement answer for the licence half, or clears it with `None` (a sign-out).
    pub fn set_entitlement_answer(&self, answer: Option<String>) {
        self.writing_style
            .edit_prefs(|prefs| prefs.ai.entitlement_answer = answer);
    }

    /// The credits Allodia's relay last reported, when requests go through it.
    #[must_use]
    pub fn ai_balance(&self) -> Option<CreditBalance> {
        let relay = self
            .writing_style
            .backend()
            .is_some_and(|backend| backend.destination() == Destination::AllodiaRelay);
        let stored = self.writing_style.edit_prefs(|prefs| prefs.ai.balance)?;
        #[allow(
            clippy::cast_precision_loss,
            reason = "a balance in thousandths of a credit is far inside f64's exact range"
        )]
        relay.then(|| CreditBalance {
            credits: stored.millicredits as f64 / 1000.0,
            as_of: stored.as_of,
        })
    }

    /// Records the credits the relay reported, and signals [`Surface::WritingStyle`].
    pub fn set_ai_balance(&self, credits: f64) {
        #[allow(
            clippy::cast_possible_truncation,
            reason = "a credit balance is nowhere near i64's range in thousandths"
        )]
        let millicredits = (credits * 1000.0).round() as i64;
        let as_of = time::OffsetDateTime::now_utc().unix_timestamp();
        self.writing_style.edit_prefs(|prefs| {
            prefs.ai.balance = Some(StoredBalance {
                millicredits,
                as_of,
            });
        });
        self.observer.surface_changed(Surface::WritingStyle);
    }

    /// Records the balance a relay answer carried, when it carried one.
    pub(super) fn note_metering(&self, metering: Option<Metering>) {
        if let Some(metering) = metering {
            self.set_ai_balance(metering.balance_credits);
        }
    }
}
