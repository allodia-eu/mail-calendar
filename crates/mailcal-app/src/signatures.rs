//! The signature library's use cases: create / rename / edit / delete a signature, assign one to
//! an account's new-message or reply-forward slot, and resolve which body a composer opens with.
//!
//! Signatures are standalone entities shared across accounts (`docs/signatures.md`), so the
//! library lives in its own store (`signatures.toml`) and only the small per-account pointer
//! rides in `preferences.toml`; see [`mailcal_account::Signatures`] for why. This module holds
//! the loaded state and writes both back read-modify-write, so a sibling setting is never
//! clobbered. A second `impl App` block keeps `lib.rs` under the 500-line limit.
//!
//! **Privacy.** A signature body is user content. Nothing here logs it; counts, ids and lengths
//! only (`docs/logging.md`).

use std::path::PathBuf;

use engine_api::Provider;
use mailcal_account::{
    Preferences, SignatureId, SignatureSlot, Signatures, StoredSignature, load_preferences,
    load_signatures, save_preferences, save_signatures,
};
use mailcal_viewmodel::{AccountSignatureRow, SignatureRow, SignatureSlotKind, SignaturesSnapshot};

use crate::{App, Surface};

/// The loaded signature library and where the two files it spans are persisted.
pub(crate) struct SignatureState {
    library: Signatures,
    /// The library's own file. `None` disables persistence (the in-memory demo and tests), in
    /// which case the library lives only for this run.
    signatures_path: Option<PathBuf>,
    /// The shared preferences file, which carries the per-account assignments.
    prefs_path: Option<PathBuf>,
    /// The per-account assignments when there is **no** preferences file to hold them; the
    /// in-memory demo and showcase boots, and tests.
    ///
    /// Without this, "persistence is off" meant two different things in one struct: the library
    /// still lived for the run (`persist_library` simply no-ops over the in-memory `library`),
    /// while an assignment vanished the instant it was made, because it existed *only* in the
    /// file. So an in-memory boot could hold signatures that no account could be pointed at;
    /// which is what the showcase screenshots hit: a seeded library, and a Settings screen
    /// reporting every slot as None.
    memory: Preferences,
}

impl SignatureState {
    /// Loads the library from `signatures_path` (empty when absent or unreadable).
    pub(crate) fn new(signatures_path: Option<PathBuf>, prefs_path: Option<PathBuf>) -> Self {
        let library = signatures_path
            .as_ref()
            .map(load_signatures)
            .unwrap_or_default();
        Self {
            library,
            signatures_path,
            prefs_path,
            memory: Preferences::default(),
        }
    }

    /// The library in display order, as metadata rows.
    fn rows(&self) -> Vec<SignatureRow> {
        self.library
            .ordered()
            .into_iter()
            .map(|(id, signature)| SignatureRow {
                id: id.as_str().to_owned(),
                name: signature.name.clone(),
            })
            .collect()
    }

    /// One signature's HTML body, or `None` when the id names nothing.
    fn html(&self, id: &SignatureId) -> Option<String> {
        self.library.get(id).map(|entry| entry.body_html.clone())
    }

    /// One signature's HTML **and** plain-text bodies; what the composer needs to seed both
    /// parts of the outgoing message.
    fn bodies(&self, id: &SignatureId) -> Option<(String, String)> {
        self.library
            .get(id)
            .map(|entry| (entry.body_html.clone(), entry.body_plain.clone()))
    }

    /// Adds a signature under the minted `id` and persists the library.
    fn create(&mut self, id: SignatureId, signature: StoredSignature) {
        self.library.insert(id, signature);
        self.persist_library();
    }

    /// Replaces an existing signature's name and body. A no-op (returning `false`) for an id
    /// that names nothing: an edit must never silently create a signature the user thought
    /// they were changing.
    fn update(&mut self, id: &SignatureId, signature: StoredSignature) -> bool {
        if !self.library.entries.contains_key(id) {
            return false;
        }
        self.library.insert(id.clone(), signature);
        self.persist_library();
        true
    }

    /// Deletes a signature from the library **and** clears it from every account slot that
    /// pointed at it, so no assignment is left naming something that no longer exists.
    fn delete(&mut self, id: &SignatureId) -> bool {
        if !self.library.remove(id) {
            return false;
        }
        self.persist_library();
        if let Some(path) = &self.prefs_path {
            let mut prefs = load_preferences(path);
            if prefs.forget_signature(id) {
                let _ = save_preferences(path, &prefs);
            }
        } else {
            self.memory.forget_signature(id);
        }
        true
    }

