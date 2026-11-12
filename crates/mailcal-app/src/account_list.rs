//! The live account list, and what a UI thread may read of it without waiting on its lock.
//!
//! The list itself sits behind an await-safe `RwLock`, which a reader holding it across a network
//! round trip keeps from a writer, and a writer queued behind that reader keeps from every later
//! reader. A screen drawn on a UI thread cannot wait on that (`docs/waiting.md` rule 4), so every
//! write leaves a copy of what such a screen reads, the order and which accounts have a live
//! mailbox, behind a lock nothing holds across an `.await`.

use std::{
    ops::{Deref, DerefMut},
    sync::{Arc, Mutex},
};

use engine_api::AccountId;
use tokio::sync::{RwLock, RwLockReadGuard, RwLockWriteGuard};

use super::Account;

/// One account as the copy keeps it.
#[derive(Debug, Clone)]
struct Listed {
    id: AccountId,
    /// Whether its mailbox is connected: it has a mail provider to ask.
    mailbox: bool,
}

/// The accounts the app drives, in the order the host stored them.
pub(crate) struct AccountList<P> {
    live: RwLock<Vec<Arc<Account<P>>>>,
    listed: Mutex<Vec<Listed>>,
}

impl<P> AccountList<P> {
    pub(crate) fn new(accounts: Vec<Arc<Account<P>>>) -> Self {
        let listed = Mutex::new(listed(&accounts));
        Self {
            live: RwLock::new(accounts),
            listed,
        }
    }

    pub(crate) async fn read(&self) -> RwLockReadGuard<'_, Vec<Arc<Account<P>>>> {
        self.live.read().await
    }

    pub(crate) fn try_read(
        &self,
    ) -> Result<RwLockReadGuard<'_, Vec<Arc<Account<P>>>>, tokio::sync::TryLockError> {
        self.live.try_read()
    }

    /// The list to change; the copy follows it when the guard is dropped.
    pub(crate) async fn write(&self) -> ListWriteGuard<'_, P> {
        ListWriteGuard {
            guard: self.live.write().await,
            listed: &self.listed,
        }
    }

    /// Every account's id, in order, as the last change to the list left it. Never waits on the
    /// list's lock.
    pub(crate) fn ids(&self) -> Vec<AccountId> {
        self.copy().iter().map(|listed| listed.id.clone()).collect()
    }

    /// Whether `id` has a connected mailbox, as the last change to the list left it. Never waits
    /// on the list's lock.
    pub(crate) fn has_mailbox(&self, id: &AccountId) -> bool {
        self.copy()
            .iter()
            .any(|listed| listed.id == *id && listed.mailbox)
    }

    fn copy(&self) -> std::sync::MutexGuard<'_, Vec<Listed>> {
        self.listed.lock().expect("account list copy poisoned")
    }
}

fn listed<P>(accounts: &[Arc<Account<P>>]) -> Vec<Listed> {
    accounts
        .iter()
        .map(|account| Listed {
            id: account.id.clone(),
            mailbox: !account.providers.is_empty(),
        })
        .collect()
}

/// The list held for a change: written through as the `Vec` it guards, and copied for the
/// readers that may not wait when it is let go.
pub(crate) struct ListWriteGuard<'a, P> {
    guard: RwLockWriteGuard<'a, Vec<Arc<Account<P>>>>,
    listed: &'a Mutex<Vec<Listed>>,
}

impl<P> Deref for ListWriteGuard<'_, P> {
    type Target = Vec<Arc<Account<P>>>;

    fn deref(&self) -> &Self::Target {
        &self.guard
    }
}

impl<P> DerefMut for ListWriteGuard<'_, P> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.guard
    }
}

impl<P> Drop for ListWriteGuard<'_, P> {
    fn drop(&mut self) {
        // Copied while the list is still held, so two changes cannot leave their copies in the
        // other order.
        let copy = listed(&self.guard);
        *self
            .listed
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner) = copy;
    }
}
