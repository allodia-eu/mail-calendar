//! Which accounts have their folder tree shut in the sidebar, and the accessors that read
//! and record it.
//!
//! Its own module because the storage is inverted relative to the question everyone asks.
//! Callers ask "is this account expanded?"; the file stores the accounts that are
//! **collapsed**. Keeping the inversion in one place is what stops a caller reading the
//! `BTreeSet` directly and getting the answer backwards.
//!
//! The pane behaviour this drives is `docs/folder-pane.md`.

use super::Preferences;

impl Preferences {
    /// Whether `account`'s folder tree is open in the sidebar.
    ///
    /// **Expanded is the default**, which is why the persisted set holds the collapsed
    /// accounts rather than the expanded ones: an account nobody has touched; including
    /// one just added, and every account on the launch after this shipped; opens showing
    /// its folders, the way a mail client that has just been given a mailbox should.
    #[must_use]
    pub fn account_expanded(&self, account: &str) -> bool {
        !self.collapsed_accounts.contains(account)
    }

    /// Records whether `account`'s folder tree is open. Expanding drops the entry rather
    /// than writing one, so the file holds only the accounts the user actually shut.
    pub fn set_account_expanded(&mut self, account: &str, expanded: bool) {
        if expanded {
            self.collapsed_accounts.remove(account);
        } else {
            self.collapsed_accounts.insert(account.to_owned());
        }
    }

    /// Drops the collapse state for an account; used when the account is removed, so a
    /// later re-add opens expanded rather than inheriting a shut tree the user has no
    /// memory of shutting. Returns whether anything was stored for it.
    pub fn remove_account_expansion(&mut self, account: &str) -> bool {
        self.collapsed_accounts.remove(account)
    }
}

#[cfg(test)]
mod tests {
    use super::Preferences;

    #[test]
    fn an_account_nobody_has_touched_is_expanded() {
        let prefs = Preferences::default();
        assert!(prefs.account_expanded("acct-1"));
    }

    #[test]
    fn collapsing_persists_and_expanding_leaves_nothing_behind() {
        let mut prefs = Preferences::default();
        prefs.set_account_expanded("acct-1", false);
        assert!(!prefs.account_expanded("acct-1"));
        assert!(prefs.collapsed_accounts.contains("acct-1"));

        prefs.set_account_expanded("acct-1", true);
        assert!(prefs.account_expanded("acct-1"));
        // Back to the default, so nothing is written out for it.
        assert!(prefs.collapsed_accounts.is_empty());
    }

    #[test]
    fn one_account_collapsing_leaves_the_others_open() {
        let mut prefs = Preferences::default();
        prefs.set_account_expanded("acct-2", false);
        assert!(prefs.account_expanded("acct-1"));
        assert!(!prefs.account_expanded("acct-2"));
        assert!(prefs.account_expanded("acct-3"));
    }

    #[test]
    fn removing_an_account_forgets_that_it_was_collapsed() {
        let mut prefs = Preferences::default();
        prefs.set_account_expanded("acct-1", false);
        assert!(prefs.remove_account_expansion("acct-1"));
        // A re-add opens expanded rather than inheriting the old shut tree.
        assert!(prefs.account_expanded("acct-1"));
        assert!(!prefs.remove_account_expansion("acct-1"));
    }
}
