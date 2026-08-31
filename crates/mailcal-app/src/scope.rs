//! What the mailbox list is currently showing: the unified inbox, one account's whole mailbox,
//! or one folder of one account.
//!
//! One value under one lock, rather than an account and a folder held separately. Separately,
//! "no account but a folder" is a state the type permits and nothing renders: the unified
//! projection has no account for the key to belong to, so it ignores the folder and shows All
//! Inboxes. That is not hypothetical: it is what a folder click did on Windows for as long as the
//! pane has shown every account's tree, because the client could only say *which folder*
//! (`docs/folder-pane.md`, rule 14). It is also what two separate locks let a reader observe
//! mid-write, while one of the two halves had landed and the other had not.

use engine_api::AccountId;

use crate::reference::FolderRef;

/// The mailbox list's scope. An account is always named alongside a folder, so the folder key;
/// unique only within its account; can never be resolved against the wrong one, or against none.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) enum Scope {
    /// Every account, each one's Inbox in view.
    #[default]
    AllInboxes,
    /// One account's whole mailbox.
    Account(AccountId),
    /// One folder of one account.
    Folder(FolderRef),
}

impl Scope {
    /// The scope for a host's account selection: an id, or the unified list for `None`.
    pub(crate) fn for_account(account: Option<AccountId>) -> Self {
        account.map_or(Self::AllInboxes, Self::Account)
    }

    /// The account in view, or `None` on the unified list.
    pub(crate) fn account(&self) -> Option<&AccountId> {
        match self {
            Self::AllInboxes => None,
            Self::Account(account) => Some(account),
            Self::Folder(folder) => Some(&folder.account),
        }
    }

    /// The folder in view, or `None` when a whole account (or the unified list) is showing.
    pub(crate) fn folder(&self) -> Option<&str> {
        match self {
            Self::AllInboxes | Self::Account(_) => None,
            Self::Folder(folder) => Some(folder.key.as_str()),
        }
    }

    /// Whether `account` is the one in view: so removing it has to reset the list.
    pub(crate) fn names(&self, account: &AccountId) -> bool {
        self.account() == Some(account)
    }
}

#[cfg(test)]
mod tests {
    use engine_api::AccountId;

    use super::Scope;
    use crate::reference::FolderRef;

    fn account(id: &str) -> AccountId {
        AccountId::try_from(id).expect("a well-formed account id")
    }

    #[test]
    fn a_folder_always_arrives_with_its_account() {
        // The point of the type: there is no way to build a scope holding a folder key and no
        // account, which is the state the unified projection silently ignored.
        let scope = Scope::Folder(
            FolderRef::from_parts("acct-1", "archive".to_owned()).expect("a folder reference"),
        );
        assert_eq!(scope.account(), Some(&account("acct-1")));
        assert_eq!(scope.folder(), Some("archive"));
    }

    #[test]
    fn an_account_and_the_unified_list_carry_no_folder() {
        assert_eq!(Scope::default().account(), None);
        assert_eq!(Scope::default().folder(), None);
        assert_eq!(Scope::Account(account("acct-1")).folder(), None);
    }

    #[test]
    fn for_account_maps_none_onto_the_unified_list() {
        assert_eq!(Scope::for_account(None), Scope::AllInboxes);
        assert_eq!(
            Scope::for_account(Some(account("acct-1"))),
            Scope::Account(account("acct-1"))
        );
    }

    #[test]
    fn a_removed_account_is_recognized_whether_or_not_a_folder_is_open() {
        let folder = Scope::Folder(
            FolderRef::from_parts("acct-1", "sent".to_owned()).expect("a folder reference"),
        );
        assert!(folder.names(&account("acct-1")));
        assert!(!folder.names(&account("acct-2")));
        assert!(Scope::Account(account("acct-1")).names(&account("acct-1")));
        assert!(!Scope::AllInboxes.names(&account("acct-1")));
    }
}
