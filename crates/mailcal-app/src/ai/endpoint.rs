//! The own endpoint's form (`docs/ai.md` rules 6 and 7): four fields judged by the core, a Save
//! and a Remove that run until the keystore and the preferences are written, and what the last
//! change came to.
//!
//! [`ai_endpoint_gaps`] is what the form shows beside its fields before Save is pressed, and what
//! the save refuses by, so the two cannot disagree. Saving dials nothing.

use std::fmt;

use engine_api::Provider;
use mailcal_account::AiEndpoint;
use mailcal_ai::{Class, EndpointError, Mode, OwnEndpoint, Refused, endpoint_errors, same_server};

use super::KeyNotStored;
use crate::App;

/// What is wrong with the address.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AiEndpointAddressGap {
    /// None was given.
    Missing,
    /// It is not a URL, or carries a user name, a query or a fragment.
    Unusable,
    /// It is plain HTTP to somewhere other than this device.
    NeedsHttps,
}

/// What is wrong with the declaration of where the endpoint runs.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AiEndpointDeclarationGap {
    /// Nothing was declared.
    Missing,
    /// The gate refuses what was declared under the mode in force.
    Refused(Refused),
}

/// What a save with the key field empty does to the key kept (`docs/ai.md` rule 7).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AiEndpointKeptKey {
    /// It stays: the address is on the stored endpoint's server.
    Carried,
    /// It is taken out: the address is on another server, or none is stored, and the key was
    /// given for that one.
    Dropped,
}

/// Every field's problem at once; a field with none has none here. Beside them, what the save
/// would do to the key kept, which refuses nothing.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct AiEndpointGaps {
    /// The address's.
    pub address: Option<AiEndpointAddressGap>,
    /// Whether the model is missing.
    pub model_missing: bool,
    /// The declaration's.
    pub declaration: Option<AiEndpointDeclarationGap>,
    /// What a save with the key field empty does to the key kept; `None` while none is kept.
    pub kept_key: Option<AiEndpointKeptKey>,
}

impl AiEndpointGaps {
    /// Whether a save would go ahead.
    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.address.is_none() && !self.model_missing && self.declaration.is_none()
    }
}

/// What a save of these fields would refuse, under `mode`. The key is not judged: it may be empty.
/// What the save does to a key kept is the app's to say ([`App::ai_endpoint_gap`]).
#[must_use]
pub fn ai_endpoint_gaps(
    address: &str,
    model: &str,
    declared: Option<Class>,
    mode: Mode,
) -> AiEndpointGaps {
    let mut gaps = AiEndpointGaps::default();
    for error in endpoint_errors(address, model) {
        match error {
            EndpointError::NoAddress => gaps.address = Some(AiEndpointAddressGap::Missing),
            EndpointError::InvalidUrl => gaps.address = Some(AiEndpointAddressGap::Unusable),
            EndpointError::NotHttps => gaps.address = Some(AiEndpointAddressGap::NeedsHttps),
            EndpointError::NoModel => gaps.model_missing = true,
        }
    }
    gaps.declaration = match declared {
        None => Some(AiEndpointDeclarationGap::Missing),
        Some(class) => mailcal_jurisdiction::gate(mode, class)
            .err()
            .map(AiEndpointDeclarationGap::Refused),
    };
    gaps
}

/// The fields a save is asked for with.
#[derive(Clone, PartialEq, Eq)]
pub struct AiEndpointEdit {
    /// The address as typed.
    pub address: String,
    /// The model as typed.
    pub model: String,
    /// Where the person declared it runs.
    pub declared: Option<Class>,
    /// The key as typed, which replaces the one kept. Empty keeps the kept key while the address
    /// stays on the stored endpoint's server, writing nothing to the keystore, and takes it out
    /// otherwise (`docs/ai.md` rule 7).
    pub key: String,
}

/// The address and the key are the person's configuration, and never printed.
impl fmt::Debug for AiEndpointEdit {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("AiEndpointEdit")
            .field("declared", &self.declared)
            .field("key", &!self.key.trim().is_empty())
            .finish_non_exhaustive()
    }
}

/// A change to the own endpoint.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AiEndpointChange {
    /// Saves the fields, and the key when one was typed.
    Save(AiEndpointEdit),
    /// Removes the endpoint and its key.
    Remove,
}

/// Which change.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AiEndpointChangeKind {
    /// A save.
    Save,
    /// A removal.
    Remove,
}

impl AiEndpointChange {
    const fn kind(&self) -> AiEndpointChangeKind {
        match self {
            Self::Save(_) => AiEndpointChangeKind::Save,
            Self::Remove => AiEndpointChangeKind::Remove,
        }
    }
}

