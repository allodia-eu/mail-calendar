//! The stores an account is used for besides its own, each listed as an account of its own: a
//! shared mailbox as a mail account, a shared calendar as a calendar account
//! (`docs/shared-mailboxes.md`).
//!
//! A shared store has no registry entry: it is stored on the account whose sign-in opens it, and
//! everything that dials it is read from that account's entry at the moment it is asked. So a
//! replaced password, a rotated grant or a new connection pool reaches the shared store with
//! nothing to keep in step.

use engine_api::{AccountId, EmailAddress, SharedAccountId};
use mailcal_account::{Capabilities, Capability, SharedStoreChoice, SharedUse};

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

/// The one use a store chosen for `kind` is opened for.
fn opened(kind: SharedUse) -> Capabilities {
    let capability = match kind {
        SharedUse::Mail => Capability::Mail,
        SharedUse::Calendar => Capability::Calendar,
    };
    [capability].into_iter().collect()
}

impl AccountDial {
    /// This dial, opening the store `choice` chosen for `kind` with the account's sign-in instead
    /// of the account itself: that use alone, under the store's own address. `None` for a kind
    /// that opens no such store: Google opens none, an IMAP account opens a shared calendar only
    /// through its CalDAV half, and Microsoft only while used for shared mailboxes.
    fn for_shared(self, choice: &SharedStoreChoice, kind: SharedUse) -> Option<Self> {
        let uses = opened(kind);
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
            } if kind == SharedUse::Mail || config.caldav.is_some() => {
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
            // Opened only while the account is used for shared mailboxes and its grant allows
            // them: what the account opens is what it is used for, less what is withheld.
            Self::Microsoft {
                tokens,
                identity,
                capabilities,
                ..
            } if kind == SharedUse::Mail && capabilities.contains(Capability::SharedMailboxes) => {
                Some(Self::Microsoft {
                    tokens,
                    shared: Some(shared(identity.clone())),
                    identity: shared(identity).identity,
                    capabilities: uses,
                    withheld: Capabilities::default(),
                    affiliation_unknown: false,
                })
            }
            Self::Imap { .. } | Self::Microsoft { .. } | Self::Google { .. } => None,
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

/// One store an account is used for besides its own: the id it is listed by, what it was chosen
/// for, and what is stored of it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct SharedChild {
    pub(crate) id: AccountId,
    pub(crate) kind: SharedUse,
    pub(crate) choice: SharedStoreChoice,
}

/// Every store `entry` is used for besides its own, mailboxes first, each in the order chosen.
pub(super) fn children(parent: &str, entry: &ConnectedAccount) -> Vec<SharedChild> {
    let Ok(parent) = AccountId::try_from(parent) else {
        return Vec::new();
    };
    let (shape, parent) = (entry.shape(), &parent);
    [SharedUse::Mail, SharedUse::Calendar]
        .into_iter()
        .flat_map(|kind| {
            shape.shared(kind).iter().filter_map(move |choice| {
                Some(SharedChild {
                    id: choice.account_id(parent, kind).ok()?,
                    kind,
                    choice: choice.clone(),
                })
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

    /// Every store `parent` is used for besides its own, mailboxes first, each in the order
    /// chosen.
    pub(crate) fn shared_children(&self, parent: &str) -> Vec<SharedChild> {
        self.entries
            .lock()
            .expect("account registry mutex poisoned")
            .get(parent)
            .map(|entry| children(parent, entry))
            .unwrap_or_default()
    }

    /// The stores `parent` is used for for `kind`, under the ids each is listed by, in the order
    /// they were chosen.
    pub(crate) fn shared_stores(
        &self,
        parent: &str,
        kind: SharedUse,
    ) -> Vec<(AccountId, SharedStoreChoice)> {
        self.shared_children(parent)
            .into_iter()
            .filter(|child| child.kind == kind)
            .map(|child| (child.id, child.choice))
            .collect()
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
        (child.kind == SharedUse::Mail).then_some(())?;
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
                    uses_mail: child.kind == SharedUse::Mail,
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
            .for_shared(&child.choice, child.kind)
    }

    /// Records whether `parent` is used for the store `choice` for `kind`. Answers whether
    /// anything changed; `false` as well for an account that is not registered.
    pub(crate) fn set_shared_store(
        &self,
        parent: &str,
        kind: SharedUse,
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
        let chosen = shape.shared_mut(kind);
        let at = chosen.iter().position(|kept| kept.handle == choice.handle);
        let changed = match (at, on) {
            (None, true) => {
                chosen.push(choice);
                true
            }
            (Some(at), false) => {
                chosen.remove(at);
                true
            }
            _ => false,
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
