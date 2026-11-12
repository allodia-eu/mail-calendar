//! The stores an account is used for besides its own, each listed as an account of its own with
//! everything it holds that the account opens: a group's mailbox and calendar are one account
//! (`docs/shared-mailboxes.md`).
//!
//! A shared store has no registry entry: it is stored on the account whose sign-in opens it, and
//! everything that dials it is read from that account's entry at the moment it is asked. So a
//! replaced password, a rotated grant or a new connection pool reaches the shared store with
//! nothing to keep in step.

use engine_api::{AccountId, EmailAddress, SharedAccountId};
use mailcal_account::{Capabilities, Capability, SharedStoreChoice, SharedUse, SharedUses};

use super::{AccountDial, AccountRegistry};
use crate::connected_account::ConnectedAccount;

/// What a dial of a shared store binds beside its parent account's credential.
#[derive(Debug, Clone)]
pub(crate) struct SharedDial {
    /// The server's handle for the store.
    pub(crate) handle: SharedAccountId,
    /// The address it is known by: its own, or its parent's when the server gives it none. A
    /// shared mailbox sends as it.
    pub(crate) identity: EmailAddress,
}

/// The account's use that opens `kind`.
const fn capability(kind: SharedUse) -> Capability {
    match kind {
        SharedUse::Mail => Capability::Mail,
        SharedUse::Calendar => Capability::Calendar,
    }
}

impl AccountDial {
    /// What the stores this account's sign-in opens besides its own are opened for: mail on an
    /// IMAP or JMAP account, whose server says, or on a Microsoft account used for shared
    /// mailboxes with a grant that allows them; calendars on a JMAP account, or an IMAP account
    /// with a CalDAV server. Never a use the account is not used for: a store is shared with a
    /// mailbox or a calendar, and an account without one would wait forever for it to connect.
    pub(crate) fn shared_uses(&self) -> SharedUses {
        let uses = self.capabilities();
        SharedUse::ALL
            .into_iter()
            .filter(|kind| uses.contains(capability(*kind)))
            .filter(|kind| match (self, kind) {
                (Self::Imap { .. } | Self::Jmap { .. }, SharedUse::Mail) => true,
                (Self::Microsoft { .. }, SharedUse::Mail) => {
                    uses.contains(Capability::SharedMailboxes)
                }
                (Self::Imap { config, .. }, SharedUse::Calendar) => config.caldav.is_some(),
                (Self::Jmap { .. }, SharedUse::Calendar) => true,
                (Self::Microsoft { .. } | Self::Google { .. }, _) => false,
            })
            .collect()
    }

    /// This dial, opening the store `choice` with the account's sign-in instead of the account
    /// itself: what it holds that the account opens, under the store's own address. `None` when
    /// that is nothing.
    fn for_shared(self, choice: &SharedStoreChoice) -> Option<Self> {
        let opened = choice.holds.intersect(self.shared_uses());
        if opened.is_empty() {
            return None;
        }
        let uses: Capabilities = opened.iter().map(capability).collect();
        let shared = |parent: EmailAddress| SharedDial {
            handle: choice.handle.clone(),
            identity: choice.address.clone().map_or(parent, EmailAddress::new),
        };
        match self {
            Self::Imap {
                config,
                connections,
                tokens,
                ..
            } => {
                let parent = EmailAddress::new(config.username().to_owned());
                Some(Self::Imap {
                    config,
                    connections,
                    tokens,
                    capabilities: uses,
                    talk: crate::talk_offer::TalkOffer::default(),
                    shared: Some(shared(parent)),
                })
            }
            Self::Jmap {
                config,
                tokens,
                session,
                ..
            } => {
                let parent = config.identity();
                Some(Self::Jmap {
                    config,
                    tokens,
                    capabilities: uses,
                    session,
                    shared: Some(shared(parent)),
                })
            }
            Self::Microsoft {
                tokens, identity, ..
            } => Some(Self::Microsoft {
                tokens,
                shared: Some(shared(identity.clone())),
                identity: shared(identity).identity,
                capabilities: uses,
                withheld: Capabilities::default(),
                affiliation_unknown: false,
            }),
            Self::Google { .. } => None,
        }
    }

    /// The handle of the shared store this dial opens, `None` for an account's own.
    pub(crate) fn shared_handle(&self) -> Option<&SharedAccountId> {
        self.shared().map(|shared| &shared.handle)
    }

    /// The address the shared store this dial opens is known by, `None` for an account's own.
    fn shared_identity(&self) -> Option<&EmailAddress> {
        self.shared().map(|shared| &shared.identity)
    }

    pub(crate) fn shared(&self) -> Option<&SharedDial> {
        match self {
            Self::Imap { shared, .. }
            | Self::Jmap { shared, .. }
            | Self::Microsoft { shared, .. } => shared.as_ref(),
            Self::Google { .. } => None,
        }
    }
}

/// One store an account is used for besides its own: the id it is listed by and what is stored
/// of it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct SharedChild {
    pub(crate) id: AccountId,
    pub(crate) choice: SharedStoreChoice,
}

