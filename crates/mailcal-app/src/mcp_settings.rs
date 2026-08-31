//! The local MCP (AI assistant access) settings: on/off, which accounts are exposed, and the
//! two send controls.
//!
//! Same read-modify-write shape as the sibling settings states (`quote_settings`,
//! `swipe_settings`): the loaded values live here, every write re-reads the whole preferences
//! file, edits, and writes it back so the other settings in it are preserved.
//!
//! # Two defaults that are the feature's safety model
//!
//! Off, and **empty**. Turning the server on grants nothing until the user ticks an account, and
//! sending is a third, separate decision behind its own toggle. Those are not conservative
//! defaults chosen out of caution; they are the design: an MCP client is a program with full
//! read and act access to a mailbox, so what it can reach has to be something the user picked
//! item by item rather than something a single switch conferred.

use std::{collections::BTreeSet, path::PathBuf};

use engine_api::Provider;
use mailcal_account::{Preferences, load_preferences, save_preferences};
use mailcal_viewmodel::{McpAccountRow, McpSettings};

use crate::{App, Surface};

/// The loaded MCP settings and where to persist them.
pub(crate) struct McpSettingsState {
    enabled: bool,
    accounts: BTreeSet<String>,
    allow_direct_send: bool,
    require_known_recipient: bool,
    prefs_path: Option<PathBuf>,
}

impl McpSettingsState {
    /// Loads the persisted settings (off, nothing exposed, direct send off, guard on, when the
    /// file is absent or unreadable).
    pub(crate) fn new(prefs_path: Option<PathBuf>) -> Self {
        let prefs = prefs_path
            .as_ref()
            .map(load_preferences)
            .unwrap_or_default();
        Self {
            enabled: prefs.mcp_enabled,
            accounts: prefs.mcp_accounts,
            allow_direct_send: prefs.mcp_allow_direct_send,
            require_known_recipient: prefs.mcp_require_known_recipient,
            prefs_path,
        }
    }

    fn set_enabled(&mut self, enabled: bool) {
        self.enabled = enabled;
        self.persist(|prefs| prefs.mcp_enabled = enabled);
    }

    fn set_account_exposed(&mut self, account: &str, exposed: bool) {
        if exposed {
            self.accounts.insert(account.to_owned());
        } else {
            self.accounts.remove(account);
        }
        let accounts = self.accounts.clone();
        self.persist(|prefs| prefs.mcp_accounts = accounts);
    }

    fn set_allow_direct_send(&mut self, allow: bool) {
        self.allow_direct_send = allow;
        self.persist(|prefs| prefs.mcp_allow_direct_send = allow);
    }

    fn set_require_known_recipient(&mut self, require: bool) {
        self.require_known_recipient = require;
        self.persist(|prefs| prefs.mcp_require_known_recipient = require);
    }

    /// Drops an account from the exposure list; called when the account is removed, so a later
    /// re-add under the same id does not silently inherit an exposure the user granted to a
    /// mailbox that no longer exists.
    fn forget_account(&mut self, account: &str) {
        if self.accounts.remove(account) {
            let accounts = self.accounts.clone();
            self.persist(|prefs| prefs.mcp_accounts = accounts);
        }
    }

    fn persist(&self, edit: impl FnOnce(&mut Preferences)) {
        if let Some(path) = &self.prefs_path {
            let mut prefs = load_preferences(path);
            edit(&mut prefs);
            let _ = save_preferences(path, &prefs);
        }
    }
}

