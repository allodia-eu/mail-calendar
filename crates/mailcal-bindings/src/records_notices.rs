//! The notices a host draws: every standing "something needs your attention" in one ranked list,
//! decided here so that no client infers one from an error, a field or a string
//! (`docs/notices.md`).

use std::collections::HashMap;

use crate::{FolderAction, FolderProblem, InvitationResponse, UseProblem};

/// Every notice in force, pulled after a `Surface::Notices` signal.
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct NoticesSnapshot {
    /// Ranked: severity, then the whole app before one account, then older before newer
    /// (`docs/notices.md` rule 3). A host draws the first that belongs to the screen on show
    /// and puts the rest behind one Issues entry.
    pub notices: Vec<Notice>,
}

/// One notice: one fact about the app or about one account.
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct Notice {
    /// Stable for as long as the fact holds; what `NoticeIntent` names it by. Opaque: a host
    /// compares it and hands it back, and never reads anything out of it.
    pub id: String,
    /// What it says, with what the words need.
    pub kind: NoticeKind,
    /// The sentence to show, ready for the generated `notice` lookup (`docs/notices.md` rule 11).
    pub title: NoticeText,
    /// What a host shows under the title when there is room: why, and what that means. `None`
    /// when the title says it all.
    pub detail: Option<NoticeText>,
    /// How much of what the person relies on it touches.
    pub severity: NoticeSeverity,
    /// Where it is drawn.
    pub screens: NoticeScreens,
    /// The id of the account it is about, or `None` when it is about the whole app.
    pub account: Option<String>,
    /// A few words for the account's row, which already shows its address, so they name no
    /// address and take no arguments: one line under it (`docs/notices.md` rule 6). There exactly
    /// when `account` is.
    pub mark: Option<NoticeText>,
    /// The one thing a host offers to do about it, or `None` when there is nothing to do. For a
    /// question (`docs/notices.md` rule 8), the answer that does something.
    pub action: Option<NoticeAction>,
    /// The label of `action`'s button, there exactly when `action` is.
    pub action_label: Option<NoticeText>,
    /// A question's other answer, drawn beside `action` as the lesser button; `None` for
    /// anything that is not a question.
    #[uniffi(default = None)]
    pub second_action: Option<NoticeAction>,
    /// The label of `second_action`'s button, there exactly when `second_action` is.
    #[uniffi(default = None)]
    pub second_action_label: Option<NoticeText>,
    /// An answer is being carried out: a host holds both buttons, with its spinner on `action`'s
    /// (`docs/waiting.md`), until it ends.
    #[uniffi(default = false)]
    pub running: bool,
    /// Whether a host offers to close it. Only a notice about something that already happened
    /// can be closed; one about a state that holds goes when the state does.
    pub dismissible: bool,
    /// The person ran `action` and the host reported that it did not complete. A host says so
    /// beside the notice. Cleared when the notice goes.
    pub remedy_failed: bool,
}

/// Words named by their catalog key, with what fills its placeholders. A host turns them into
/// text with the generated `notice` lookup, the same for every key, so no client keeps a switch
/// of its own; every key starts `notice_`, and every argument is a string.
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct NoticeText {
    /// The catalog key.
    pub key: String,
    /// Each placeholder's value, by its name in the catalog.
    pub args: HashMap<String, String>,
}

