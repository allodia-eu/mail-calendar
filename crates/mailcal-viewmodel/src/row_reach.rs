//! Which messages an action on a mailbox-list row reaches (`docs/row-actions.md`).
//!
//! A conversation row stands for several messages, and an action on it reaches some of them:
//! Reply answers the newest, Flag marks the newest one received, marking read touches only what
//! was received. The table lives here, once, because two callers must agree on it: the row menu,
//! which reads it off the snapshot to label and enable its items, and the batch write, which
//! reads it off the store's thread index when the action lands. A message row reaches itself.

use engine_api::{Keyword, MailListRow, SystemKeyword};

/// What a message is to the account holding it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MessageKind {
    /// Somebody else's message to this account.
    Received,
    /// The account owner's own message, sent.
    Sent,
    /// The account owner's own message, written and not sent.
    Draft,
}

impl MessageKind {
    /// The kind of a stored message: a draft by the engine's draft keyword, which every
    /// transport's own tell is normalised onto; otherwise sent when `outgoing` (its `From` is
    /// the owner's address, which only the app knows), else received.
    #[must_use]
    pub fn of(row: &MailListRow, outgoing: bool) -> Self {
        if row
            .keywords
            .contains(&Keyword::system(SystemKeyword::Draft))
        {
            Self::Draft
        } else if outgoing {
            Self::Sent
        } else {
            Self::Received
        }
    }
}

/// One action a row menu offers, listed in the order every menu lists them.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RowAction {
    /// Open the message in the reading pane; a draft resumes in a composer.
    Open,
    /// Open the message in a reading window of its own (desktops only).
    OpenInWindow,
    /// Reply to the sender.
    Reply,
    /// Reply to everyone on the message.
    ReplyAll,
    /// Forward the message.
    Forward,
    /// Mark read.
    MarkRead,
    /// Mark unread.
    MarkUnread,
    /// Flag.
    Flag,
    /// Clear the flag.
    Unflag,
    /// Report as junk, which files it under Junk.
    MarkAsSpam,
    /// Report as not junk, which files it back in the Inbox.
    MarkAsNotSpam,
    /// Move to a folder the user picks.
    MoveToFolder,
    /// Move to the account's Archive.
    Archive,
    /// Move to the account's Trash.
    Delete,
    /// Delete for good.
    PermanentlyDelete,
}

/// One message of a conversation, as much of it as deciding an action's reach needs.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Member {
    /// What the message is to its account.
    pub kind: MessageKind,
    /// Whether it is unread.
    pub unread: bool,
    /// Whether it is flagged.
    pub flagged: bool,
}

/// The positions in `members` (a conversation, **newest first**) that a read or flag action on
/// the whole conversation reaches.
///
/// - **Mark read / unread** reach every received message. The owner's own messages carry no read
///   state anyone asked about, and marking a Sent copy unread would bold a row for mail nobody has
///   to read.
/// - **Flag** reaches the newest received message, the one a follow-up is about; a conversation
///   holding nothing received flags its newest message.
/// - **Unflag** reaches every flagged message, so nothing stays flagged behind a collapsed row and
///   the row's label turns back to Flag.
///
/// Any other action is not a keyword edit and reaches every message; what a *move* leaves
/// behind is [`stays_put`]'s.
#[must_use]
pub fn conversation_reach(action: RowAction, members: &[Member]) -> Vec<usize> {
    let positions = 0..members.len();
    match action {
        RowAction::MarkRead | RowAction::MarkUnread => positions
            .filter(|&at| members[at].kind == MessageKind::Received)
            .collect(),
        RowAction::Flag => flag_target(members).into_iter().collect(),
        RowAction::Unflag => positions.filter(|&at| members[at].flagged).collect(),
        _ => positions.collect(),
    }
}

/// The message Flag marks on a conversation (newest first): its newest received message, else
/// its newest message.
#[must_use]
pub fn flag_target(members: &[Member]) -> Option<usize> {
    members
        .iter()
        .position(|member| member.kind == MessageKind::Received)
        .or_else(|| (!members.is_empty()).then_some(0))
}

/// Whether a conversation's move leaves this member where it is.
///
/// Two kinds of message never leave with the conversation. A copy filed in Sent stays in Sent,
/// so reopening the conversation from wherever it went still shows both sides; and a draft stays
/// a draft where its composer will look for it, so deleting a conversation never throws away
/// unfinished writing. `kept` holds the keys of the account's Sent folders.
///
/// Only a **conversation's** members are judged here. A message row the user picked out by
/// itself moves wherever they sent it: they named that one message.
#[must_use]
pub fn stays_put<'a>(
    kind: MessageKind,
    folders: impl IntoIterator<Item = &'a str>,
    kept: &[String],
) -> bool {
    kind == MessageKind::Draft
        || folders
            .into_iter()
            .any(|folder| kept.iter().any(|kept| kept == folder))
}

/// Where one account's moves land, and which of its folders a conversation's move leaves alone.
///
/// Resolved by the app with the same rule the write itself uses (RFC 6154 role, then the
/// conventional name), so a menu item the snapshot enables is one the write will act on. Not a
/// client concern, and not carried across the FFI.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct AccountPlaces {
    /// The account these folders belong to.
    pub account: String,
    /// The Inbox's key, where not-junk files a message.
    pub inbox: Option<String>,
    /// The Archive's key.
    pub archive: Option<String>,
    /// The Trash's key.
    pub trash: Option<String>,
    /// The Junk folder's key.
    pub junk: Option<String>,
    /// The Sent folders' keys: a conversation's move takes nothing out of them.
    pub kept: Vec<String>,
}
