//! The mailbox list the model holds, numbered as the core publishes it.

use std::ops::Deref;

use mailcal_bindings::MailboxListSnapshot;

/// The mailbox list the core last published, and how many it has published before it, so a view
/// can tell a new list from the one it already answered for without comparing the two.
///
/// Read through `Deref` and replaced only through [`Self::replace`], so no path can swap the list
/// and leave the number behind.
pub(crate) struct HeldSnapshot {
    snapshot: MailboxListSnapshot,
    generation: u64,
}

impl HeldSnapshot {
    pub(crate) fn new(snapshot: MailboxListSnapshot) -> Self {
        Self {
            snapshot,
            generation: 0,
        }
    }

    pub(crate) fn replace(&mut self, snapshot: MailboxListSnapshot) {
        self.snapshot = snapshot;
        self.generation += 1;
    }

    pub(crate) fn generation(&self) -> u64 {
        self.generation
    }
}

impl Deref for HeldSnapshot {
    type Target = MailboxListSnapshot;

    fn deref(&self) -> &MailboxListSnapshot {
        &self.snapshot
    }
}
