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
    /// Per account, the bodies whose fetch failed permanently this session. Asking again only
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

    pub(crate) fn is_unfetchable(&self, account: &AccountId, key: &ProviderKey) -> bool {
        self.unfetchable
            .lock()
            .expect("unfetchable mutex poisoned")
            .get(account.as_str())
            .is_some_and(|keys| keys.contains(key))
    }

    pub(crate) fn unfetchable_count(&self, account: &AccountId) -> usize {
        self.unfetchable
            .lock()
            .expect("unfetchable mutex poisoned")
            .get(account.as_str())
            .map_or(0, HashSet::len)
    }

    pub(crate) fn mark_unfetchable(&self, account: &AccountId, key: ProviderKey) {
        self.unfetchable
            .lock()
            .expect("unfetchable mutex poisoned")
            .entry(account.as_str().to_owned())
            .or_default()
            .insert(key);
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
