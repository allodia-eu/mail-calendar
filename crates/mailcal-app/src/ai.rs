//! The writing assistant in the core (`docs/ai.md`): whether it is offered, where a request goes,
//! what can go wrong and what that offers, the work that sends, and the stores it keeps.
//!
//! Each surface of the feature (Settings → Writing style, Draft a reply, the own endpoint's form)
//! is a record read from here and changed through intents; this module holds what all of them
//! share. The ports it reaches the outside through are in [`AiHost`] and [`AiRelay`].
//!
//! **Privacy.** A style, a passage, a draft and an endpoint's address are the person's own; none
//! is logged (`docs/logging.md`).

use std::{
    path::Path,
    sync::{Arc, Mutex, MutexGuard},
};

use engine_api::Provider;
use mailcal_account::AiPreferences;
use mailcal_ai::ModeSource;
pub use mailcal_ai::{Class, Mode};

mod drafted;
mod endpoint;
mod failure;
mod host;
mod offer;
mod own_mail;
mod reply_draft;
mod reply_draft_flow;
mod reply_draft_work;
mod route;
mod styles;
mod worker;
mod writing_style;

pub use endpoint::{
    AiEndpointAddressGap, AiEndpointChange, AiEndpointChangeKind, AiEndpointDeclarationGap,
    AiEndpointEdit, AiEndpointFields, AiEndpointGaps, AiEndpointOutcome, AiEndpointResult,
    AiEndpointView, ai_endpoint_gaps,
};
pub use failure::{AiFailure, AiRemedy};
pub use host::{AiCancel, AiHost, AiRelay, KeyNotStored, RelayStatus};
pub use mailcal_account::{DRAFTED_REPLY_KEYWORD, StoredWritingStyle, WritingStyleId};
pub use offer::{AiEntitlement, BuildKind, ai_offered};
pub(crate) use reply_draft::ReplyDraftHeld;
pub use reply_draft::{
    REPLY_DRAFT_SUGGESTIONS, ReplyDraftControl, ReplyDraftItem, ReplyDraftScript, ReplyDraftState,
    ReplyDraftSuggestion, ReplyDraftView,
};
pub use reply_draft_flow::ReplyDraftStart;
pub use route::AiRoute;
pub use styles::StylesWritten;
pub use worker::{AiQueued, AiRun, AiWork, AiWorkKey};
pub use writing_style::*;

use crate::{App, Surface, preferences_file::PreferencesFile};

/// Everything the assistant holds, as one field of [`App`].
pub(crate) struct AiState<P> {
    /// The `[ai]` table as it stands: read from the preferences once, then kept here and written
    /// through `prefs`. Shared with the gate's mode source, so a request reads the mode in force.
    settings: Arc<Mutex<AiPreferences>>,
    prefs: PreferencesFile,
    styles: Mutex<styles::StyleLibrary>,
    drafted: Mutex<drafted::DraftedLog>,
    ports: Mutex<host::Ports>,
    endpoint: Mutex<endpoint::EndpointWork>,
    work: Mutex<worker::Queue<P>>,
    writing_style: writing_style::WritingStyleState,
}

impl<P> AiState<P> {
    /// The state over the app data directory `dir` (none: nothing is kept, as in the demo and most
    /// tests) and the shared preferences file. `on_failure` is told when whether one of the
    /// assistant's own files could be written changes.
    pub(crate) fn new(
        dir: Option<&Path>,
        prefs: PreferencesFile,
        on_failure: impl Fn() + Clone + Send + Sync + 'static,
    ) -> Self {
        Self {
            settings: Arc::new(Mutex::new(*prefs.load().ai)),
            prefs,
            styles: Mutex::new(styles::StyleLibrary::new(dir, on_failure.clone())),
            drafted: Mutex::new(drafted::DraftedLog::new(dir, on_failure)),
            ports: Mutex::new(host::Ports::default()),
            endpoint: Mutex::new(endpoint::EndpointWork::default()),
            work: Mutex::new(worker::Queue::default()),
            writing_style: writing_style::WritingStyleState::default(),
        }
    }

