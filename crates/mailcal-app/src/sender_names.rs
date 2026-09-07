//! The name each account's outgoing mail is sent under.
//!
//! Held in memory because it is read on **every** snapshot rebuild: it labels the account
//! rows, so it cannot be a file read the way the per-sync settings are. The persisted copy
//! is written only when the user actually sets a name.
//!
//! The name lives here rather than in the engine even though three providers keep one
//! server-side, and the reason is IMAP: an IMAP account has no server-side notion at all,
//! so a local value is needed regardless, and two owners of one header is the state to
//! avoid. What the engine holds is a *seed* to fill the field in with, and, where the
//! provider allows it, a copy kept in step for the account's other clients
//! (`docs/sending.md`).

use std::path::PathBuf;

use engine_api::{EmailAddress, Provider};
use mailcal_account::{Preferences, load_preferences, save_preferences};

use crate::{App, Surface};

/// The loaded per-account sender names and where to persist them.
pub(crate) struct SenderNameState {
    prefs: Preferences,
    prefs_path: Option<PathBuf>,
}

impl SenderNameState {
    /// Loads the persisted names (none, when the file is absent or unreadable).
    pub(crate) fn new(prefs_path: Option<PathBuf>) -> Self {
        let prefs = prefs_path
            .as_ref()
            .map_or_else(Preferences::default, load_preferences);
        Self { prefs, prefs_path }
    }

    /// The name `account` sends under, or `None` when nobody has set one.
    pub(crate) fn name(&self, account: &str) -> Option<String> {
        self.prefs.sender_name_of(account).map(str::to_owned)
    }

    /// Records the name and persists it. Returns whether anything changed, so a client
    /// re-asserting the value it already shows costs no disk write and no snapshot rebuild.
    fn set(&mut self, account: &str, name: &str) -> bool {
        let changed = self.prefs.set_account_sender_name(account, name);
        if changed {
            self.persist();
        }
        changed
    }

    /// Forgets an account's name, on removal, so a re-added id does not inherit a name the
    /// user set for a different mailbox.
    pub(crate) fn remove_account(&mut self, account: &str) {
        if self.prefs.remove_account_sender_name(account) {
            self.persist();
        }
    }

    /// Writes the names back, read-modify-write against what is on disk right now, so a
    /// concurrent write to an unrelated preference is not clobbered by our in-memory copy.
    fn persist(&self) {
        if let Some(path) = &self.prefs_path {
            let mut on_disk = load_preferences(path);
            on_disk
                .account_sender_names
                .clone_from(&self.prefs.account_sender_names);
            let _ = save_preferences(path, &on_disk);
        }
    }
}

impl<P: Provider> App<P> {
    /// The name `account` sends under, for the account rows and the settings card.
    pub(crate) fn sender_name(&self, account: &str) -> Option<String> {
        self.sender_names
            .lock()
            .expect("sender-name mutex poisoned")
            .name(account)
    }

    /// The **`From` this account sends as**: its address, carrying the name the user set.
    ///
    /// The one resolver, so no send path can put a bare address on the wire while the
    /// account rows show a name. Distinct from
    /// [`account_identity`](Self::account_identity), which answers the *address* question
    /// (is this message mine, which `ATTENDEE` line is me) and must stay a bare address:
    /// giving a name to an address that is about to be compared would not change the
    /// comparison, but it would invite one that did.
    pub(crate) async fn sender_identity(&self, account: &engine_api::AccountId) -> Option<EmailAddress> {
        let address = self.account_identity(account).await?;
        Some(match self.sender_name(account.as_str()) {
            Some(name) => EmailAddress::named(name, address.email),
            None => address,
        })
    }

    /// Forgets an account's sender name (account removal).
    pub(crate) fn remove_account_sender_name(&self, account: &str) {
        self.sender_names
            .lock()
            .expect("sender-name mutex poisoned")
            .remove_account(account);
    }

    /// Sets the name `account` sends under, and pushes it to the provider when the provider
    /// is one whose copy the account holder owns.
    ///
    /// The local value is the one that reaches the wire, so it is stored **first** and the
    /// provider write is best-effort: a server that refuses, or that cannot be reached,
    /// leaves the user with the name they asked for rather than an error about somebody
    /// else's copy. The name is sanitised on store (`mailcal_account::sanitize_sender_name`),
    /// so what is pushed can never be a header the user did not intend.
    pub async fn set_account_sender_name(&self, account: &str, name: &str) {
        let changed = self
            .sender_names
            .lock()
            .expect("sender-name mutex poisoned")
            .set(account, name);
        if !changed {
            return;
        }
        self.push_sender_name(account).await;
        self.rebuild_snapshot().await;
        self.observer.surface_changed(Surface::Settings);
    }
}
