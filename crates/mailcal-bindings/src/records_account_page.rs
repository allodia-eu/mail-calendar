//! An account's page in Settings as the core decides it (`docs/accounts.md` rules 11 to 13): every
//! control on it, what it shows, whether it can be used, and whether work it started is still
//! running (`docs/waiting.md`). A client draws this and keeps no rule of its own about it.

use crate::{AccountCapability, LinkSlot, LinkedAccount, UseProblem};

/// What a control the person can act on is doing, as every client draws it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, uniffi::Record)]
pub struct ControlState {
    /// It can be acted on now.
    pub enabled: bool,
    /// Work it started is still running: drawn as the platform's spinner on or beside it. A
    /// running control is never enabled, so it cannot start the same work again.
    pub running: bool,
}

/// Why a use's switch cannot be pressed: the reason the core would refuse it, said before it is
/// (`docs/waiting.md` rule 6). The client words it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, uniffi::Enum)]
pub enum AccountUseBlock {
    /// The last of mail, calendar and contacts: the account is removed instead.
    LastUse,
    /// Colleagues, while contacts are off.
    NeedsContacts,
    /// Shared mailboxes, while mail is off.
    NeedsMail,
    /// The server offers none.
    NotOffered,
    /// The account has no server for it yet: its address goes in the servers below.
    NeedsServer,
}

impl AccountUseBlock {
    /// The reason in a sentence, for a log and for the core's own refusal.
    #[must_use]
    pub const fn reason(self) -> &'static str {
        match self {
            Self::LastUse => "an account is used for at least one of mail, calendar and contacts",
            Self::NeedsContacts => "colleagues are only read beside contacts",
            Self::NeedsMail => "shared mailboxes are only opened beside mail",
            Self::NotOffered => "this account does not offer that",
            Self::NeedsServer => "this account has no server for that",
        }
    }
}

/// What is wrong with a use, said beneath its switch whether or not it can be pressed. The client
/// words it with the switch's capability.
#[derive(Debug, Clone, Copy, PartialEq, Eq, uniffi::Enum)]
pub enum AccountUseStatus {
    /// Its server did not connect, for this reason (`docs/accounts.md` rule 16).
    Problem { problem: UseProblem },
    /// Chosen, and the provider has not granted it; switching it on asks the provider.
    Withheld,
}

/// One use's switch.
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct AccountUseSwitch {
    pub capability: AccountCapability,
    /// Drawn on. A use waiting on the provider's permission is drawn off: it is not working,
    /// and switching it on is what asks again.
    pub on: bool,
    /// Its control is enabled exactly when nothing blocks it and no change runs.
    pub control: ControlState,
    pub blocked: Option<AccountUseBlock>,
    pub status: Option<AccountUseStatus>,
    /// Switching it on goes to the provider's sign-in, asking for it, rather than to the core.
    pub asks_provider: bool,
    /// Switching it off is confirmed first: it deletes what the device holds of it.
    pub confirms_off: bool,
    /// The question that allows its new server is offered beneath it: under the last of the uses
    /// paused for one, so it follows every status that says so.
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
    /// The account was removed, and its stored credential could not be erased: it comes back at
    /// the next launch unless the store lets it go.
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