impl<P: Provider> App<P> {
    /// The MCP settings a host renders (pulled after a [`Surface::Settings`] signal).
    ///
    /// `running` and `endpoint` are the host's to fill in: the core owns the *decisions*, the
    /// binding layer owns whether a socket is currently bound and where. Keeping them in one
    /// snapshot means the panel shows the user's choice and its actual effect together, which is
    /// the difference between "MCP is on" and "MCP is on and a client can reach it".
    pub async fn mcp_settings(&self) -> McpSettings {
        let (enabled, exposed, allow_direct_send, require_known_recipient) = {
            let state = self
                .mcp_settings
                .lock()
                .expect("mcp-settings mutex poisoned");
            (
                state.enabled,
                state.accounts.clone(),
                state.allow_direct_send,
                state.require_known_recipient,
            )
        };
        McpSettings {
            enabled,
            running: false,
            accounts: self
                .account_rows()
                .await
                .into_iter()
                .map(|row| McpAccountRow {
                    exposed: exposed.contains(&row.id),
                    account_id: row.id,
                    email: row.email,
                })
                .collect(),
            allow_direct_send,
            require_known_recipient,
            endpoint: None,
        }
    }

    /// The account ids exposed to assistants; what the binding layer hands the server as its
    /// allow list, without going through the whole snapshot.
    #[must_use]
    pub fn mcp_exposed_accounts(&self) -> BTreeSet<String> {
        self.mcp_settings
            .lock()
            .expect("mcp-settings mutex poisoned")
            .accounts
            .clone()
    }

    /// Whether the user has turned the local MCP server on.
    #[must_use]
    pub fn mcp_enabled(&self) -> bool {
        self.mcp_settings
            .lock()
            .expect("mcp-settings mutex poisoned")
            .enabled
    }

    /// Whether an assistant may send mail directly.
    #[must_use]
    pub fn mcp_allow_direct_send(&self) -> bool {
        self.mcp_settings
            .lock()
            .expect("mcp-settings mutex poisoned")
            .allow_direct_send
    }

    /// Whether a direct send is restricted to known recipients.
    #[must_use]
    pub fn mcp_require_known_recipient(&self) -> bool {
        self.mcp_settings
            .lock()
            .expect("mcp-settings mutex poisoned")
            .require_known_recipient
    }

    /// Turns the local MCP server on or off, persists it, and signals [`Surface::Settings`].
    ///
    /// Turning it **off never clears the account list**: a user who switches the feature off for
    /// an afternoon should not have to re-tick every mailbox afterwards, and the list is inert
    /// while the server is not running.
    // `async` with no inner `await`: every dispatched settings method shares one shape so the
    // FFI adapter drives them uniformly.
    #[allow(clippy::unused_async)]
    pub async fn set_mcp_enabled(&self, enabled: bool) {
        self.mcp_settings
            .lock()
            .expect("mcp-settings mutex poisoned")
            .set_enabled(enabled);
        log::info!("mcp: assistant access turned {}", on_off(enabled));
        self.observer.surface_changed(Surface::Settings);
    }

    /// Exposes or hides one account, persists it, and signals [`Surface::Settings`].
    #[allow(clippy::unused_async)]
    pub async fn set_mcp_account_exposed(&self, account: &str, exposed: bool) {
        self.mcp_settings
            .lock()
            .expect("mcp-settings mutex poisoned")
            .set_account_exposed(account, exposed);
        // A count, never an id or an address: this line goes in a support log.
        log::info!(
            "mcp: {} account(s) exposed to assistants",
            self.mcp_exposed_accounts().len(),
        );
        self.observer.surface_changed(Surface::Settings);
    }

    /// Sets whether an assistant may send mail directly, persists it, and signals
    /// [`Surface::Settings`].
    #[allow(clippy::unused_async)]
    pub async fn set_mcp_allow_direct_send(&self, allow: bool) {
        self.mcp_settings
            .lock()
            .expect("mcp-settings mutex poisoned")
            .set_allow_direct_send(allow);
        log::info!("mcp: direct sending turned {}", on_off(allow));
        self.observer.surface_changed(Surface::Settings);
    }

