//! The signature library (`docs/signatures.md`): what the library and each account's two slots
//! hold, read from memory, and the composer's resolution of which body a message opens with.
//!
//! Signatures are standalone entities shared across accounts, so the library lives in its own
//! file (`signatures.toml`) and only the small per-account pointer rides in `preferences.toml`;
//! see [`mailcal_account::Signatures`] for why. Both are loaded once and held here: a read never
//! touches the disk, and a change is applied here and written behind (`crate::preferences_file`).
//! Saving and deleting a signature run as work of their own ([`work`]); an
//! assignment is applied at once.
//!
//! **Privacy.** A signature body is user content. Nothing here logs it; counts, ids and lengths
//! only (`docs/logging.md`).

use std::path::PathBuf;

use engine_api::Provider;
use mailcal_account::{
    Preferences, SignatureId, SignatureSlot, Signatures, StoredSignature, load_signatures,
};
use mailcal_viewmodel::{AccountSignatureRow, SignatureRow, SignatureSlotKind, SignaturesView};

#[path = "signature_rules.rs"]
mod rules;
#[path = "signature_work.rs"]
mod work;

pub use rules::{SIGNATURE_IMAGE_LIMIT_BYTES, signature_image, signature_name};
pub use work::SignatureChange;
use work::Work;

use crate::{
    App, Surface,
    preferences_file::{PreferencesFile, SignaturesFile},
};

/// The signature library and the assignments, as the app holds them.
pub(crate) struct SignatureState {
    pub(crate) library: Signatures,
    /// The library's own file, which keeps nothing when there is no preferences file (the demo,
    /// the showcase and tests): the library then lives for this run.
    pub(crate) file: SignaturesFile,
    /// The shared preferences file, which carries the per-account assignments.
    prefs: PreferencesFile,
    /// The assignments as they stand: read from the preferences once, then kept here and written
    /// through `prefs`. Only its signature assignments are read.
    assignments: Preferences,
    /// The id the next new signature is saved under.
    pub(crate) new_id: SignatureId,
    pub(crate) work: Work,
}

