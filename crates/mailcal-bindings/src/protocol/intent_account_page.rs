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
    /// Send the account's mail under `name`; empty for none. Its provider is told afterwards,
    /// where it keeps one, without the page waiting on it.
    SetSenderName { account: String, name: String },
    /// Keep `months` of the account's mail, from the depth choice's options. The mail is
    /// reconciled afterwards, as sync progress, without the page waiting on it.
    SetSyncDepth { account: String, months: u16 },
    /// Download messages up to `megabytes` in full in the background, from the size choice's
    /// options. Reconciled afterwards, without the page waiting on it.
    SetMessageSizeLimit { account: String, megabytes: u16 },
    /// Have mail arrive as it comes (`on`) or on a timer.
    SetPush { account: String, on: bool },
    /// Check every `minutes`, from the interval choice's options.
    SetPollInterval { account: String, minutes: u16 },
    /// Watch the folder `key` for mail as it arrives, or stop.
    SetWatchedFolder {
        account: String,
        key: String,
        on: bool,
    },
    /// Try `endpoints` and keep them once they connect, from the page's servers with what the
    /// person typed; `password: None` keeps the stored one.
    SaveServers {
        account: String,
        endpoints: crate::AccountEndpoints,
    },
    /// Use the account for the shared mailbox `handle`, from its switch, or stop: switched off,
    /// what the device holds of it is deleted.
    SetSharedMailbox {
        account: String,
        handle: String,
        on: bool,
    },
    /// Use the account for the shared mailbox at `address`, once its server opens one there.
    AddSharedMailbox { account: String, address: String },
}

impl AccountPageIntent {
    /// The account the change is to.
    pub(crate) fn account(&self) -> &str {
        match self {
            Self::SetUse { account, .. }
            | Self::SetLink { account, .. }
            | Self::SetReplyFallback { account, .. }
            | Self::Remove { account }
            | Self::SetSenderName { account, .. }
            | Self::SetSyncDepth { account, .. }
            | Self::SetMessageSizeLimit { account, .. }
            | Self::SetPush { account, .. }
            | Self::SetPollInterval { account, .. }
            | Self::SetWatchedFolder { account, .. }
            | Self::SaveServers { account, .. }
            | Self::SetSharedMailbox { account, .. }
            | Self::AddSharedMailbox { account, .. } => account,
        }
    }
}
