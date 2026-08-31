//! The default send-account setting: which account a **new** message composes from when the
//! unified all-inboxes view is showing and no single mailbox scopes the choice. A persisted
//! app-level preference the host shows in settings and the composer's From dropdown opens on.
//! Like the quote-style setting ([`crate::quote_settings`]) it lives in the shared preferences
//! file; this small state holds the loaded value and writes it back read-modify-write so the
//! sibling preferences are preserved. A second `impl App` block keeps `lib.rs` under the
//! 500-line limit.
//!
//! It is only ever a *fallback*: selecting one account's mailbox composes from that account, and
//! an explicit `from` on a submit intent (the composer's From dropdown) overrides both. See
//! `App::compose_account` in `crate::mail_ops`, which resolves the chain.

use std::path::PathBuf;

use engine_api::Provider;
use mailcal_account::{load_preferences, save_preferences};

use crate::{App, Surface};

/// The loaded default send account (an account id) and where to persist it.
pub(crate) struct SendSettingsState {
    account: Option<String>,
    prefs_path: Option<PathBuf>,
}

impl SendSettingsState {
    /// Loads the persisted default from the preferences file (`None` when absent or unreadable).
    pub(crate) fn new(prefs_path: Option<PathBuf>) -> Self {
        let account = prefs_path
            .as_ref()
            .and_then(|path| load_preferences(path).default_send_account);
        Self {
            account,
            prefs_path,
        }
    }

    fn get(&self) -> Option<String> {
        self.account.clone()
    }

    /// Stores `account` and persists it. Read-modify-write so the sibling display-zone /
    /// sync-depth / quote-style / swipe preferences in the same file are preserved. A blank id
    /// is normalised to `None` ("no default; derive it") rather than stored as an empty string.
    fn set(&mut self, account: Option<String>) {
        self.account = account.filter(|id| !id.trim().is_empty());
        if let Some(path) = &self.prefs_path {
            let mut prefs = load_preferences(path);
            prefs.default_send_account.clone_from(&self.account);
            let _ = save_preferences(path, &prefs);
        }
    }
}

impl<P: Provider> App<P> {
    /// The persisted default send account's id (pulled after a [`Surface::Settings`] signal), or
    /// `None` when the user has never chosen one. Note this is the **stored** choice, which may
    /// name an account that has since been removed; `App::compose_account` validates it against
    /// the configured set before using it.
    #[must_use]
    pub fn default_send_account(&self) -> Option<String> {
        self.send_settings
            .lock()
            .expect("send-settings mutex poisoned")
            .get()
    }

    /// Sets and persists the default send account (`None` clears it, restoring "the first
    /// configured account"), then signals [`Surface::Settings`] so the host re-pulls.
    // `async` with no inner `await` is intentional: every dispatched command method shares one
    // async shape so `dispatch` and the FFI adapter drive them uniformly (this one just locks,
    // sets, and signals).
    #[allow(clippy::unused_async)]
    pub async fn set_default_send_account(&self, account: Option<String>) {
        self.send_settings
            .lock()
            .expect("send-settings mutex poisoned")
            .set(account);
        self.observer.surface_changed(Surface::Settings);
    }
}

#[cfg(test)]
mod tests {
    use mailcal_account::{QuoteStyle, load_preferences, save_preferences};

    use super::SendSettingsState;

    #[test]
    fn set_persists_and_reloads_without_clobbering_sibling_preferences() {
        let dir = std::env::temp_dir().join("mailcal-default-send-account-test");
        let _ = std::fs::remove_dir_all(&dir);
        let path = dir.join("preferences.toml");
        // Seed a sibling preference the send-account write must preserve.
        let seeded = mailcal_account::Preferences {
            quote_style: QuoteStyle::LineAndHeader,
            ..Default::default()
        };
        save_preferences(&path, &seeded).unwrap();

        // No default until one is chosen, then a set persists it.
        let mut state = SendSettingsState::new(Some(path.clone()));
        assert_eq!(state.get(), None);
        state.set(Some("acct-2".to_owned()));

        // A fresh state reads back the persisted choice, and the sibling pref survived.
        assert_eq!(
            SendSettingsState::new(Some(path.clone())).get().as_deref(),
            Some("acct-2")
        );
        assert_eq!(
            load_preferences(&path).quote_style,
            QuoteStyle::LineAndHeader
        );

        // Clearing it restores "derive the account", and persists that too.
        state.set(None);
        assert_eq!(SendSettingsState::new(Some(path.clone())).get(), None);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_blank_account_id_is_stored_as_no_default() {
        // A host that hands back an empty selection means "no default", not an account whose id
        // is the empty string (which could never match a configured account).
        let mut state = SendSettingsState::new(None);
        state.set(Some("   ".to_owned()));
        assert_eq!(state.get(), None);
    }
}
