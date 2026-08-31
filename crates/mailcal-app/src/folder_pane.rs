//! Which accounts have their folder tree open in the sidebar.
//!
//! Held in memory and consulted on **every** snapshot rebuild; including one per search
//! keystroke: so it cannot be a file read the way the per-sync settings are. The persisted
//! copy is written only when the user actually toggles a tree.
//!
//! Expansion lives here, in the core, rather than in each client, for the two reasons the
//! contract gives (`docs/folder-pane.md`): a client that kept its own would disagree with the
//! others, and one that kept it in view state would lose it on every restart, which is the
//! behaviour this replaces.

use std::path::PathBuf;

use engine_api::Provider;
use mailcal_account::{Preferences, load_preferences, save_preferences};

use crate::App;

/// The loaded per-account expansion state and where to persist it.
pub(crate) struct FolderPaneState {
    prefs: Preferences,
    prefs_path: Option<PathBuf>,
}

impl FolderPaneState {
    /// Loads the persisted state (every account expanded when the file is absent or unreadable).
    pub(crate) fn new(prefs_path: Option<PathBuf>) -> Self {
        let prefs = prefs_path
            .as_ref()
            .map_or_else(Preferences::default, load_preferences);
        Self { prefs, prefs_path }
    }

    /// Whether `account`'s folder tree is open; `true` for an account nobody has shut.
    pub(crate) fn expanded(&self, account: &str) -> bool {
        self.prefs.account_expanded(account)
    }

    /// Records whether `account`'s tree is open and persists it. Returns whether anything
    /// changed, so a client re-asserting the state it already has costs no disk write and no
    /// snapshot rebuild.
    fn set(&mut self, account: &str, expanded: bool) -> bool {
        if self.prefs.account_expanded(account) == expanded {
            return false;
        }
        self.prefs.set_account_expanded(account, expanded);
        self.persist();
        true
    }

    /// Forgets an account's expansion, and persists: so a later re-add opens showing its
    /// folders rather than inheriting a shut tree nobody remembers shutting.
    pub(crate) fn remove_account(&mut self, account: &str) {
        if self.prefs.remove_account_expansion(account) {
            self.persist();
        }
    }

    /// Writes the expansion state back, read-modify-write against what is on disk right now, so
    /// a concurrent write to an unrelated preference is not clobbered by our in-memory copy.
    fn persist(&self) {
        if let Some(path) = &self.prefs_path {
            let mut on_disk = load_preferences(path);
            on_disk
                .collapsed_accounts
                .clone_from(&self.prefs.collapsed_accounts);
            let _ = save_preferences(path, &on_disk);
        }
    }
}

impl<P: Provider> App<P> {
    /// Opens or shuts one account's folder tree, and persists the choice.
    ///
    /// Deliberately touches neither the selected account nor the selected folder: expanding is
    /// not navigating. That separation is the whole point; it is what stops the tree
    /// collapsing when the user moves to All Inboxes, the calendar, or contacts.
    // `async` with no inner `await` is intentional: every dispatched command method shares one
    // async shape so `dispatch` and the FFI adapter drive them uniformly.
    #[allow(clippy::unused_async)]
    pub async fn set_account_expanded(&self, account: &str, expanded: bool) {
        let changed = self
            .folder_pane
            .lock()
            .expect("folder-pane mutex poisoned")
            .set(account, expanded);
        if changed {
            self.rebuild_snapshot().await;
        }
    }

    /// Whether `account`'s folder tree is open; read while projecting the account rows.
    pub(crate) fn account_expanded(&self, account: &str) -> bool {
        self.folder_pane
            .lock()
            .expect("folder-pane mutex poisoned")
            .expanded(account)
    }

    /// Forgets an account's expansion state (account removal).
    pub(crate) fn remove_account_expansion(&self, account: &str) {
        self.folder_pane
            .lock()
            .expect("folder-pane mutex poisoned")
            .remove_account(account);
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use mailcal_account::{Preferences, QuoteStyle, load_preferences, save_preferences};

    use super::FolderPaneState;

    /// A fresh preferences file of its own, so one test's writes cannot reach another's.
    fn scratch_prefs(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("mailcal-folder-pane-{name}"));
        let _ = std::fs::remove_dir_all(&dir);
        let path = dir.join("preferences.toml");
        save_preferences(&path, &Preferences::default()).unwrap();
        path
    }

    #[test]
    fn an_account_nobody_shut_is_expanded() {
        // `bool::default()` is `false`, so storing the expanded accounts instead of the
        // collapsed ones would have shipped every tree shut; exactly the bug this replaces.
        assert!(FolderPaneState::new(None).expanded("acct-1"));
    }

    #[test]
    fn shutting_a_tree_survives_a_reload_and_leaves_the_others_open() {
        let path = scratch_prefs("reload");

        let mut state = FolderPaneState::new(Some(path.clone()));
        assert!(state.set("acct-1", false));
        assert!(!state.expanded("acct-1"));
        assert!(state.expanded("acct-2"));

        // A fresh launch reads the same answer back off disk.
        let reloaded = FolderPaneState::new(Some(path));
        assert!(!reloaded.expanded("acct-1"));
        assert!(reloaded.expanded("acct-2"));
    }

    #[test]
    fn re_asserting_the_current_state_changes_nothing() {
        let mut state = FolderPaneState::new(None);
        // Already expanded: no write, no snapshot rebuild.
        assert!(!state.set("acct-1", true));
        assert!(state.set("acct-1", false));
        assert!(!state.set("acct-1", false));
    }

    #[test]
    fn persisting_preserves_the_sibling_preferences_in_the_file() {
        let path = scratch_prefs("siblings");
        let mut on_disk = load_preferences(&path);
        on_disk.quote_style = QuoteStyle::LineAndHeader;
        save_preferences(&path, &on_disk).unwrap();

        let mut state = FolderPaneState::new(Some(path.clone()));
        state.set("acct-1", false);

        let after = load_preferences(&path);
        assert_eq!(after.quote_style, QuoteStyle::LineAndHeader);
        assert!(after.collapsed_accounts.contains("acct-1"));
    }

    #[test]
    fn removing_an_account_forgets_that_its_tree_was_shut() {
        let path = scratch_prefs("removal");
        let mut state = FolderPaneState::new(Some(path.clone()));
        state.set("acct-1", false);

        state.remove_account("acct-1");

        assert!(state.expanded("acct-1"));
        assert!(load_preferences(&path).collapsed_accounts.is_empty());
    }
}