/// What a change came to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AiEndpointResult {
    /// Saved: the key in the keystore and the rest in the preferences.
    Saved,
    /// Removed, the key with it.
    Removed,
    /// Not saved, for these reasons; the form keeps the edit.
    Refused {
        /// Every field's problem.
        gaps: AiEndpointGaps,
    },
    /// The keystore refused the key, so nothing was changed; the form keeps the edit.
    KeyNotStored,
    /// Made, and the preferences could not be written, so it holds until the app closes. The
    /// failed-save notice says so.
    NotWritten,
}

/// The last change and what it came to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AiEndpointOutcome {
    /// Grows with each change, so a client acts on each outcome once.
    pub serial: u64,
    /// Which change.
    pub kind: AiEndpointChangeKind,
    /// What it came to.
    pub result: AiEndpointResult,
}

/// The fields as the form draws them.
#[derive(Clone, PartialEq, Eq)]
pub struct AiEndpointFields {
    /// The address.
    pub address: String,
    /// The model.
    pub model: String,
    /// Where it runs.
    pub declared: Option<Class>,
    /// Whether a key is kept, which Remove takes out. The key itself is never drawn back, and
    /// whether a save keeps it is [`AiEndpointGaps::kept_key`], asked of the address as typed.
    pub key_kept: bool,
}

impl fmt::Debug for AiEndpointFields {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("AiEndpointFields")
            .field("declared", &self.declared)
            .field("key_kept", &self.key_kept)
            .finish_non_exhaustive()
    }
}

/// The form, as the core holds it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AiEndpointView {
    /// Whether the feature is offered, and with it the form.
    pub offered: bool,
    /// The fields: from the edit being saved, then from an edit a save refused, otherwise from
    /// what is stored.
    pub fields: AiEndpointFields,
    /// Whether an endpoint is stored.
    pub stored: bool,
    /// Whether Remove is offered: an endpoint or a key is stored, or a removal runs or waits.
    pub removable: bool,
    /// The change running, then any waiting behind it.
    pub work: Vec<AiEndpointChangeKind>,
    /// What the last change came to, until the next starts.
    pub outcome: Option<AiEndpointOutcome>,
}

/// The changes running and waiting, the edit the last save refused, and the last outcome.
#[derive(Default)]
pub(crate) struct EndpointWork {
    running: Option<AiEndpointChange>,
    waiting: Vec<AiEndpointChange>,
    refused: Option<AiEndpointEdit>,
    outcome: Option<AiEndpointOutcome>,
    serial: u64,
}

impl EndpointWork {
    fn finish(&mut self, kind: AiEndpointChangeKind, result: AiEndpointResult) {
        self.serial += 1;
        self.outcome = Some(AiEndpointOutcome {
            serial: self.serial,
            kind,
            result,
        });
        self.running = (!self.waiting.is_empty()).then(|| self.waiting.remove(0));
        if self.running.is_some() {
            self.outcome = None;
        }
    }
}