impl SignatureState {
    /// Loads the library from `signatures_path` (empty when absent or unreadable) and the
    /// assignments from `prefs`. `on_failure` is told when whether the library's file could be
    /// written changes.
    pub(crate) fn new(
        signatures_path: Option<PathBuf>,
        prefs: PreferencesFile,
        on_failure: impl Fn() + Send + Sync + 'static,
    ) -> Self {
        let library = signatures_path
            .as_ref()
            .map(load_signatures)
            .unwrap_or_default();
        Self {
            library,
            file: SignaturesFile::at(signatures_path, on_failure),
            assignments: prefs.load(),
            prefs,
            new_id: mint_signature_id(),
            work: Work::default(),
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

    /// One signature's HTML **and** plain-text bodies; what the composer needs to seed both
    /// parts of the outgoing message.
    fn bodies(&self, id: &SignatureId) -> Option<(String, String)> {
        self.library
            .get(id)
            .map(|entry| (entry.body_html.clone(), entry.body_plain.clone()))
    }

    /// Stores `signature` under `id` and has the library written.
    pub(crate) fn put(&mut self, id: SignatureId, signature: StoredSignature) {
        self.library.insert(id.clone(), signature.clone());
        self.file
            .edit(move |library| library.insert(id.clone(), signature.clone()));
    }

    /// Deletes a signature from the library **and** clears it from every account slot that
    /// pointed at it, so no assignment is left naming something that no longer exists.
    pub(crate) fn delete(&mut self, id: &SignatureId) {
        if !self.library.remove(id) {
            return;
        }
        let id = id.clone();
        self.assignments.forget_signature(&id);
        self.file.edit({
            let id = id.clone();
            move |library| {
                library.remove(&id);
            }
        });
        self.prefs.edit(move |prefs| {
            prefs.forget_signature(&id);
        });
    }

    /// The signature assigned to `account`'s `slot`, if the account has one **and** it still
    /// names a signature in the library. A stale pointer reads as "no signature" rather than an
    /// error: the file is hand-editable, and the composer must open either way.
    fn assigned(&self, account: &str, slot: SignatureSlot) -> Option<SignatureId> {
        let id = self
            .assignments
            .account_signature(account)
            .slot(slot)
            .cloned()?;
        self.library.entries.contains_key(&id).then_some(id)
    }

    /// Assigns (or clears) one of an account's slots. An id naming nothing in the library clears
    /// it instead of storing a pointer that resolves to nothing.
    fn assign(&mut self, account: &str, slot: SignatureSlot, signature: Option<SignatureId>) {
        let signature = signature.filter(|id| self.library.entries.contains_key(id));
        self.assignments
            .set_account_signature(account, slot, signature.clone());
        let account = account.to_owned();
        self.prefs.edit(move |prefs| {
            prefs.set_account_signature(&account, slot, signature.clone());
        });
    }

    /// Drops an account's assignment.
    fn forget_account(&mut self, account: &str) {
        self.assignments.remove_account_signature(account);
        let account = account.to_owned();
        self.prefs.edit(move |prefs| {
            prefs.remove_account_signature(&account);
        });
    }

    /// An account's row, each slot naming a signature that is in the library or nothing.
    fn account_row(&self, account: &str, email: String) -> AccountSignatureRow {
        let slot = |slot| {
            self.assigned(account, slot)
                .map(|id| id.as_str().to_owned())
        };
        AccountSignatureRow {
            account_id: account.to_owned(),
            email,
            new_message: slot(SignatureSlot::NewMessage),
            reply_forward: slot(SignatureSlot::ReplyForward),
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
pub(crate) fn mint_signature_id() -> SignatureId {
    use base64::{Engine as _, engine::general_purpose::URL_SAFE_NO_PAD};
    use ring::rand::{SecureRandom, SystemRandom};

    let mut bytes = [0u8; 16];
    SystemRandom::new()
        .fill(&mut bytes)
        .expect("system CSPRNG fills 16 bytes");
    SignatureId::new(URL_SAFE_NO_PAD.encode(bytes)).expect("base64 is a valid signature id")
}

impl<P: Provider> App<P> {
    /// The library in the user's order, every account used for mail with its two slots, and
    /// what runs on the library. Read from memory, so a UI thread may ask it; pulled again after
    /// [`Surface::Signatures`], and after [`Surface::Settings`], which a change to the accounts
    /// raises.
    ///
    /// Metadata only; bodies are fetched one at a time through [`App::signature_html`], so
    /// opening Settings does not drag every signature's images across the FFI.
    #[must_use]
    pub fn signatures(&self) -> SignaturesView {
        let senders = self.accounts.senders();
        let guard = self.signatures.lock().expect("signatures mutex poisoned");
        SignaturesView {
            library: guard.rows(),
            accounts: senders
                .into_iter()
                .map(|(id, email)| guard.account_row(id.as_str(), email))
                .collect(),
            new_id: guard.new_id.as_str().to_owned(),
            work: guard.work.listed(),
            outcome: guard.work.outcome.clone(),
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
            .library
            .get(&id)
            .map(|entry| entry.body_html.clone())
    }

    /// Assigns (or clears, with `None`) which signature `account` uses in one slot, then signals
    /// [`Surface::Signatures`]. Applied before it returns and written behind it; an id naming
    /// nothing in the library clears the slot instead of storing a pointer that resolves to
    /// nothing.
    pub fn assign_signature(
        &self,
        account: &str,
        slot: SignatureSlotKind,
        signature: Option<String>,
    ) {
        self.signatures
            .lock()
            .expect("signatures mutex poisoned")
            .assign(account, to_slot(slot), signature.and_then(SignatureId::new));
        self.observer.surface_changed(Surface::Signatures);
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
        self.signatures
            .lock()
            .expect("signatures mutex poisoned")
            .forget_account(account);
        self.observer.surface_changed(Surface::Signatures);
    }

    /// The library's file, for the settings writers' notice, retry and closing wait.
    pub(crate) fn signature_library_file(&self) -> SignaturesFile {
        self.signatures
            .lock()
            .expect("signatures mutex poisoned")
            .file
            .clone()
    }
}
