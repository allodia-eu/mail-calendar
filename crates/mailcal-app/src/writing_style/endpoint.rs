//! The backend slot, the jurisdiction mode, and the own endpoint's stored settings.
//!
//! The binding layer decides which backend exists and hands it over; the core only holds it and
//! reports whether there is one. An own endpoint's address, model and declaration are ordinary
//! preferences; its key is a secret and lives in the platform keystore, which the binding layer
//! owns.

use std::sync::Arc;

use engine_api::Provider;
use mailcal_account::{AiEndpoint, load_preferences};
use mailcal_ai::{GatedBackend, Mode, ModeSource};

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
}
