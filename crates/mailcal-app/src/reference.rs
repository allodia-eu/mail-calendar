//! Typed references to a synced message or calendar event: an [`AccountId`] bound to
//! its [`ProviderKey`]. The command layer speaks in these so an action can never carry a
//! key without its owning account, nor pair a key with a *different* account's id; the
//! wrong-account routing class of bug (a provider key is unique only *within* an account,
//! so two accounts can mint the same one). There is no command that takes a bare key, so
//! the mismatch is unrepresentable in the core rather than merely tested against.
//!
//! A reference is built **once**, at the FFI boundary ([`crate::Intent`] conversion),
//! from the very row the host clicked; both halves travel together from there on.

use engine_api::{AccountId, ProviderKey};

/// A synced message, identified by its owning account and provider key **together**.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MessageRef {
    /// The account that owns the message (the row's account).
    pub account: AccountId,
    /// The message's provider key; unique only within `account`.
    pub key: ProviderKey,
}

/// A synced calendar event, identified by its owning account and provider key **together**
/// (the calendar counterpart of [`MessageRef`]).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EventRef {
    /// The account that owns the event (the row's account).
    pub account: AccountId,
    /// The event's provider key; unique only within `account`.
    pub key: ProviderKey,
}

/// A folder of one account, identified by its owning account and folder key **together**.
///
/// A folder key is unique only *within* an account (every provider calls its inbox `inbox`) so
/// a pane holding every account's tree holds several rows carrying the same key
/// (`docs/folder-pane.md`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FolderRef {
    /// The account that owns the folder (the pane row's account).
    pub account: AccountId,
    /// The folder's key, as projected into the snapshot's folder rows; unique only within
    /// `account`. A plain identifier, not a provider key.
    pub key: String,
}

/// A synced conversation, identified by its owning account and thread id **together** (the
/// thread counterpart of [`MessageRef`]): a thread id is projected per account, so it must
/// travel with its account for an action to route correctly.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ThreadRef {
    /// The account that owns the conversation (the row's account).
    pub account: AccountId,
    /// The thread's id, as projected into the mailbox-list snapshot (a resolved thread id, or
    /// a lone message's own key). A plain string, not a provider key.
    pub thread_id: String,
}

/// A mailbox-list **row** an action names: one message in the flat list, or one whole
/// conversation in the threaded list.
///
/// What a user selects is a row, and the two view modes put different things on one, so an
/// action over several rows carries the rows themselves rather than a set of messages a client
/// resolved for it. Only the core can expand a conversation correctly: its members come from
/// the store's thread index, which holds messages a windowed list never listed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RowRef {
    /// One message: a flat row, or one message of an expanded conversation.
    Message(MessageRef),
    /// One conversation: a threaded row, standing for every message on the thread.
    Thread(ThreadRef),
}

impl RowRef {
    /// The account that owns the row. Every write routes by this, so a selection spanning
    /// accounts (the unified list allows one) is applied account by account.
    #[must_use]
    pub fn account(&self) -> &AccountId {
        match self {
            Self::Message(message) => &message.account,
            Self::Thread(thread) => &thread.account,
        }
    }
}

impl MessageRef {
    /// Builds a reference from a host's `account` id and provider `key` strings; the
    /// single construction point, at the FFI boundary. Returns `None` if either is
    /// malformed, so the caller can drop the intent rather than risk routing to the wrong
    /// account (in practice impossible; both come from a real row the host passed back).
    #[must_use]
    pub fn from_parts(account: &str, key: String) -> Option<Self> {
        Some(Self {
            account: AccountId::try_from(account).ok()?,
            key: ProviderKey::new(key).ok()?,
        })
    }
}

impl EventRef {
    /// Builds an event reference from a host's `account` id and provider `key` strings;
    /// the single construction point, at the FFI boundary. Returns `None` if either is
    /// malformed (see [`MessageRef::from_parts`]).
    #[must_use]
    pub fn from_parts(account: &str, key: String) -> Option<Self> {
        Some(Self {
            account: AccountId::try_from(account).ok()?,
            key: ProviderKey::new(key).ok()?,
        })
    }
}

impl FolderRef {
    /// Builds a folder reference from a host's `account` id and folder `key` strings; the
    /// single construction point, at the FFI boundary. Returns `None` if the `account` is
    /// malformed, so the caller can drop the intent rather than open some other account's
    /// folder of the same name. `key` is a plain identifier and is not validated.
    #[must_use]
    pub fn from_parts(account: &str, key: String) -> Option<Self> {
        Some(Self {
            account: AccountId::try_from(account).ok()?,
            key,
        })
    }
}

impl ThreadRef {
    /// Builds a thread reference from a host's `account` id and `thread_id` strings; the
    /// single construction point, at the FFI boundary. Returns `None` if the `account` is
    /// malformed, so the caller can drop the intent rather than route to the wrong account.
    /// `thread_id` is a plain identifier and is not validated.
    #[must_use]
    pub fn from_parts(account: &str, thread_id: String) -> Option<Self> {
        Some(Self {
            account: AccountId::try_from(account).ok()?,
            thread_id,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::MessageRef;

    #[test]
    fn from_parts_needs_both_halves_well_formed() {
        // A well-formed account + key builds a reference; a blank (malformed) account or
        // key yields `None`, so the boundary drops it rather than carrying half a reference.
        assert!(MessageRef::from_parts("acct-1", "imap:v1:u1@INBOX".to_owned()).is_some());
        assert!(MessageRef::from_parts("", "imap:v1:u1@INBOX".to_owned()).is_none());
        assert!(MessageRef::from_parts("acct-1", String::new()).is_none());
    }
}
