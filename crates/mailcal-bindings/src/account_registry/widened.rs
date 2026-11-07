//! A grant a refresh widened to open a use the old one withheld.
//!
//! What an account dials is planned from its stored grant, so a grant widened outside this app's
//! own sign-in (an administrator's consent, or a consent still propagating when the sign-in
//! finished) opened nothing until the next launch: its calendar stayed closed and its prompt up.
//! The registry notes such an account when it records the wider grant, and asks for it to be
//! dialled again at once; the next retry dials its calendar and contacts.

use std::{
    collections::HashSet,
    sync::{Mutex, OnceLock},
};

use mailcal_account::Capabilities;

use super::AccountRegistry;

/// What dials the widened accounts again: set once the app exists.
type Reopen = Box<dyn Fn() + Send + Sync>;

/// The accounts whose grant widened since they were last dialled, and what dials them.
#[derive(Default)]
pub(crate) struct Widened {
    accounts: Mutex<HashSet<String>>,
    reopen: OnceLock<Reopen>,
}

impl std::fmt::Debug for Widened {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Widened")
            .field("accounts", &self.accounts)
            .finish_non_exhaustive()
    }
}

impl AccountRegistry {
    /// Installs what dials the widened accounts again. The first call wins.
    pub(crate) fn on_grant_widened(&self, reopen: Reopen) {
        let _ = self.widened.reopen.set(reopen);
    }

    /// Notes `id` when its grant now opens a use that `before` withheld.
    pub(super) fn note_widened(&self, id: &str, before: &Capabilities, after: &Capabilities) {
        if before.iter().any(|capability| !after.contains(capability)) {
            self.widened
                .accounts
                .lock()
                .expect("widened mutex poisoned")
                .insert(id.to_owned());
        }
    }

    /// Asks for the widened accounts to be dialled again, when there are any.
    pub(crate) fn reopen_widened(&self) {
        let any = !self
            .widened
            .accounts
            .lock()
            .expect("widened mutex poisoned")
            .is_empty();
        if let (true, Some(reopen)) = (any, self.widened.reopen.get()) {
            reopen();
        }
    }

    /// The accounts to dial the calendar and contacts of again, each once.
    pub(crate) fn take_widened(&self) -> Vec<String> {
        self.widened
            .accounts
            .lock()
            .expect("widened mutex poisoned")
            .drain()
            .collect()
    }
}
