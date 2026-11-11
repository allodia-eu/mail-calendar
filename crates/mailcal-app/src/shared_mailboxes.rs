//! Asking a connected account which mailboxes its sign-in opens besides its own
//! (`docs/shared-mailboxes.md`). Nothing here is kept: the answer is the server's, read when
//! Settings asks.

use engine_api::{
    AccountId, ApiError, FailureClass, Provider, SearchDomain, SharedAccount, SharedAccounts,
    SyncError,
};

use crate::App;

/// What an account's sign-in can say about the mailboxes it opens besides its own.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SharedMailboxOffer {
    /// The account's mailbox is not connected, so there is nothing to ask yet.
    NotConnected,
    /// The server lists them.
    Listed(Vec<SharedAccount>),
    /// The server lists none, but answers for an address the person types.
    ByAddress,
    /// The server has no way to open one.
    Unsupported,
}

/// What the server said of an address the person typed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SharedMailboxLookup {
    /// A mailbox the sign-in opens.
    Found(SharedAccount),
    /// Nothing the sign-in can open answers to it, or it is not an address. The server will not
    /// say which of "no such mailbox" and "not shared with you" it is.
    NotOpenable,
    /// The account's mailbox is not connected, so there is nothing to ask yet.
    NotConnected,
}

impl<P: Provider> App<P> {
    /// Whether `account` has a live mailbox to ask which mailboxes it opens: what
    /// [`App::shared_mailbox_offer`] answers [`SharedMailboxOffer::NotConnected`] without.
    pub async fn has_live_mailbox(&self, account: &AccountId) -> bool {
        self.account_handle(account)
            .await
            .is_some_and(|handle| !handle.providers.is_empty())
    }

    /// Which mailboxes `account`'s sign-in opens besides its own, asked of its live mailbox.
    ///
    /// # Errors
    ///
    /// Returns the engine's error when the server was asked and did not answer.
    pub async fn shared_mailbox_offer(
        &self,
        account: &AccountId,
    ) -> Result<SharedMailboxOffer, ApiError> {
        let Some(handle) = self.account_handle(account).await else {
            return Ok(SharedMailboxOffer::NotConnected);
        };
        let Some(provider) = handle.providers.first() else {
            return Ok(SharedMailboxOffer::NotConnected);
        };
        match provider.connection_info().capabilities.shared_accounts() {
            SharedAccounts::Unsupported => Ok(SharedMailboxOffer::Unsupported),
            SharedAccounts::ByAddress => Ok(SharedMailboxOffer::ByAddress),
            SharedAccounts::Enumerable => self
                .engine
                .list_shared_accounts(provider)
                .await
                .map(|listed| SharedMailboxOffer::Listed(mailboxes(listed))),
        }
    }

    /// The mailbox at `address`, asked of `account`'s live mailbox.
    ///
    /// # Errors
    ///
    /// Returns the engine's error when the server did not answer, or refused the sign-in.
    pub async fn resolve_shared_mailbox(
        &self,
        account: &AccountId,
        address: &str,
    ) -> Result<SharedMailboxLookup, ApiError> {
        let Some(handle) = self.account_handle(account).await else {
            return Ok(SharedMailboxLookup::NotConnected);
        };
        let Some(provider) = handle.providers.first() else {
            return Ok(SharedMailboxLookup::NotConnected);
        };
        match self.engine.resolve_shared_account(provider, address).await {
            Ok(mailbox) => Ok(SharedMailboxLookup::Found(mailbox)),
            Err(ApiError::InvalidInput(_)) => Ok(SharedMailboxLookup::NotOpenable),
            Err(ApiError::Sync(SyncError::Provider(err)))
                if err.class() == FailureClass::Permanent =>
            {
                Ok(SharedMailboxLookup::NotOpenable)
            }
            Err(err) => Err(err),
        }
    }
}

/// The stores in `listed` that hold mail. A server lists every store the sign-in opens, a
/// colleague's calendar among them, and one without mail is no mailbox.
fn mailboxes(listed: Vec<SharedAccount>) -> Vec<SharedAccount> {
    listed
        .into_iter()
        .filter(|store| store.domains.contains(SearchDomain::Mail))
        .collect()
}

#[cfg(test)]
mod tests {
    use engine_api::{SharedAccountId, StoreDomains};

    use super::*;

    fn store(handle: &str, domains: StoreDomains) -> SharedAccount {
        SharedAccount::new(SharedAccountId::try_from(handle).unwrap(), handle, domains)
    }

    #[test]
    fn a_store_without_mail_is_not_offered_as_a_mailbox() {
        let group = store(
            "group",
            StoreDomains::of(SearchDomain::Mail).with(SearchDomain::Calendar),
        );
        let colleague = store("colleague", StoreDomains::of(SearchDomain::Calendar));
        let inbox = store("inbox", StoreDomains::of(SearchDomain::Mail));
        assert_eq!(
            mailboxes(vec![group.clone(), colleague, inbox.clone()]),
            [group, inbox]
        );
    }
}