impl<P: Provider> App<P> {
    fn endpoint_work(&self) -> std::sync::MutexGuard<'_, EndpointWork> {
        self.ai.endpoint.lock().expect("ai endpoint mutex poisoned")
    }

    /// The own endpoint's form. Read from memory, so a UI thread may ask it.
    #[must_use]
    pub fn ai_endpoint(&self) -> AiEndpointView {
        let stored = self.ai.settings().endpoint.clone();
        let key_kept = self.ai.ports().key.is_some();
        let work = self.endpoint_work();
        let editing = match &work.running {
            Some(AiEndpointChange::Save(edit)) => Some(edit),
            _ => work.refused.as_ref(),
        };
        let fields = match (editing, &stored) {
            (Some(edit), _) => AiEndpointFields {
                address: edit.address.clone(),
                model: edit.model.clone(),
                declared: edit.declared,
                key_kept,
            },
            (None, Some(stored)) => AiEndpointFields {
                address: stored.base_url.clone(),
                model: stored.model.clone(),
                declared: stored.declared,
                key_kept,
            },
            (None, None) => AiEndpointFields {
                address: String::new(),
                model: String::new(),
                declared: None,
                key_kept,
            },
        };
        let queued: Vec<_> = work
            .running
            .iter()
            .chain(&work.waiting)
            .map(AiEndpointChange::kind)
            .collect();
        AiEndpointView {
            offered: self.ai_offer(),
            removable: stored.is_some()
                || fields.key_kept
                || queued.contains(&AiEndpointChangeKind::Remove),
            fields,
            stored: stored.is_some(),
            work: queued,
            outcome: work.outcome,
        }
    }

    /// What a save of these fields would refuse under the mode in force, [`ai_endpoint_gaps`]
    /// with the mode read here, and what it would do to the key kept.
    #[must_use]
    pub fn ai_endpoint_gap(
        &self,
        address: &str,
        model: &str,
        declared: Option<Class>,
    ) -> AiEndpointGaps {
        AiEndpointGaps {
            kept_key: self.kept_key_for(address),
            ..ai_endpoint_gaps(address, model, declared, self.ai_mode())
        }
    }

    /// What a save of `address` with the key field empty does to the key kept, or `None` while
    /// none is kept. A key goes only to the server it was given for.
    fn kept_key_for(&self, address: &str) -> Option<AiEndpointKeptKey> {
        self.ai.ports().key.as_ref()?;
        let stored = self.ai.settings().endpoint.clone();
        Some(
            if stored.is_some_and(|stored| same_server(&stored.base_url, address)) {
                AiEndpointKeptKey::Carried
            } else {
                AiEndpointKeptKey::Dropped
            },
        )
    }

    /// Queues a change to the own endpoint and signals [`crate::Surface::AiEndpoint`]. Whether the
    /// caller is to run [`App::run_ai_endpoint_changes`] off its own thread: nothing else was
    /// running. The same change asked for again while it runs or waits is dropped.
    pub fn queue_ai_endpoint_change(&self, change: AiEndpointChange) -> bool {
        let run = {
            let mut work = self.endpoint_work();
            if work.running.as_ref() == Some(&change) || work.waiting.contains(&change) {
                return false;
            }
            if work.running.is_some() {
                work.waiting.push(change);
                false
            } else {
                work.outcome = None;
                work.running = Some(change);
                true
            }
        };
        self.observer.surface_changed(crate::Surface::AiEndpoint);
        run
    }

    /// Runs the change [`App::queue_ai_endpoint_change`] started, and those queued behind it, each
    /// until the keystore and the preferences have answered. Not for a UI thread.
    pub fn run_ai_endpoint_changes(&self) {
        let mut next = self.endpoint_work().running.clone();
        while let Some(change) = next {
            let kind = change.kind();
            let result = self.apply_ai_endpoint_change(change);
            next = {
                let mut work = self.endpoint_work();
                work.finish(kind, result);
                work.running.clone()
            };
            self.signal_ai_surfaces();
        }
    }

    /// Queues `change` and runs it on this thread: for tests. Not for a UI thread.
    pub fn change_ai_endpoint_now(&self, change: AiEndpointChange) {
        if self.queue_ai_endpoint_change(change) {
            self.run_ai_endpoint_changes();
        }
    }

    fn apply_ai_endpoint_change(&self, change: AiEndpointChange) -> AiEndpointResult {
        match change {
            AiEndpointChange::Save(edit) => {
                let gaps = self.ai_endpoint_gap(&edit.address, &edit.model, edit.declared);
                let endpoint =
                    match OwnEndpoint::new(&edit.address, None, &edit.model, edit.declared) {
                        Ok(endpoint) if gaps.is_empty() => endpoint,
                        _ => {
                            self.endpoint_work().refused = Some(edit);
                            return AiEndpointResult::Refused { gaps };
                        }
                    };
                let key = edit.key.trim();
                let written = if !key.is_empty() {
                    self.write_endpoint_key(Some(key))
                } else if gaps.kept_key == Some(AiEndpointKeptKey::Dropped) {
                    log::info!("ai: the endpoint moved to another server, so its key was dropped");
                    self.write_endpoint_key(None)
                } else {
                    Ok(())
                };
                if written.is_err() {
                    self.endpoint_work().refused = Some(edit);
                    return AiEndpointResult::KeyNotStored;
                }
                self.endpoint_work().refused = None;
                let stored = AiEndpoint {
                    base_url: endpoint.base_url().to_owned(),
                    model: endpoint.model().to_owned(),
                    declared: endpoint.declared(),
                };
                self.ai
                    .edit_settings(move |settings| settings.endpoint = Some(stored.clone()));
                log::info!("ai: an own endpoint was saved");
                self.settled(AiEndpointResult::Saved)
            }
            AiEndpointChange::Remove => {
                let key_kept = self.ai.ports().key.is_some();
                if key_kept && self.write_endpoint_key(None).is_err() {
                    return AiEndpointResult::KeyNotStored;
                }
                self.endpoint_work().refused = None;
                self.ai.edit_settings(|settings| settings.endpoint = None);
                log::info!("ai: the own endpoint was removed");
                self.settled(AiEndpointResult::Removed)
            }
        }
    }

    /// `done`, once the preferences are written; [`AiEndpointResult::NotWritten`] when they could
    /// not be.
    fn settled(&self, done: AiEndpointResult) -> AiEndpointResult {
        self.prefs.settle();
        if self.prefs.failed() {
            AiEndpointResult::NotWritten
        } else {
            done
        }
    }

    /// Writes the key to the keystore through the host, and keeps it here once it is there.
    fn write_endpoint_key(&self, key: Option<&str>) -> Result<(), KeyNotStored> {
        let host = self.ai.ports().host.clone().ok_or(KeyNotStored)?;
        host.store_endpoint_key(key)?;
        self.ai.ports().key = key.map(str::to_owned);
        Ok(())
    }
}
