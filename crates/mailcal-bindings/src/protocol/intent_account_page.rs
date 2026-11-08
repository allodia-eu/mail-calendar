//! The account page half of the FFI intent surface, nested behind one `Intent::AccountPage` variant
//! as the other families are. Each is handled by the binding itself, which holds the account
//! registry, and never reaches the core's own intent (`docs/waiting.md`).

use crate::{AccountCapability, LinkSlot};

/// A change an account's page in Settings asks for. Each returns at once: the page says it runs
/// (`AccountPage`) until it ends.
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Enum)]
pub enum AccountPageIntent {
    /// Switch one of the account's uses on or off. A use whose switch `asks_provider` is not
    /// sent here when switched on: it goes to the provider's sign-in, asking for it.
    SetUse {
        /// The account's id.
        account: String,
        /// The use.
        capability: AccountCapability,
        /// On or off.
        on: bool,
    },
    /// Link another account to one of this account's slots, or clear it (`target: None`).
    SetLink {
        /// The account's id.
        account: String,
        /// The slot.
        slot: LinkSlot,
        /// The account to link, from the picker's options; `None` for none.
        target: Option<String>,
    },
    /// Choose what happens when the account's calendar server cannot pass an invitation answer
    /// on.
    SetReplyFallback {
        /// The account's id.
        account: String,
        /// The choice.
        choice: crate::ReplyFallbackChoice,
    },
    /// Remove the account from this device.
    Remove {
        /// The account's id.
        account: String,
    },
}
