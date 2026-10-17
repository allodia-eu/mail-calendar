//! Which of the rows in view belong to the list being shown: the one predicate the unified inbox
//! and a single folder share (see [`super`]).

use std::collections::HashMap;

use engine_api::{AccountId, MailListRow};

/// Which of the rows in view actually belong to the list being shown.
///
/// The rows themselves come from the accounts in view; this decides which of them the flat list
/// shows and which conversations the threaded list lists. Out-of-view rows are still projected;
/// they are what completes a conversation with the owner's Sent replies.
pub(crate) enum InView {
    /// Every message of the accounts in view counts: an account's own all-mail.
    Everything,
    /// Only messages filed in the named mailbox **of their own account**: the selected folder for
    /// one account, or each account's Inbox for the unified view.
    Mailbox(HashMap<String, String>),
}

impl InView {
    /// The unified view: each account's own INBOX is what counts as in view.
    pub(crate) fn inboxes(by_account: HashMap<String, String>) -> Self {
        Self::Mailbox(by_account)
    }

    /// One account's view: the selected folder, or everything when its all-mail is showing.
    pub(crate) fn folder(account: &AccountId, folder: Option<&str>) -> Self {
        match folder {
            None => Self::Everything,
            Some(key) => Self::Mailbox(HashMap::from([(
                account.as_str().to_owned(),
                key.to_owned(),
            )])),
        }
    }

    pub(crate) fn holds(&self, row: &MailListRow) -> bool {
        match self {
            Self::Everything => true,
            Self::Mailbox(by_account) => by_account
                .get(row.account.as_str())
                .is_some_and(|wanted| row.mailboxes.iter().any(|id| id.as_str() == wanted)),
        }
    }
}