    fn settings(&self) -> MutexGuard<'_, AiPreferences> {
        self.settings.lock().expect("ai settings mutex poisoned")
    }

    fn ports(&self) -> MutexGuard<'_, host::Ports> {
        self.ports.lock().expect("ai ports mutex poisoned")
    }

    /// Changes the `[ai]` table here and in the preferences file, behind the caller.
    fn edit_settings(&self, edit: impl Fn(&mut AiPreferences) + Send + Sync + 'static) {
        edit(&mut self.settings());
        self.prefs.edit(move |prefs| edit(&mut prefs.ai));
    }
}

impl<P: Provider> App<P> {
    /// Installs the host's ports, once, as the bindings build the app; until then nothing can be
    /// sent and the feature is not offered.
    pub fn install_ai_host(&self, host: Arc<dyn AiHost>, build: BuildKind) {
        {
            let mut ports = self.ai.ports();
            ports.host = Some(host);
            ports.build = build;
        }
        self.signal_ai_surfaces();
    }

    /// Installs Allodia's relay, in a build that can sign in.
    pub fn install_ai_relay(&self, relay: Arc<dyn AiRelay>) {
        self.ai.ports().relay = Some(relay);
        self.signal_ai_surfaces();
    }

    /// Tells the core the relay's [`RelayStatus`] may have moved, so every surface that names
    /// where a request goes is read again.
    pub fn ai_relay_changed(&self) {
        self.signal_ai_surfaces();
    }

    /// What the Allodia entitlement says about the assistant, from the licence side whenever its
    /// answer changes. A change of offer takes effect at once (`docs/ai.md` rule 5).
    pub fn set_ai_entitlement(&self, entitlement: AiEntitlement) {
        let changed = {
            let mut ports = self.ai.ports();
            std::mem::replace(&mut ports.entitlement, entitlement) != entitlement
        };
        if changed {
            log::info!("ai: the entitlement changed ({entitlement:?})");
            self.signal_ai_surfaces();
        }
    }

    /// Whether the feature is offered ([`ai_offered`]): every surface of it reads this, so no
    /// client asks apart. Read from memory.
    #[must_use]
    pub fn ai_offer(&self) -> bool {
        let ports = self.ai.ports();
        ai_offered(ports.build, ports.entitlement)
    }

    /// The jurisdiction mode in force.
    #[must_use]
    pub fn ai_mode(&self) -> Mode {
        self.ai.settings().jurisdiction
    }

    /// The mode in force at the moment of each request, for a `GatedBackend`.
    pub(crate) fn ai_mode_source(&self) -> ModeSource {
        let settings = Arc::clone(&self.ai.settings);
        Arc::new(move || {
            settings
                .lock()
                .expect("ai settings mutex poisoned")
                .jurisdiction
        })
    }

    /// The own endpoint's key, read from the keystore by the bindings at launch.
    pub fn restore_ai_endpoint_key(&self, key: Option<String>) {
        self.ai.ports().key = key.filter(|key| !key.trim().is_empty());
    }

    /// Every surface the feature draws, signalled when whether it is offered or where a request
    /// goes may have moved.
    pub(crate) fn signal_ai_surfaces(&self) {
        for surface in [
            Surface::Settings,
            Surface::WritingStyle,
            Surface::ReplyDraft,
            Surface::AiEndpoint,
        ] {
            self.observer.surface_changed(surface);
        }
    }

    /// Drops what the assistant keeps for `account`: its style slot and its drafted replies.
    pub(crate) fn forget_account_writing_assistant(&self, account: &str) {
        let account = account.to_owned();
        self.forget_drafted_replies(&account);
        self.ai.edit_settings(move |settings| {
            settings.writing_styles.remove(&account);
        });
        self.observer.surface_changed(Surface::WritingStyle);
    }
}

#[cfg(test)]
mod tests_endpoint;
#[cfg(test)]
mod tests_route;
#[cfg(test)]
mod tests_stores;
#[cfg(test)]
pub(crate) mod tests_support;
#[cfg(test)]
mod tests_worker;