    /// The signature assigned to `account`'s `slot`, if the account has one **and** it still
    /// resolves to a signature in the library. A stale pointer reads as "no signature" rather
    /// than an error: the file is hand-editable, and the composer must open either way.
    fn assigned(&self, account: &str, slot: SignatureSlot) -> Option<SignatureId> {
        let id = match &self.prefs_path {
            Some(path) => load_preferences(path)
                .account_signature(account)
                .slot(slot)
                .cloned()?,
            None => self.memory.account_signature(account).slot(slot).cloned()?,
        };
        self.library.entries.contains_key(&id).then_some(id)
    }

    /// Assigns (or clears) one of an account's slots, persisting through the shared preferences
    /// file read-modify-write so sibling settings survive.
    fn assign(&mut self, account: &str, slot: SignatureSlot, signature: Option<SignatureId>) {
        if let Some(path) = &self.prefs_path {
            let mut prefs = load_preferences(path);
            prefs.set_account_signature(account, slot, signature);
            let _ = save_preferences(path, &prefs);
        } else {
            self.memory.set_account_signature(account, slot, signature);
        }
    }

    /// Every account's stored assignment, for the settings snapshot.
    fn assignments(&self, account: &str) -> (Option<String>, Option<String>) {
        let assignment = match &self.prefs_path {
            Some(path) => load_preferences(path).account_signature(account),
            None => self.memory.account_signature(account),
        };
        (
            assignment.new_message.map(|id| id.as_str().to_owned()),
            assignment.reply_forward.map(|id| id.as_str().to_owned()),
        )
    }

    fn persist_library(&self) {
        if let Some(path) = &self.signatures_path {
            let _ = save_signatures(path, &self.library);
        }
    }
}

/// The two bodies a composer seeds a signature from: the HTML that goes into the editor, and the
/// plain text that rides alongside it into the outgoing `text/plain` part.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SignatureBody {
    /// The signature's id, so a host can show which one is selected in its picker.
    pub id: String,
    /// The HTML fragment to seed into the editor.
    pub body_html: String,
    /// The plain-text rendering that accompanies it.
    pub body_plain: String,
}

/// Builds the record to store from a host's authored signature, **sanitising the HTML first**.
///
/// The library is sanitised on write, not only on send, for the same reason a quoted original is
/// sanitised at seed: what comes out of it is assigned into the composer's editor with
/// `innerHTML`, and the editor page's CSP permits inline handlers. A stored body is therefore
/// inert before it is ever seeded, and `signatures.toml` is a plain file a user (or anything
/// with disk access) can edit, so this is the boundary where that stops mattering. The submit-time
/// pass (`crate::mail_compose_signature`) is the second line: it re-hardens what the editor hands
/// back. See `docs/composer-security.md`, Gate 10.
fn stored(name: String, body_html: &str, body_plain: String) -> StoredSignature {
    StoredSignature {
        name,
        body_html: crate::html::sanitize(body_html).html,
        body_plain,
    }
}

/// Maps the host-facing slot to the persisted one.
fn to_slot(kind: SignatureSlotKind) -> SignatureSlot {
    match kind {
        SignatureSlotKind::NewMessage => SignatureSlot::NewMessage,
        SignatureSlotKind::ReplyForward => SignatureSlot::ReplyForward,
    }
}

/// Mints an opaque signature id from the system CSPRNG; random, never derived from the name, so
/// renaming a signature cannot break the accounts pointing at it. Same shape as the analytics
/// install id (`crate::telemetry`): 16 bytes, URL-safe base64, no padding.
fn mint_signature_id() -> SignatureId {
    use base64::{Engine as _, engine::general_purpose::URL_SAFE_NO_PAD};
    use ring::rand::{SecureRandom, SystemRandom};

    let mut bytes = [0u8; 16];
    SystemRandom::new()
        .fill(&mut bytes)
        .expect("system CSPRNG fills 16 bytes");
    SignatureId::new(URL_SAFE_NO_PAD.encode(bytes)).expect("base64 is a valid signature id")
}

impl<P: Provider> App<P> {
    /// The signatures surface (pulled after a [`Surface::Settings`] signal): the library in the
    /// user's order, plus one row per configured account with its new-message and
    /// reply-forward assignments.
    ///
    /// Metadata only; bodies are fetched one at a time through [`App::signature_html`], so
    /// opening Settings does not drag every signature's images across the FFI.
    pub async fn signatures(&self) -> SignaturesSnapshot {
        let accounts = self.account_handles().await;
        let guard = self.signatures.lock().expect("signatures mutex poisoned");
        let rows = accounts
            .iter()
            .map(|account| {
                let (new_message, reply_forward) = guard.assignments(account.id.as_str());
                AccountSignatureRow {
                    account_id: account.id.as_str().to_owned(),
                    email: account.identity.email.clone(),
                    new_message,
                    reply_forward,
                }
            })
            .collect();
        SignaturesSnapshot {
            signatures: guard.rows(),
            accounts: rows,
        }
    }

