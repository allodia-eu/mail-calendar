//! Which trees are shut in the sidebar, and the accessors that read and record it: one per
//! account, one per folder that holds folders, and the **All Accounts** group's.
//!
//! Its own module because the storage is inverted relative to the question everyone asks.
//! Callers ask "is this expanded?"; the file stores what is **collapsed**. Keeping the
//! inversion in one place is what stops a caller reading the `BTreeSet` directly and getting
//! the answer backwards.
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

    /// Whether the **All Accounts** group is open in the sidebar. Open by default, the same
    /// way an account nobody has shut is, and for the same reason: the group holds the only
    /// row that opens the unified list, so a shut default would hide it.
    #[must_use]
    pub const fn unified_expanded(&self) -> bool {
        !self.unified_collapsed
    }

    /// Records whether the **All Accounts** group is open.
    pub const fn set_unified_expanded(&mut self, expanded: bool) {
        self.unified_collapsed = !expanded;
    }

    /// Whether `folder`'s own sub-folders are showing, in `account`'s tree.
    ///
    /// **Expanded is the default**, as it is for an account. A folder nobody has touched shows
    /// what is inside it, so nothing goes missing on the launch this ships in: a mailbox whose
    /// folders were drawn as one flat list becomes a tree with every row still on screen, and
    /// tidying it up is the user's to do rather than ours to do for them.
    #[must_use]
    pub fn folder_expanded(&self, account: &str, folder: &str) -> bool {
        !self
            .collapsed_folders
            .get(account)
            .is_some_and(|shut| shut.contains(folder))
    }

    /// Records whether `folder`'s sub-folders are showing. Expanding drops the entry, and the
    /// account's last entry takes the account's own map row with it, so the file holds only
    /// what the user actually shut rather than growing a row per folder they ever opened.
    pub fn set_folder_expanded(&mut self, account: &str, folder: &str, expanded: bool) {
        if expanded {
            let Some(shut) = self.collapsed_folders.get_mut(account) else {
                return;
            };
            shut.remove(folder);
            if shut.is_empty() {
                self.collapsed_folders.remove(account);
            }
        } else {
            self.collapsed_folders
                .entry(account.to_owned())
                .or_default()
                .insert(folder.to_owned());
        }
    }

    /// Drops every folder's collapse state for an account; used when the account is removed,
    /// for the reason [`Preferences::remove_account_expansion`] exists. Returns whether
    /// anything was stored for it.
    pub fn remove_folder_expansions(&mut self, account: &str) -> bool {
        self.collapsed_folders.remove(account).is_some()
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
    fn the_all_accounts_group_nobody_has_touched_is_expanded() {
        let mut prefs = Preferences::default();
        assert!(prefs.unified_expanded());

        prefs.set_unified_expanded(false);
        assert!(!prefs.unified_expanded());
        assert!(prefs.unified_collapsed);

        prefs.set_unified_expanded(true);
        assert!(prefs.unified_expanded());
    }

    #[test]
    fn the_group_and_the_accounts_collapse_independently() {
        let mut prefs = Preferences::default();
        prefs.set_unified_expanded(false);
        assert!(prefs.account_expanded("acct-1"));

        prefs.set_account_expanded("acct-1", false);
        prefs.set_unified_expanded(true);
        assert!(!prefs.account_expanded("acct-1"));
    }

    #[test]
    fn a_folder_nobody_has_touched_shows_what_is_inside_it() {
        let mut prefs = Preferences::default();
        assert!(prefs.folder_expanded("acct-1", "archive"));

        prefs.set_folder_expanded("acct-1", "archive", false);
        assert!(!prefs.folder_expanded("acct-1", "archive"));
        // One folder shutting leaves its siblings, and the same key in another account, open.
        assert!(prefs.folder_expanded("acct-1", "clients"));
        assert!(prefs.folder_expanded("acct-2", "archive"));
    }

    #[test]
    fn expanding_a_folder_leaves_nothing_behind_in_the_file() {
        let mut prefs = Preferences::default();
        prefs.set_folder_expanded("acct-1", "archive", false);
        prefs.set_folder_expanded("acct-1", "clients", false);

        prefs.set_folder_expanded("acct-1", "archive", true);
        assert_eq!(prefs.collapsed_folders["acct-1"].len(), 1);

        // The account's last shut folder takes the account's own row with it, so opening
        // everything back up leaves the file exactly as it started.
        prefs.set_folder_expanded("acct-1", "clients", true);
        assert!(prefs.collapsed_folders.is_empty());
        // And expanding something never shut writes nothing at all.
        prefs.set_folder_expanded("acct-3", "archive", true);
        assert!(prefs.collapsed_folders.is_empty());
    }

    #[test]
    fn removing_an_account_forgets_its_folders_too() {
        let mut prefs = Preferences::default();
        prefs.set_folder_expanded("acct-1", "archive", false);
        prefs.set_folder_expanded("acct-2", "archive", false);

        assert!(prefs.remove_folder_expansions("acct-1"));
        assert!(prefs.folder_expanded("acct-1", "archive"));
        // The account beside it keeps what it had.
        assert!(!prefs.folder_expanded("acct-2", "archive"));
        assert!(!prefs.remove_folder_expansions("acct-1"));
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
