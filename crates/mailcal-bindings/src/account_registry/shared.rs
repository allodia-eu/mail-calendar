//! The shared mailboxes an account is used for, each listed as a mail account of its own
//! (`docs/shared-mailboxes.md`).
//!
//! A shared mailbox has no registry entry: it is stored on the account whose sign-in opens it,
//! and everything that dials it is read from that account's entry at the moment it is asked.
//! So a replaced password, a rotated grant or a new connection pool reaches the shared mailbox
//! with nothing to keep in step.

use engine_api::{AccountId, EmailAddress, SharedMailboxId};
use mailcal_account::{Capabilities, Capability, SharedMailboxChoice};

use super::{AccountDial, AccountRegistry};
use crate::connected_account::ConnectedAccount;

/// What a dial of a shared mailbox binds beside its parent account's credential.
#[derive(Debug, Clone)]
pub(crate) struct SharedDial {
    /// The server's handle for the mailbox.
    pub(crate) handle: SharedMailboxId,
    /// The address it sends as: its own, or its parent's when the server gives it none.
    pub(crate) identity: EmailAddress,
}

impl AccountDial {
    /// This dial, opening the shared mailbox `choice` with the account's sign-in instead of the
    /// account itself: mail alone, under the mailbox's own address. `None` for a kind that opens
    /// no shared mailbox.
    fn for_shared(self, choice: &SharedMailboxChoice) -> Option<Self> {
        let mail: Capabilities = [Capability::Mail].into_iter().collect();
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
                    capabilities: mail,
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
                    capabilities: mail,
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
            } if capabilities.contains(Capability::SharedMailboxes) => Some(Self::Microsoft {
                tokens,
                shared: Some(shared(identity.clone())),
                identity: shared(identity).identity,
                capabilities: mail,
                withheld: Capabilities::default(),
                affiliation_unknown: false,
            }),
            Self::Microsoft { .. } | Self::Google { .. } => None,
        }
    }

    /// The handle of the shared mailbox this dial opens, `None` for an account's own.
    pub(crate) fn shared_handle(&self) -> Option<&SharedMailboxId> {
        self.shared().map(|shared| &shared.handle)
    }

    /// The address the shared mailbox this dial opens sends as, `None` for an account's own.
    fn shared_identity(&self) -> Option<&EmailAddress> {
        self.shared().map(|shared| &shared.identity)
    }

    fn shared(&self) -> Option<&SharedDial> {
        match self {
            Self::Imap { shared, .. }
            | Self::Jmap { shared, .. }
            | Self::Microsoft { shared, .. } => shared.as_ref(),
            Self::Google { .. } => None,
        }
    }
}

/// Every shared mailbox `entry` is used for, under the id each is listed by.
pub(super) fn children(
    parent: &str,
    entry: &ConnectedAccount,
) -> impl Iterator<Item = (AccountId, SharedMailboxChoice)> {
    let parent = AccountId::try_from(parent).ok();
    entry
        .shape()
        .shared_mailboxes
        .clone()
        .into_iter()
        .filter_map(move |choice| {
            let id = choice.account_id(parent.as_ref()?).ok()?;
            Some((id, choice))
        })
}

impl AccountRegistry {
    /// The account whose sign-in opens the shared mailbox listed as `id`, with what is stored
    /// about it; `None` when `id` is no shared mailbox.
    pub(crate) fn shared_parent(&self, id: &str) -> Option<(String, SharedMailboxChoice)> {
        self.entries
            .lock()
            .expect("account registry mutex poisoned")
            .iter()
            .find_map(|(parent, entry)| {
                children(parent, entry)
                    .find(|(child, _)| child.as_str() == id)
                    .map(|(_, choice)| (parent.clone(), choice))
            })
    }

    /// The shared mailboxes `parent` is used for, under the ids each is listed by, in the order
    /// they were chosen.
    pub(crate) fn shared_mailboxes(&self, parent: &str) -> Vec<(AccountId, SharedMailboxChoice)> {
        self.entries
            .lock()
            .expect("account registry mutex poisoned")
            .get(parent)
            .map(|entry| children(parent, entry).collect())
            .unwrap_or_default()
    }

    /// What a push watch on a folder of `id` needs: the IMAP half of `id` itself, or of the
    /// account whose sign-in opens it with the shared mailbox's handle. `None` for an account with
    /// no IMAP half.
    pub(crate) fn imap_store(
        &self,
        id: &str,
    ) -> Option<(super::ImapEntry, Option<SharedMailboxId>)> {
        if let Some(own) = self.imap(id) {
            return Some((own, None));
        }
        let (parent, choice) = self.shared_parent(id)?;
        Some((self.imap(&parent)?, Some(choice.handle)))
    }

    /// The provider-less accounts the app lists for `parent`'s shared mailboxes until each is
    /// dialled.
    pub(crate) fn shared_placeholders(&self, parent: &str) -> Vec<crate::BoxedAccount> {
        self.shared_mailboxes(parent)
            .into_iter()
            .filter_map(|(id, _)| {
                let dial = self.dial(id.as_str())?;
                Some(mailcal_app::Account {
                    identity: dial.shared_identity()?.clone(),
                    providers: Vec::new().into(),
                    calendar_provider: None,
                    contact_providers: Vec::new(),
                    dialled: false,
                    uses_mail: true,
                    id,
                })
            })
            .collect()
    }

    /// How to dial the shared mailbox listed as `id`, from its parent's entry as it is now.
    pub(super) fn dial_shared(&self, id: &str) -> Option<AccountDial> {
        let (parent, choice) = self.shared_parent(id)?;
        self.entries
            .lock()
            .expect("account registry mutex poisoned")
            .get(&parent)
            .map(AccountDial::from_entry)?
            .for_shared(&choice)
    }

    /// Records whether `parent` is used for the shared mailbox `choice`. Answers whether
    /// anything changed; `false` as well for an account that is not registered.
    pub(crate) fn set_shared_mailbox(
        &self,
        parent: &str,
        choice: SharedMailboxChoice,
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
        let chosen = &mut shape.shared_mailboxes;
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
        // Pinned, because each shared mailbox's id is derived from this account's.
        if changed && shape.id.is_none() {
            shape.id = current_id;
        }
        changed
    }
}

#[cfg(test)]
#[path = "shared_tests.rs"]
mod tests;
