//! The mailbox list's row cache: one load, reused by every read it answers.
//!
//! Split from `lib.rs`, which is the runtime's state and lifecycle: this is a small value type
//! two of its readers share (`live_mailbox`, `snapshot`).

use std::sync::Arc;

use engine_api::{AccountId, MailListRow};

/// One cached mailbox-list load: the accounts it spans, the window depth it was read at, and the
/// shared, individually-`Arc`-ed rows, as `App::row_cache` holds them.
pub(crate) struct CachedRows {
    pub(crate) accounts: Vec<AccountId>,
    pub(crate) window: usize,
    pub(crate) rows: Arc<Vec<Arc<MailListRow>>>,
}

impl CachedRows {
    /// Whether this load answers a read of `accounts` at `window`.
    ///
    /// The accounts must match exactly: a load of one account is not the unified list, and the
    /// unified list is not one account's; while a **deeper or equal** window is a superset the
    /// view simply truncates.
    pub(crate) fn serves(&self, accounts: &[AccountId], window: usize) -> bool {
        self.window >= window && self.accounts == accounts
    }

    /// Whether the shown list draws from `account` at all: a commit for one it does not span
    /// changes nothing on screen.
    pub(crate) fn spans(&self, account: &AccountId) -> bool {
        self.accounts.contains(account)
    }

    /// An empty load for `accounts` at `window`; what a first sync splices its rows into.
    pub(crate) fn empty(accounts: Vec<AccountId>, window: usize) -> Self {
        Self {
            accounts,
            window,
            rows: Arc::new(Vec::new()),
        }
    }
}
