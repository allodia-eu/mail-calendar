//! An account's page in Settings as the core decides it (`docs/accounts.md` rules 11 to 13): every
//! control on it, what it shows, whether it can be used, and whether work it started is still
//! running (`docs/waiting.md`). A client draws this and keeps no rule of its own about it.

use crate::{AccountCapability, FolderRole, LinkSlot, LinkedAccount, UseProblem};

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
    /// The action that opens the separate server's form is offered beneath it: under the last of
    /// the uses that need it, so it follows every status that says why.
    #[uniffi(default = false)]
    pub signs_in_separately: bool,
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

/// The name an account's mail is sent under (`docs/sending.md`), drawn under the sharing control
/// while the account is used for mail.
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct AccountSenderName {
    /// What the field holds, empty when nobody has set one. Where the name is not `editable` it is
    /// the line drawn instead, and falls back to the account's address.
    pub name: String,
    /// The person may change it: `false` where the provider holds the name, as a Microsoft
    /// mailbox's directory does, and the page says so instead of offering a field.
    pub editable: bool,
    pub control: ControlState,
}

/// A choice of one count from a list, such as months of mail or megabytes. `0` in `options`
/// means all of it, which the client words.
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct AccountCountChoice {
    pub options: Vec<u16>,
    /// The index into `options` of the count in effect.
    pub selected: u32,
    pub control: ControlState,
}

/// A folder that can be watched for mail as it arrives.
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct AccountWatchedFolder {
    /// The folder's key, sent back to watch it or not.
    pub key: String,
    /// Its server name with the folders it sits in; a client words a role-bearing folder its own
    /// way, as the folder pane does (`docs/folder-pane.md`).
    pub name: String,
    pub role: Option<FolderRole>,
    pub on: bool,
    /// Held as well while it is off and the most folders an account may watch already are.
    pub control: ControlState,
}

/// The mailboxes an account's sign-in opens besides its own (`docs/shared-mailboxes.md`), as its
/// server last answered and as the account is used.
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct AccountSharedMailboxes {
    /// A switch per mailbox: the chosen ones first, then the rest the server listed.
    pub mailboxes: Vec<AccountSharedMailbox>,
    /// The line under the switches, when one is said.
    pub note: Option<SharedMailboxesNote>,
    /// For a server that lists none and opens one at an address the person types: the field.
    pub address: Option<AccountSharedAddress>,
}

/// One mailbox's switch. Switching one off deletes what the device holds of it, so a client asks
/// first (rule 7).
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct AccountSharedMailbox {
    /// The server's handle, sent back to switch it.
    pub handle: String,
    /// Its address, or the server's label where it has none.
    pub title: String,
    /// The server's label beside the address, when it says something else.
    pub label: Option<String>,
    /// Chosen, and no longer listed by the server: said in place of the label (rule 8).
    pub withdrawn: bool,
    pub on: bool,
    pub control: ControlState,
}

/// The line under an account's shared mailboxes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, uniffi::Enum)]
pub enum SharedMailboxesNote {
    /// The server lists them, and lists none.
    NoneShared,
    /// The server lists none and opens one at an address the person types.
    ByAddress,
    /// The account is not connected yet, so there is nothing to ask.
    NotConnected,
    /// The server was asked and did not answer.
    Unreachable,
}

/// The address field on a server that opens a shared mailbox at an address the person types.
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct AccountSharedAddress {
    /// What the field is drawn with: the address being added while an add runs, the one the
    /// last add did not find so it can be corrected, and otherwise nothing.
    pub text: String,
    /// Add, running while the server is asked.
    pub add: ControlState,
}