/// What a notice says. The words are the catalog's, per kind; the arguments are here.
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Enum)]
pub enum NoticeKind {
    /// The account's stored sign-in no longer works, and nothing about it syncs until the
    /// person signs in again.
    SignInExpired,
    /// The account's grant does not let this app send or change mail; reading still works.
    MailReauth,
    /// The account's grant does not let this app see its calendar; mail is unaffected.
    CalendarReauth,
    /// The account's calendar did not connect while the account itself did.
    CalendarOutage {
        /// Why it did not.
        problem: UseProblem,
    },
    /// The account's address book did not connect while the account itself did.
    ContactsOutage {
        /// Why it did not.
        problem: UseProblem,
    },
    /// The account's server could not be reached while the device is online.
    Unreachable,
    /// The device is offline; what is shown is what was last synced.
    Offline,
    /// The account service refused the Allodia account's sign-in.
    AllodiaSignInEnded {
        /// The Allodia account's address, which is in no mail account list.
        address: String,
    },
    /// Stored accounts that could not be read at launch were left out.
    AccountsSkipped {
        /// How many.
        count: u32,
    },
    /// A sent message reached its recipients, but its copy could not be saved in the account's
    /// Sent folder (`docs/sending.md`). Never a failed send.
    SentNotFiled {
        /// The message's subject, so the sentence names which one.
        subject: String,
    },
    /// An answer to an invitation is stored, but the calendar server could not pass it on to
    /// the organiser, and this app can email it instead (`docs/invitations.md`).
    ReplyNotDelivered {
        /// The meeting's title.
        summary: String,
        /// The address the email would go to, which the sentence names: consent to send mail on
        /// someone's behalf needs the recipient.
        organizer: String,
        /// The answer that was given; the reply's subject is the client's to compose from it.
        response: InvitationResponse,
    },
    /// The device reported a time zone other than the one the calendar is shown in
    /// (`docs/calendar.md`).
    TimeZoneChanged {
        /// The device's new zone, an IANA id.
        zone: String,
    },
    /// The server refused a change to the account's folders.
    FolderChangeRefused {
        /// The folder's name as the person last saw it, or the name asked for on a create.
        folder: String,
        /// What was asked for.
        action: FolderAction,
        /// Why it did not happen.
        problem: FolderProblem,
    },
}

/// How much of what the person relies on a notice touches (`docs/notices.md` rule 4).
///
/// Declared in rank order: a notice of an earlier severity is drawn first.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, uniffi::Enum)]
pub enum NoticeSeverity {
    /// Something the person relies on has stopped. The only severity drawn in the warning
    /// colour.
    Blocked,
    /// Part of it works.
    Degraded,
    /// Nothing is broken.
    Informational,
}

/// Which screens a notice belongs to.
#[derive(Debug, Clone, Copy, PartialEq, Eq, uniffi::Enum)]
pub enum NoticeScreens {
    /// Every screen.
    Everywhere,
    /// Mail only.
    Mail,
    /// The calendar only.
    Calendar,
    /// Contacts only.
    Contacts,
}

/// What a host offers to do about a notice; it runs the flow it already has.
#[derive(Debug, Clone, Copy, PartialEq, Eq, uniffi::Enum)]
pub enum NoticeAction {
    /// Run the account's own browser sign-in again, which also grants what a narrower grant
    /// withheld.
    SignInAgain,
    /// Open the account's page in Settings: where a password is entered again, and where a use
    /// that did not connect says why.
    OpenAccountSettings,
    /// Run the Allodia account's sign-in again.
    SignInToAllodia,
    /// Refresh what the notice's screen shows, which dials the account again.
    Retry,
    /// Ask whether to allow the account's new server, found on another site
    /// (`MailcalApp::account_server_change`, `docs/account-autodetect.md` rule 14).
    Review,
    /// Save the sent message's copy to Sent: `Intent::RetryUnfiledCopy`. Sends nothing.
    FileSentCopy,
    /// Leave the copy missing: `Intent::DismissUnfiledCopy`.
    LeaveSentCopy,
    /// Email the invitation reply from the account: `Intent::AnswerReplyPrompt` with `send`, and
    /// the subject the client composes from the notice's `response`.
    SendReply,
    /// Do not email it: `Intent::AnswerReplyPrompt` without `send`. The answer stays stored.
    DontSendReply,
    /// Show the calendar in the device's new zone: `Intent::AcceptTimeZoneChange`.
    UseDeviceTimeZone,
    /// Keep the zone it is shown in: `Intent::DismissTimeZoneChange`.
    KeepTimeZone,
}
