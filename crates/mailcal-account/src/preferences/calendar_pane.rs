//! Which account groups are shut in the calendar pane (`docs/calendar-pane.md`).
//!
//! Stored inverted, as the folder pane's trees are: callers ask whether a group is open, and the
//! file holds the shut ones, so a group nobody has touched, a new account's included, is open.

use super::Preferences;

impl Preferences {
    /// Whether `account`'s calendars are showing in the calendar pane; `true` until shut.
    #[must_use]
    pub fn calendar_group_expanded(&self, account: &str) -> bool {
        !self.collapsed_calendar_groups.contains(account)
    }

    /// Records whether `account`'s group is open. Opening drops the entry, so the file holds only
    /// the groups the person shut.
    pub fn set_calendar_group_expanded(&mut self, account: &str, expanded: bool) {
        if expanded {
            self.collapsed_calendar_groups.remove(account);
        } else {
            self.collapsed_calendar_groups.insert(account.to_owned());
        }
    }

    /// Forgets whether `account`'s group was shut, for when the account is removed. Returns
    /// whether anything was stored for it.
    pub fn remove_calendar_group(&mut self, account: &str) -> bool {
        self.collapsed_calendar_groups.remove(account)
    }
}

#[cfg(test)]
mod tests {
    use super::Preferences;

    #[test]
    fn a_group_nobody_has_touched_is_open_and_opening_leaves_nothing_behind() {
        let mut prefs = Preferences::default();
        assert!(prefs.calendar_group_expanded("acct-1"));

        prefs.set_calendar_group_expanded("acct-1", false);
        assert!(!prefs.calendar_group_expanded("acct-1"));
        assert!(prefs.calendar_group_expanded("acct-2"));
        // The folder pane's tree for the same account is a separate fact.
        assert!(prefs.account_expanded("acct-1"));

        prefs.set_calendar_group_expanded("acct-1", true);
        assert!(prefs.collapsed_calendar_groups.is_empty());
    }

    #[test]
    fn removing_an_account_forgets_its_shut_group() {
        let mut prefs = Preferences::default();
        prefs.set_calendar_group_expanded("acct-1", false);
        assert!(prefs.remove_calendar_group("acct-1"));
        assert!(prefs.calendar_group_expanded("acct-1"));
        assert!(!prefs.remove_calendar_group("acct-1"));
    }
}
