//! What the body-warming pass keeps between passes: which accounts have one in flight, and which
//! bodies a server has said it can never give back.

use std::{
    collections::{HashMap, HashSet},
    sync::Mutex,
};

use engine_api::{AccountId, ProviderKey};

#[derive(Default)]
pub(crate) struct Prefetching {
    /// Accounts with a pass in flight, so overlapping triggers collapse into the running drain.
    in_flight: Mutex<HashSet<String>>,
    /// Per account, the bodies a server said it can never give back this session
    /// ([`ApiError::item_unavailable`](engine_api::ApiError::item_unavailable)). Asking again only
    /// repeats the answer, so no later pass asks; a restart tries each once more.
    unfetchable: Mutex<HashMap<String, HashSet<ProviderKey>>>,
}

impl Prefetching {
    /// Marks `account` as having a pass in flight, or returns `None` if one already is. The
    /// returned guard clears the mark when dropped, on every exit path.
    pub(crate) fn begin(&self, account: &AccountId) -> Option<PrefetchGuard<'_>> {
        let mut in_flight = self.in_flight.lock().expect("prefetching mutex poisoned");
        if !in_flight.insert(account.as_str().to_owned()) {
            return None;
        }
        Some(PrefetchGuard {
            in_flight: &self.in_flight,
            account: account.as_str().to_owned(),
        })
    }

    /// The bodies of `account` a server said it can never give back this session.
    pub(crate) fn unfetchable(&self, account: &AccountId) -> HashSet<ProviderKey> {
        self.unfetchable
            .lock()
            .expect("unfetchable mutex poisoned")
            .get(account.as_str())
            .cloned()
            .unwrap_or_default()
    }

    pub(crate) fn mark_unfetchable(&self, account: &AccountId, key: ProviderKey) {
        self.unfetchable
            .lock()
            .expect("unfetchable mutex poisoned")
            .entry(account.as_str().to_owned())
            .or_default()
            .insert(key);
    }

    /// Forgets a removed account, so an id added again starts by asking for every body.
    pub(crate) fn forget(&self, account: &AccountId) {
        self.unfetchable
            .lock()
            .expect("unfetchable mutex poisoned")
            .remove(account.as_str());
    }
}

/// Clears an account's in-flight mark on drop, so an early return or error path can never leave
/// the account permanently "already warming".
pub(crate) struct PrefetchGuard<'a> {
    in_flight: &'a Mutex<HashSet<String>>,
    account: String,
}

impl Drop for PrefetchGuard<'_> {
    fn drop(&mut self) {
        self.in_flight
            .lock()
            .expect("prefetching mutex poisoned")
            .remove(&self.account);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_removed_account_leaves_no_unfetchable_body_behind() {
        let state = Prefetching::default();
        let account = AccountId::try_from("acct-1").unwrap();
        state.mark_unfetchable(&account, ProviderKey::new("m1").unwrap());
        assert_eq!(state.unfetchable(&account).len(), 1);

        state.forget(&account);

        assert!(state.unfetchable(&account).is_empty());
    }
}