/// Every store `entry` is used for besides its own, in the order chosen.
pub(super) fn children(parent: &str, entry: &ConnectedAccount) -> Vec<SharedChild> {
    let Ok(parent) = AccountId::try_from(parent) else {
        return Vec::new();
    };
    entry
        .shape()
        .shared_stores
        .iter()
        .filter_map(|choice| {
            Some(SharedChild {
                id: choice.account_id(&parent).ok()?,
                choice: choice.clone(),
            })
        })
        .collect()
}
impl AccountRegistry {
    /// The account whose sign-in opens the shared store listed as `id`, with what is stored
    /// about it; `None` when `id` is no shared store.
    pub(crate) fn shared_parent(&self, id: &str) -> Option<(String, SharedChild)> {
        self.entries
            .lock()
            .expect("account registry mutex poisoned")
            .iter()
            .find_map(|(parent, entry)| {
                children(parent, entry)
                    .into_iter()
                    .find(|child| child.id.as_str() == id)
                    .map(|child| (parent.clone(), child))
            })
    }

    /// Every store `parent` is used for besides its own, in the order chosen.
    pub(crate) fn shared_children(&self, parent: &str) -> Vec<SharedChild> {
        self.entries
            .lock()
            .expect("account registry mutex poisoned")
            .get(parent)
            .map(|entry| children(parent, entry))
            .unwrap_or_default()
    }

    /// What a push watch on a folder of `id` needs: the IMAP half of `id` itself, or of the
    /// account whose sign-in opens it with the shared mailbox's handle. `None` for an account with
    /// no IMAP half.
    pub(crate) fn imap_store(
        &self,
        id: &str,
    ) -> Option<(super::ImapEntry, Option<SharedAccountId>)> {
        if let Some(own) = self.imap(id) {
            return Some((own, None));
        }
        let (parent, child) = self.shared_parent(id)?;
        child.choice.holds.contains(SharedUse::Mail).then_some(())?;
        Some((self.imap(&parent)?, Some(child.choice.handle)))
    }

    /// The provider-less accounts the app lists for `parent`'s shared stores until each is
    /// dialled.
    pub(crate) fn shared_placeholders(&self, parent: &str) -> Vec<crate::BoxedAccount> {
        self.shared_children(parent)
            .into_iter()
            .filter_map(|child| {
                let dial = self.dial(child.id.as_str())?;
                Some(mailcal_app::Account {
                    identity: dial.shared_identity()?.clone(),
                    providers: Vec::new().into(),
                    calendar_provider: None,
                    meetings: None,
                    contact_providers: Vec::new(),
                    dialled: false,
                    uses_mail: dial.capabilities().contains(Capability::Mail),
                    id: child.id,
                })
            })
            .collect()
    }

    /// How to dial the shared store listed as `id`, from its parent's entry as it is now.
    pub(super) fn dial_shared(&self, id: &str) -> Option<AccountDial> {
        let (parent, child) = self.shared_parent(id)?;
        self.entries
            .lock()
            .expect("account registry mutex poisoned")
            .get(&parent)
            .map(AccountDial::from_entry)?
            .for_shared(&child.choice)
    }

    /// Puts back the stores `parent` is used for as they were, after storing a change failed.
    pub(crate) fn restore_shared_stores(&self, parent: &str, stores: Vec<SharedStoreChoice>) {
        if let Some(entry) = self
            .entries
            .lock()
            .expect("account registry mutex poisoned")
            .get_mut(parent)
        {
            entry.shape_mut().shared_stores = stores;
        }
    }

    /// Records whether `parent` is used for the store `choice`. Switched on again, a store it is
    /// already used for keeps what it held and gains what `choice` says it holds now. Answers
    /// whether anything changed; `false` as well for an account that is not registered.
    pub(crate) fn set_shared_store(
        &self,
        parent: &str,
        choice: SharedStoreChoice,
        on: bool,
    ) -> bool {
        let mut entries = self
            .entries
            .lock()
            .expect("account registry mutex poisoned");
        let Some(entry) = entries.get_mut(parent) else {
            return false;
        };
        let current_id = entry.account_id();
        let shape = entry.shape_mut();
        let chosen = &mut shape.shared_stores;
        let at = chosen.iter().position(|kept| kept.handle == choice.handle);
        let changed = match (at, on) {
            (None, true) => {
                chosen.push(choice);
                true
            }
            (Some(at), true) => {
                let holds = chosen[at].holds.union(choice.holds);
                let gained = holds != chosen[at].holds;
                chosen[at].holds = holds;
                gained
            }
            (Some(at), false) => {
                chosen.remove(at);
                true
            }
            (None, false) => false,
        };
        // Pinned, because each shared store's id is derived from this account's.
        if changed && shape.id.is_none() {
            shape.id = current_id;
        }
        changed
    }
}

#[cfg(test)]
#[path = "shared_tests.rs"]
mod tests;
