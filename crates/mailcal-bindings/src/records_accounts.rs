//! The records Settings → Accounts is drawn from: every account, mail or not, what it is used for,
//! and which accounts it relies on.

use crate::{AccountCapability, ConnectionSecurity};

/// The accounts on this device, in the order the host stored them.
///
/// Empty means there is no account left, and a client returns to first-run setup.
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct AccountsSnapshot {
    /// Every account, mail or not.
    pub accounts: Vec<AccountEntry>,
}

/// One account as Settings → Accounts lists it.
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
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
    /// Its servers and login, for an account Settings can edit them on: one that signs in with a
    /// password to servers of its own. `None` for one that signs in through its provider.
    pub endpoints: Option<AccountEndpoints>,
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
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
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
#[derive(Debug, Clone, Default, PartialEq, Eq, uniffi::Record)]
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
#[derive(Debug, Clone, Default, PartialEq, Eq, uniffi::Record)]
pub struct LinkCandidates {
    /// Calendars a mail account may file its invitations through.
    pub calendar: Vec<LinkedAccount>,
    /// Address books a mail account may save new contacts to.
    pub contacts: Vec<LinkedAccount>,
    /// Mail accounts a calendar account may send its invitations through.
    pub mail: Vec<LinkedAccount>,
    /// The ids of the candidates above to suggest: a calendar whose server schedules as this
    /// account's address, or a mail account whose address this calendar's server schedules as.
    /// Setup and Settings pre-select one where nothing is linked yet; the person confirms.
    pub suggested: Vec<String>,
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

/// What switching one of an account's uses on or off did.
#[derive(Debug, Clone, Copy, PartialEq, Eq, uniffi::Enum)]
pub enum CapabilityChange {
    /// Done, and stored. Switched off, its local data is gone; switched on, the account is
    /// reconnecting to open it.
    Applied,
    /// Nothing changed: the provider has not granted it. Sign the account in again for it
    /// (`begin_account_consent`, adding it), which switches it on.
    NeedsConsent,
    /// Nothing changed: the account has no server for it. Its address has to be entered first.
    NeedsEndpoint,
}

/// An account's servers and sign-in, as Settings shows and edits them. A server that is `None` is
/// one the account does not have.
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct AccountEndpoints {
    /// The IMAP server, `host` or `host:port`; the standard port for its security when bare.
    pub imap_host: Option<String>,
    /// How the IMAP connection is secured.
    pub imap_security: ConnectionSecurity,
    /// The SMTP server, `host` or `host:port`.
    pub smtp_host: Option<String>,
    /// How the SMTP connection is secured.
    pub smtp_security: ConnectionSecurity,
    /// The CalDAV base URL; `https://` is assumed when it names no scheme.
    pub caldav_url: Option<String>,
    /// The CardDAV base URL, when it is not the calendar's.
    pub carddav_url: Option<String>,
    /// The login every server takes.
    pub username: String,
    /// A new password for every server, or `None` to keep the stored one. Always `None` in the
    /// snapshot: a password is never handed back.
    pub password: Option<String>,
}

impl From<mailcal_account::EndpointEdit> for AccountEndpoints {
    fn from(edit: mailcal_account::EndpointEdit) -> Self {
        Self {
            imap_host: edit.imap_host,
            imap_security: edit.imap_security.into(),
            smtp_host: edit.smtp_host,
            smtp_security: edit.smtp_security.into(),
            caldav_url: edit.caldav_url,
            carddav_url: edit.carddav_url,
            username: edit.username,
            password: None,
        }
    }
}

impl From<AccountEndpoints> for mailcal_account::EndpointEdit {
    fn from(endpoints: AccountEndpoints) -> Self {
        Self {
            imap_host: endpoints.imap_host,
            imap_security: endpoints.imap_security.into(),
            smtp_host: endpoints.smtp_host,
            smtp_security: endpoints.smtp_security.into(),
            caldav_url: endpoints.caldav_url,
            carddav_url: endpoints.carddav_url,
            username: endpoints.username,
            password: endpoints.password,
        }
    }
}
