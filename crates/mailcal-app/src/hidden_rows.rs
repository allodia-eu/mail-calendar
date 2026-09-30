//! Messages taken off the list the moment the user moves them, kept off until the store agrees.

use std::collections::{BTreeSet, HashMap, HashSet};

use engine_api::{AccountId, MailListRow, MailboxId, Provider, ProviderKey};

use crate::App;

/// Per hidden `(account, key)`: the folders the message was filed in when it was hidden, or
/// `None` when the store did not hold it then.
pub(crate) type HiddenRows = HashMap<(String, String), Option<BTreeSet<String>>>;

impl<P: Provider> App<P> {
    /// Hides `keys` until the store stops reporting them or files them somewhere else.
    ///
    /// The folders are read now, before the write goes out, because a move that keeps its key
    /// (Gmail, JMAP) only ever comes back as the same message filed elsewhere: without them the
    /// hide could not tell that the move had landed, and the message would stay hidden from the
    /// folder it went to.
    pub(crate) async fn hide_rows(&self, account: &AccountId, keys: &[ProviderKey]) {
        let mut filed: HashMap<String, BTreeSet<String>> = self
            .engine
            .messages_by_keys(account, keys)
            .await
            .unwrap_or_default()
            .into_iter()
            .map(|message| {
                let folders = message.mailboxes.iter().map(|id| id.as_str().to_owned());
                (message.id.key().as_str().to_owned(), folders.collect())
            })
            .collect();
        let mut hidden = self.hidden_rows();
        for key in keys {
            let was = filed.remove(key.as_str());
            hidden.insert((account.as_str().to_owned(), key.as_str().to_owned()), was);
        }
    }

    /// Undoes one row's hide, so a refused write puts that row back on the next rebuild.
    pub(crate) fn restore_row(&self, account: &AccountId, key: &ProviderKey) {
        self.hidden_rows()
            .remove(&(account.as_str().to_owned(), key.as_str().to_owned()));
    }

    /// Every hidden `(account, key)`, as the list filters on it.
    pub(crate) fn pending_hidden_keys(&self) -> HashSet<(String, String)> {
        self.hidden_rows().keys().cloned().collect()
    }

    /// Drops the hides `base` shows have done their job, for the accounts `read` covers, and
    /// returns what is still hidden. A hide is done once its message is gone or is filed
    /// somewhere else than when it was hidden; an account absent from `read` says nothing.
    pub(crate) fn prune_hidden_rows(
        &self,
        read: &[AccountId],
        base: &[std::sync::Arc<MailListRow>],
    ) -> HashSet<(String, String)> {
        let read: HashSet<&str> = read.iter().map(AccountId::as_str).collect();
        let now: HashMap<(&str, &str), BTreeSet<&str>> = base
            .iter()
            .map(|row| {
                let folders = row.mailboxes.iter().map(MailboxId::as_str).collect();
                ((row.account.as_str(), row.mail.key.as_str()), folders)
            })
            .collect();
        let mut hidden = self.hidden_rows();
        hidden.retain(|(account, key), was| {
            if !read.contains(account.as_str()) {
                return true;
            }
            now.get(&(account.as_str(), key.as_str()))
                .is_some_and(|folders| was.as_ref().is_none_or(|was| same(was, folders)))
        });
        hidden.keys().cloned().collect()
    }

    fn hidden_rows(&self) -> std::sync::MutexGuard<'_, HiddenRows> {
        self.pending_removals
            .lock()
            .expect("pending-removals mutex poisoned")
    }
}

fn same(was: &BTreeSet<String>, now: &BTreeSet<&str>) -> bool {
    was.len() == now.len() && was.iter().all(|folder| now.contains(folder.as_str()))
}
