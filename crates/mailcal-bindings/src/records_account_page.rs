//! An account's page in Settings as the core decides it (`docs/accounts.md` rules 11 to 13): every
//! control on it, what it shows, whether it can be used, and whether work it started is still
//! running (`docs/waiting.md`). A client draws this and keeps no rule of its own about it.

use crate::{AccountCapability, LinkSlot, LinkedAccount};

/// What a control the person can act on is doing, as every client draws it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, uniffi::Record)]
pub struct ControlState {
    /// It can be acted on now.
    pub enabled: bool,
    /// Work it started is still running: drawn as the platform's spinner on or beside it. A
    /// running control is never enabled, so it cannot start the same work again.
    pub running: bool,
}

/// The line under a use's switch. The client words it; which one applies is decided here.
#[derive(Debug, Clone, Copy, PartialEq, Eq, uniffi::Enum)]
pub enum AccountUseNote {
    /// The last of mail, calendar and contacts: the account is removed instead.
    Last,
    /// Colleagues, while contacts are off.
    NeedsContacts,
    /// Shared mailboxes, while mail is off.
    NeedsMail,
    /// Chosen, and the provider has not granted it; switching it on asks the provider.
    Withheld,
    /// The server offers none, so nothing can switch it on.
    NotOffered,
    /// Off, and the account has no server for it yet: its address goes in the servers below.
    NeedsServer,
    /// On, and its server could not be reached; the next refresh tries again.
    UnreachableCalendar,
    UnreachableContacts,
    /// On, and its server refused the account's sign-in.
    RefusedCalendar,
    RefusedContacts,
    /// On, and its server needs a sign-in of its own.
    OwnSignInCalendar,
    OwnSignInContacts,
    /// Paused: its server was found on another site, and waits for the person to allow it.
    ServerChangedCalendar,
    ServerChangedContacts,
    /// On, and its server has none for this account; a refresh does not ask again.
    NoneOnServerCalendar,
    NoneOnServerContacts,
}

/// One use's switch.
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct AccountUseSwitch {
    pub capability: AccountCapability,
    /// Drawn on. A use waiting on the provider's permission is drawn off: it is not working,
    /// and switching it on is what asks again.
    pub on: bool,
    pub control: ControlState,
    pub note: Option<AccountUseNote>,
    /// Switching it on goes to the provider's sign-in, asking for it, rather than to the core.
    pub asks_provider: bool,
    /// Switching it off is confirmed first: it deletes what the device holds of it.
    pub confirms_off: bool,
    /// The question that allows its new server is offered beneath it.
    pub reviews_server_change: bool,
}

/// One link slot's picker: "none", then the accounts it may name.
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct AccountLinkPicker {
    pub slot: LinkSlot,
    pub options: Vec<LinkedAccount>,
    /// The index into `options` of the account linked now; `None` for "none".
    pub selected: Option<u32>,
    /// The index into `options` to suggest while nothing is linked: an account whose calendar
    /// server schedules as the mail account's address.
    pub suggested: Option<u32>,
    pub control: ControlState,
}

/// Something about the page's last change that needs saying, once. A change that worked says
/// nothing: the page shows its result.
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Enum)]
pub enum AccountChangeNotice {
    /// Switching `capability` did not take; `detail` is the core's reason.
    UseFailed {
        capability: AccountCapability,
        on: bool,
        detail: String,
    },
    /// Linking `slot` did not take.
    LinkFailed { slot: LinkSlot, detail: String },
    /// The account could not be removed.
    RemoveFailed { detail: String },
}

/// An account's page: what [`crate::MailcalApp::account_page`] answers for the page on screen.
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct AccountPage {
    pub account: String,
    pub uses: Vec<AccountUseSwitch>,
    pub links: Vec<AccountLinkPicker>,
    pub remove: ControlState,
    /// What the last change came to, when it needs saying; `id` grows with each, so a client
    /// says each one once.
    pub notice: Option<AccountChangeNoticeRaised>,
}

/// A notice and the number that tells it from the one before.
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct AccountChangeNoticeRaised {
    pub id: u64,
    pub notice: AccountChangeNotice,
}