    /// One signature's HTML body, or `None` when the id names nothing. Fetched on demand so a
    /// list or a picker ships names, not image bytes.
    #[must_use]
    pub fn signature_html(&self, id: &str) -> Option<String> {
        let id = SignatureId::new(id)?;
        self.signatures
            .lock()
            .expect("signatures mutex poisoned")
            .html(&id)
    }

    /// Creates a signature and returns the row for it (the minted id included), so a host can
    /// select what it just created without re-pulling the whole snapshot. Signals
    /// [`Surface::Settings`].
    // `async` with no inner `await` is intentional: every command method shares one async shape
    // so `dispatch` and the FFI adapter drive them uniformly.
    #[allow(clippy::unused_async)]
    pub async fn create_signature(
        &self,
        name: String,
        body_html: String,
        body_plain: String,
    ) -> SignatureRow {
        let id = mint_signature_id();
        self.signatures
            .lock()
            .expect("signatures mutex poisoned")
            .create(id.clone(), stored(name.clone(), &body_html, body_plain));
        log::info!("signatures: created one signature");
        self.observer.surface_changed(Surface::Settings);
        SignatureRow {
            id: id.as_str().to_owned(),
            name,
        }
    }

    /// Replaces a signature's name and body. Returns whether the id named one; an unknown id is
    /// a no-op rather than a silent create. Signals [`Surface::Settings`].
    #[allow(clippy::unused_async)]
    pub async fn update_signature(
        &self,
        id: &str,
        name: String,
        body_html: String,
        body_plain: String,
    ) -> bool {
        let Some(id) = SignatureId::new(id) else {
            return false;
        };
        let updated = self
            .signatures
            .lock()
            .expect("signatures mutex poisoned")
            .update(&id, stored(name, &body_html, body_plain));
        if updated {
            self.observer.surface_changed(Surface::Settings);
        }
        updated
    }

    /// Deletes a signature and clears it from every account that used it. Returns whether the id
    /// named one. Signals [`Surface::Settings`].
    #[allow(clippy::unused_async)]
    pub async fn delete_signature(&self, id: &str) -> bool {
        let Some(id) = SignatureId::new(id) else {
            return false;
        };
        let deleted = self
            .signatures
            .lock()
            .expect("signatures mutex poisoned")
            .delete(&id);
        if deleted {
            log::info!("signatures: deleted one signature");
            self.observer.surface_changed(Surface::Settings);
        }
        deleted
    }

    /// Assigns (or clears, with `None`) which signature `account` uses in one slot, then signals
    /// [`Surface::Settings`]. An id naming nothing in the library clears the slot instead of
    /// storing a pointer that resolves to nothing.
    #[allow(clippy::unused_async)]
    pub async fn set_account_signature(
        &self,
        account: &str,
        slot: SignatureSlotKind,
        signature: Option<String>,
    ) {
        let mut guard = self.signatures.lock().expect("signatures mutex poisoned");
        let signature = signature
            .and_then(SignatureId::new)
            .filter(|id| guard.library.entries.contains_key(id));
        guard.assign(account, to_slot(slot), signature);
        drop(guard);
        self.observer.surface_changed(Surface::Settings);
    }

    /// The signature a composer should open with for `account` in `slot`; its id and both
    /// bodies, or `None` when that slot is unassigned (or points at a signature that has since
    /// been deleted). This is the resolution every client runs at composer open and again when
    /// the From account changes, so the rule lives here rather than three times over.
    #[must_use]
    pub fn resolve_signature(
        &self,
        account: &str,
        slot: SignatureSlotKind,
    ) -> Option<SignatureBody> {
        let guard = self.signatures.lock().expect("signatures mutex poisoned");
        let id = guard.assigned(account, to_slot(slot))?;
        let (body_html, body_plain) = guard.bodies(&id)?;
        Some(SignatureBody {
            id: id.as_str().to_owned(),
            body_html,
            body_plain,
        })
    }

    /// One signature's id and both bodies, for the composer's per-message override picker;
    /// where the user names a signature directly rather than inheriting the account's.
    #[must_use]
    pub fn signature_body(&self, id: &str) -> Option<SignatureBody> {
        let id = SignatureId::new(id)?;
        let guard = self.signatures.lock().expect("signatures mutex poisoned");
        let (body_html, body_plain) = guard.bodies(&id)?;
        Some(SignatureBody {
            id: id.as_str().to_owned(),
            body_html,
            body_plain,
        })
    }

    /// Drops an account's signature assignment; called from account removal, so a re-add starts
    /// with no signature rather than a pointer the user thought removal had cleared.
    pub(crate) fn remove_account_signature(&self, account: &str) {
        let guard = self.signatures.lock().expect("signatures mutex poisoned");
        if let Some(path) = &guard.prefs_path {
            let mut prefs = load_preferences(path);
            if prefs.remove_account_signature(account) {
                let _ = save_preferences(path, &prefs);
            }
        }
    }
}