/// An account's servers and login, for an account whose servers can be edited
/// (`docs/accounts.md` rule 14): what is stored, which the form is drawn with, and its Save.
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct AccountServers {
    /// What the form is drawn with: the edit being saved while a save runs, the edit the last
    /// save refused (its password included) so one field is corrected rather than all retyped,
    /// and otherwise what is stored, with no password.
    pub form: crate::AccountEndpoints,
    /// Save, running while the new servers are tried. The fields are held with it, without its
    /// spinner, so nothing typed during a save is drawn over when it ends; until Save is pressed
    /// they are the person's own, and [`crate::MailcalApp::account_endpoints_gap`] says what they
    /// leave out.
    pub save: ControlState,
    /// The server refused the stored password, so the section says so: a new one is typed into
    /// its form.
    #[uniffi(default = false)]
    pub sign_in_refused: bool,
}

/// What an edit of an account's servers leaves out, which its save refuses and the page says
/// before Save is pressed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, uniffi::Enum)]
pub enum AccountEndpointsGap {
    /// No login.
    Username,
    /// A new password given empty.
    Password,
    /// The IMAP server has no address.
    Imap,
    /// The SMTP server has no address: mail could no longer be sent.
    Smtp,
    /// The calendar server has no address.
    CalDav,
    /// The address book server has no address, and no calendar server serves contacts instead.
    CardDav,
}

impl From<mailcal_account::EndpointGap> for AccountEndpointsGap {
    fn from(gap: mailcal_account::EndpointGap) -> Self {
        use mailcal_account::EndpointGap;
        match gap {
            EndpointGap::Username => Self::Username,
            EndpointGap::Password => Self::Password,
            EndpointGap::Imap => Self::Imap,
            EndpointGap::Smtp => Self::Smtp,
            EndpointGap::CalDav => Self::CalDav,
            EndpointGap::CardDav => Self::CardDav,
        }
    }
}

/// How an account's mail arrives and how much of it this device keeps (`docs/settings.md`).
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct AccountMailSettings {
    /// Months of mail kept.
    pub depth: AccountCountChoice,
    /// The largest message downloaded in full in the background, in megabytes.
    pub size: AccountCountChoice,
    /// Mail arrives as it comes (`on`) or on a timer. `None` where the server cannot push, and the
    /// page says the account checks on a timer.
    pub push: Option<AccountToggle>,
    /// Minutes between checks, while the account checks on a timer.
    pub interval: Option<AccountCountChoice>,
    /// The folders watched, while mail arrives as it comes.
    pub folders: Vec<AccountWatchedFolder>,
    /// The most folders an account may watch, which the page says above them.
    pub watch_limit: u8,
}

/// A setting that is on or off.
#[derive(Debug, Clone, Copy, PartialEq, Eq, uniffi::Record)]
pub struct AccountToggle {
    pub on: bool,
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
        refusal: AccountChangeRefusal,
    },
    /// Linking `slot` did not take.
    LinkFailed {
        slot: LinkSlot,
        refusal: AccountChangeRefusal,
    },
    /// The account was removed, and its stored credential could not be erased: it comes back at
    /// the next launch unless the store lets it go.
    RemoveFailed { detail: String },
    /// The edited servers were not kept: they did not connect, refused the login, or are another
    /// account's. The account is as it was.
    ServersFailed { refusal: AccountChangeRefusal },
    /// Switching a shared mailbox, or adding one, did not take.
    SharedMailboxFailed { refusal: AccountChangeRefusal },
    /// The password typed into the Sign-in section was not kept: the server refused it, or could
    /// not be reached. The account signs in as it did.
    PasswordFailed { refusal: AccountChangeRefusal },
    /// Signing in through the browser did not complete, and the account signs in as it did.
    /// `switching`: it was to move the account from a password to its provider or Nextcloud.
    SignInFailed {
        switching: bool,
        refusal: AccountChangeRefusal,
    },
    /// The sign-in worked, and the provider still withholds `uses` the account is used for: the
    /// person left them unticked on its consent page, or an administrator does not allow them.
    SignedInWithheld { uses: Vec<AccountCapability> },
    /// Nothing the account's sign-in opens answers to `address`. The server will not say
    /// whether no such mailbox exists or it was not shared with the account, so neither does this.
    SharedMailboxNotFound { address: String },
}

