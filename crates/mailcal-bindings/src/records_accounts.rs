//! The records Settings → Accounts is drawn from: every account, mail or not, what it is used for,
//! and which accounts it relies on.

use crate::AccountCapability;

/// The accounts on this device, in the order the host stored them.
///
/// Empty means there is no account left, and a client returns to first-run setup.
#[derive(Debug, Clone, uniffi::Record)]
pub struct AccountsSnapshot {
    /// Every account, mail or not.
    pub accounts: Vec<AccountEntry>,
}

/// One account as Settings → Accounts lists it.
#[derive(Debug, Clone, uniffi::Record)]
pub struct AccountEntry {
    /// The account's id, which every account method takes. Never shown.
    pub id: String,
    /// What a person reads the account by.
    pub address: String,
    /// Which sign-in the account is, for its label.
    pub kind: AccountKind,
    /// Each use the kind can offer, in a fixed order, with its state.
    pub uses: Vec<AccountUse>,
    /// The accounts this one relies on for what it is not used for itself.
    pub links: AccountLinksView,
    /// The accounts relying on this one, which lose their link if it is removed.
    pub linked_from: Vec<LinkedAccount>,
    /// The accounts each link may name, for its picker: empty for a slot the account cannot
    /// hold, because it is used for that itself.
    pub link_candidates: LinkCandidates,
}

/// Which sign-in an account is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, uniffi::Enum)]
pub enum AccountKind {
    /// A mailbox over IMAP and SMTP, with any calendar and address book beside it.
    Imap,
    /// A calendar or address book over CalDAV and CardDAV, used for no mail.
    Dav,
    /// A JMAP account.
    Jmap,
    /// A Microsoft 365 or Outlook.com account.
    Microsoft,
    /// A Google account.
    Google,
}

/// One use of an account and its state.
#[derive(Debug, Clone, uniffi::Record)]
pub struct AccountUse {
    /// The use.
    pub capability: AccountCapability,
    /// Whether the account is used for it, and whether that works.
    pub state: CapabilityState,
}

/// Whether an account is used for something.
#[derive(Debug, Clone, Copy, PartialEq, Eq, uniffi::Enum)]
pub enum CapabilityState {
    /// Used for it.
    On,
    /// Not used for it.
    Off,
    /// Used for it, and the provider has not granted the permission it needs: signing in again
    /// (`begin_account_consent`) asks for it.
    NeedsPermission,
}

/// The accounts one account relies on, each already checked against the accounts that exist.
#[derive(Debug, Clone, Default, uniffi::Record)]
pub struct AccountLinksView {
    /// The calendar a mail account files and answers its invitations through.
    pub calendar: Option<LinkedAccount>,
    /// The address book a mail account saves new contacts to.
    pub contacts: Option<LinkedAccount>,
    /// The mail account a calendar account sends its invitations through.
    pub mail: Option<LinkedAccount>,
}

/// Another account, as a link names it.
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct LinkedAccount {
    /// Its id.
    pub id: String,
    /// What a person reads it by.
    pub address: String,
}

/// The accounts each link slot may name.
#[derive(Debug, Clone, Default, uniffi::Record)]
pub struct LinkCandidates {
    /// Calendars a mail account may file its invitations through.
    pub calendar: Vec<LinkedAccount>,
    /// Address books a mail account may save new contacts to.
    pub contacts: Vec<LinkedAccount>,
    /// Mail accounts a calendar account may send its invitations through.
    pub mail: Vec<LinkedAccount>,
}

/// One link an account can hold.
#[derive(Debug, Clone, Copy, PartialEq, Eq, uniffi::Enum)]
pub enum LinkSlot {
    /// The calendar a mail account files and answers its invitations through.
    Calendar,
    /// The address book a mail account saves new contacts to.
    Contacts,
    /// The mail account a calendar account sends its invitations through.
    Mail,
}
