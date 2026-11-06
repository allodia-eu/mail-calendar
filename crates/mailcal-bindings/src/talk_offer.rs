//! Whether a Nextcloud account's server offered Talk at its last connect (`docs/accounts.md`
//! rule 19), kept with the account so Settings can tell a server without Talk from a use the
//! person switched off.

use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};

/// Shared between the registry entry and every dial taken from it, as a JMAP session is, so the
/// dial records what the server said without holding the registry. Offered until a connect has
/// found otherwise.
#[derive(Debug, Clone, Default)]
pub(crate) struct TalkOffer(Arc<AtomicBool>);

impl TalkOffer {
    /// Records what a connect found: whether the account can make Talk meetings there.
    pub(crate) fn record(&self, offered: bool) {
        self.0.store(!offered, Ordering::Relaxed);
    }

    /// Whether the last connect found no Talk the account can use.
    pub(crate) fn lacking(&self) -> bool {
        self.0.load(Ordering::Relaxed)
    }
}