    /// Sets whether a direct send is restricted to known recipients, persists it, and signals
    /// [`Surface::Settings`].
    #[allow(clippy::unused_async)]
    pub async fn set_mcp_require_known_recipient(&self, require: bool) {
        self.mcp_settings
            .lock()
            .expect("mcp-settings mutex poisoned")
            .set_require_known_recipient(require);
        log::info!("mcp: known-recipient guard turned {}", on_off(require));
        self.observer.surface_changed(Surface::Settings);
    }

    /// Drops a removed account from the exposure list. Called from the account-removal path.
    pub(crate) fn forget_mcp_account(&self, account: &str) {
        self.mcp_settings
            .lock()
            .expect("mcp-settings mutex poisoned")
            .forget_account(account);
    }
}

/// "on" / "off", for a log line that says what happened without saying whose mailbox it was.
const fn on_off(value: bool) -> &'static str {
    if value { "on" } else { "off" }
}

#[cfg(test)]
mod tests {
    use mailcal_account::{MessageGrouping, load_preferences, save_preferences};

    use super::McpSettingsState;

    /// A preferences file with a sibling setting the MCP writes must preserve.
    fn seeded(name: &str) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(name);
        let _ = std::fs::remove_dir_all(&dir);
        let path = dir.join("preferences.toml");
        let seeded = mailcal_account::Preferences {
            display_timezone: Some("Europe/Amsterdam".to_owned()),
            message_grouping: MessageGrouping::Flat,
            ..Default::default()
        };
        save_preferences(&path, &seeded).unwrap();
        path
    }

    #[test]
    fn the_defaults_are_off_and_empty_with_the_recipient_guard_on() {
        // The feature's whole safety model, asserted rather than assumed. If any of these three
        // ever flips, turning MCP on would silently expose mail or silently allow a send.
        let state = McpSettingsState::new(Some(seeded("mailcal-mcp-defaults-test")));
        assert!(!state.enabled, "the server is off");
        assert!(state.accounts.is_empty(), "no account is exposed");
        assert!(!state.allow_direct_send, "direct sending is off");
        assert!(
            state.require_known_recipient,
            "and if direct sending is ever turned on, the guard is already on",
        );
    }

    #[test]
    fn exposure_persists_and_leaves_the_sibling_preferences_alone() {
        let path = seeded("mailcal-mcp-exposure-test");
        let mut state = McpSettingsState::new(Some(path.clone()));
        state.set_enabled(true);
        state.set_account_exposed("work", true);
        state.set_account_exposed("private", true);
        state.set_account_exposed("private", false);

        let reloaded = McpSettingsState::new(Some(path.clone()));
        assert!(reloaded.enabled);
        assert_eq!(
            reloaded
                .accounts
                .iter()
                .map(String::as_str)
                .collect::<Vec<_>>(),
            ["work"],
        );
        let on_disk = load_preferences(&path);
        assert_eq!(
            on_disk.display_timezone.as_deref(),
            Some("Europe/Amsterdam")
        );
        assert_eq!(on_disk.message_grouping, MessageGrouping::Flat);
    }

    #[test]
    fn turning_the_server_off_keeps_the_account_list() {
        // A user who switches the feature off for an afternoon should not have to re-tick every
        // mailbox afterwards; the list is inert while nothing is listening.
        let path = seeded("mailcal-mcp-off-test");
        let mut state = McpSettingsState::new(Some(path.clone()));
        state.set_enabled(true);
        state.set_account_exposed("work", true);
        state.set_enabled(false);

        let reloaded = McpSettingsState::new(Some(path));
        assert!(!reloaded.enabled);
        assert!(reloaded.accounts.contains("work"));
    }

    #[test]
    fn removing_an_account_drops_its_exposure() {
        // Otherwise a later account re-added under the same id would inherit an exposure the
        // user granted to a different mailbox.
        let path = seeded("mailcal-mcp-forget-test");
        let mut state = McpSettingsState::new(Some(path.clone()));
        state.set_account_exposed("work", true);
        state.forget_account("work");

        assert!(McpSettingsState::new(Some(path)).accounts.is_empty());
    }
}