/// Why a change did not take, for a client to say in the person's language: the reasons it can
/// name, and anything else as the core puts it.
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Enum)]
pub enum AccountChangeRefusal {
    /// The server refused the sign-in: a wrong password, or a grant it no longer accepts.
    SignInRefused,
    /// The servers are another account's, already set up.
    AlreadySetUp,
    /// The sign-in was for a different account than this one.
    DifferentAccount,
    /// The browser returned from another sign-in.
    StaleSignIn,
    /// A use of a server of its own did not connect, for `problem`.
    UseNotConnected {
        capability: AccountCapability,
        problem: crate::UseProblem,
    },
    /// The switch is refused for `block`, as its page says beside it.
    Blocked { block: AccountUseBlock },
    /// The provider has not granted the use.
    Withheld,
    /// The shared mailbox is no longer one the account's sign-in opens.
    NoLongerShared,
    /// The browser could not be opened.
    BrowserNotOpened,
    /// Nothing came back from the browser before the sign-in's wait ran out.
    NotFinished,
    /// Anything else, as the core renders it.
    Other { detail: String },
}

impl From<&crate::MailcalError> for AccountChangeRefusal {
    fn from(error: &crate::MailcalError) -> Self {
        use crate::MailcalError;
        match error {
            MailcalError::SignInRefused { .. } => Self::SignInRefused,
            MailcalError::AlreadySetUp { .. } => Self::AlreadySetUp,
            MailcalError::DifferentAccount => Self::DifferentAccount,
            MailcalError::StaleSignIn => Self::StaleSignIn,
            MailcalError::UseNotConnected {
                capability,
                problem,
            } => Self::UseNotConnected {
                capability: *capability,
                problem: *problem,
            },
            other => Self::Other {
                detail: other.to_string(),
            },
        }
    }
}

/// What happens when the account's calendar server cannot pass an invitation answer on to the
/// organiser (`docs/invitations.md`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, uniffi::Enum)]
pub enum ReplyFallbackChoice {
    /// Ask each time, with the `ReplyNotDelivered` notice.
    Ask,
    /// Email the answer from the account without asking: standing permission to send mail as the
    /// person.
    Always,
    /// Never email it. The card still says the organiser was not told.
    Never,
}

/// The picker for [`ReplyFallbackChoice`], on the page of an account used for its calendar.
#[derive(Debug, Clone, Copy, PartialEq, Eq, uniffi::Record)]
pub struct AccountReplyFallback {
    /// What is chosen, or what a change running is moving it to.
    pub choice: ReplyFallbackChoice,
    pub control: ControlState,
}

/// An account's page: what [`crate::MailcalApp::account_page`] answers for the page on screen.
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct AccountPage {
    pub account: String,
    pub uses: Vec<AccountUseSwitch>,
    pub links: Vec<AccountLinkPicker>,
    /// While the account is used for mail.
    pub sender_name: Option<AccountSenderName>,
    /// While the account is used for mail.
    pub mail: Option<AccountMailSettings>,
    /// For an account whose servers can be edited: a standards account that signs in with a
    /// password.
    pub servers: Option<AccountServers>,
    /// Once its server has answered, for an account whose sign-in can open a shared mailbox or
    /// that still has one chosen.
    pub shared: Option<AccountSharedMailboxes>,
    /// How it signs in, and the ways in it offers; `None` for a calendar and contacts account
    /// that is not at a Nextcloud server.
    #[uniffi(default = None)]
    pub sign_in: Option<crate::AccountSignIn>,
    /// While calendar or contacts need a sign-in of their own: the form that gives them one.
    #[uniffi(default = None)]
    pub separate_server: Option<crate::AccountSeparateServer>,
    pub remove: ControlState,
    /// What happens to an invitation answer its calendar server cannot pass on; `None` for an
    /// account not used for its calendar.
    #[uniffi(default = None)]
    pub reply_fallback: Option<AccountReplyFallback>,
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
