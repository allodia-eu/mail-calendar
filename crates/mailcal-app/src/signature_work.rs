//! Saving and deleting a signature (`docs/signatures.md`, "Changing the library";
//! `docs/waiting.md`).
//!
//! A change is queued on the host's thread and run off it: a save sanitises a body that can carry
//! half a megabyte of picture, and both wait for the library's file to be written, which is what
//! the person waits for before the editor closes. One change runs at a time; one asked for while
//! another runs waits and runs next, and the same change asked for again is dropped, so a double
//! press on Save saves once. What the last change came to is kept until the next one starts.

use std::collections::VecDeque;

use engine_api::Provider;
use mailcal_account::{SignatureId, StoredSignature};
use mailcal_viewmodel::{
    SignatureChangeKind, SignatureOutcome, SignatureRefusal, SignatureResult, SignatureWork,
};

use super::{
    mint_signature_id,
    rules::{body_images, signature_name},
};
use crate::{App, Surface};

/// A change to the signature library.
#[derive(Clone, PartialEq, Eq)]
pub enum SignatureChange {
    /// Stores a signature under `id`: one in the library, or the view's `new_id` for a new one.
    Save {
        /// The signature it stores.
        id: String,
        /// Its name as typed; stored without the spaces around it.
        name: String,
        /// The editor's HTML, sanitised before it is stored.
        body_html: String,
        /// Its plain-text rendering, for a message's text part.
        body_plain: String,
    },
    /// Deletes the signature `id`, and clears it from every account that used it.
    Delete {
        /// The signature it deletes.
        id: String,
    },
}

/// The body is the person's own writing, so it is never printed (`docs/logging.md`).
impl std::fmt::Debug for SignatureChange {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let work = self.work();
        f.debug_struct("SignatureChange")
            .field("id", &work.id)
            .field("kind", &work.kind)
            .finish_non_exhaustive()
    }
}

impl SignatureChange {
    fn work(&self) -> SignatureWork {
        match self {
            Self::Save { id, .. } => SignatureWork {
                id: id.clone(),
                kind: SignatureChangeKind::Save,
            },
            Self::Delete { id } => SignatureWork {
                id: id.clone(),
                kind: SignatureChangeKind::Delete,
            },
        }
    }
}

/// The changes running and waiting, and what the last one came to.
#[derive(Default)]
pub(crate) struct Work {
    running: Option<SignatureChange>,
    waiting: VecDeque<SignatureChange>,
    pub(crate) outcome: Option<SignatureOutcome>,
    serial: u64,
}

impl Work {
    /// The change running, then those waiting.
    pub(crate) fn listed(&self) -> Vec<SignatureWork> {
        self.running
            .iter()
            .chain(&self.waiting)
            .map(SignatureChange::work)
            .collect()
    }

    /// Queues `change`. Whether it is to run now: nothing else was running.
    fn queue(&mut self, change: SignatureChange) -> Queued {
        let work = change.work();
        if self
            .running
            .iter()
            .chain(&self.waiting)
            .any(|queued| queued.work() == work)
        {
            return Queued::Dropped;
        }
        if self.running.is_some() {
            self.waiting.push_back(change);
            return Queued::Waiting;
        }
        self.outcome = None;
        self.running = Some(change);
        Queued::Run
    }

    /// Records what the running change came to and starts the next.
    fn finish(&mut self, result: SignatureResult) -> Option<SignatureChange> {
        if let Some(done) = self.running.take() {
            let work = done.work();
            self.serial += 1;
            self.outcome = Some(SignatureOutcome {
                serial: self.serial,
                id: work.id,
                kind: work.kind,
                result,
            });
        }
        self.running = self.waiting.pop_front();
        if self.running.is_some() {
            self.outcome = None;
        }
        self.running.clone()
    }
}

/// What queueing a change did.
enum Queued {
    /// Nothing else was running: the caller runs it.
    Run,
    /// It runs after the change running now.
    Waiting,
    /// The same change is already running or waiting.
    Dropped,
}

impl<P: Provider> App<P> {
    /// Queues a change to the signature library, and signals [`Surface::Signatures`]. Returns
    /// whether the caller is to run the queue with [`App::run_signature_changes`], off the host's
    /// thread: nothing else was running. Returns at once.
    pub fn queue_signature_change(&self, change: SignatureChange) -> bool {
        let queued = self
            .signatures
            .lock()
            .expect("signatures mutex poisoned")
            .work
            .queue(change);
        if matches!(queued, Queued::Dropped) {
            return false;
        }
        self.observer.surface_changed(Surface::Signatures);
        matches!(queued, Queued::Run)
    }

    /// Runs the change [`App::queue_signature_change`] started, and every one queued behind it,
    /// each until its library write has ended. Not for a UI thread.
    pub fn run_signature_changes(&self) {
        let mut next = self
            .signatures
            .lock()
            .expect("signatures mutex poisoned")
            .work
            .running
            .clone();
        while let Some(change) = next {
            let result = self.apply_signature_change(change);
            next = self
                .signatures
                .lock()
                .expect("signatures mutex poisoned")
                .work
                .finish(result);
            self.observer.surface_changed(Surface::Signatures);
        }
    }

    /// Queues `change` and runs it on this thread, for a boot that seeds the library and for
    /// tests. Not for a UI thread.
    pub fn change_signature_now(&self, change: SignatureChange) {
        if self.queue_signature_change(change) {
            self.run_signature_changes();
        }
    }

    fn apply_signature_change(&self, change: SignatureChange) -> SignatureResult {
        let refused = |reason| SignatureResult::Refused { reason };
        match change {
            SignatureChange::Save {
                id,
                name,
                body_html,
                body_plain,
            } => {
                let name = match signature_name(&name) {
                    Ok(name) => name,
                    Err(reason) => return refused(reason),
                };
                // Sanitised as it is stored, not only as it is sent: what comes out of the
                // library is assigned into the composer's editor with `innerHTML`, whose page
                // permits inline handlers, and `signatures.toml` is a file anything with disk
                // access can edit (`docs/composer-security.md`, Gate 10).
                let body_html = engine_html::sanitize(&body_html).into_html();
                if let Err(refusal) = body_images(&body_html) {
                    return refused(SignatureRefusal::Image { refusal });
                }
                let signature = StoredSignature {
                    name,
                    body_html,
                    body_plain,
                };
                let Some(id) = SignatureId::new(id) else {
                    return refused(SignatureRefusal::Gone);
                };
                let file = {
                    let mut state = self.signatures.lock().expect("signatures mutex poisoned");
                    if id == state.new_id {
                        state.new_id = mint_signature_id();
                        log::info!("signatures: created one signature");
                    } else if !state.library.entries.contains_key(&id) {
                        // An edit never makes a signature the person thought they were changing.
                        return refused(SignatureRefusal::Gone);
                    }
                    state.put(id, signature);
                    state.file.clone()
                };
                file.settle();
                if file.failed() {
                    SignatureResult::NotWritten
                } else {
                    SignatureResult::Saved
                }
            }
            SignatureChange::Delete { id } => {
                let file = {
                    let mut state = self.signatures.lock().expect("signatures mutex poisoned");
                    if let Some(id) = SignatureId::new(id) {
                        state.delete(&id);
                    }
                    state.file.clone()
                };
                log::info!("signatures: deleted one signature");
                file.settle();
                if file.failed() {
                    SignatureResult::NotWritten
                } else {
                    SignatureResult::Deleted
                }
            }
        }
    }
}

#[cfg(test)]
#[path = "tests_signature_work.rs"]
mod tests;
