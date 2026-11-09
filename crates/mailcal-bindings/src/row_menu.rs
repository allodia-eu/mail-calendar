//! The menu a mailbox-list row opens (`docs/row-actions.md`): asked for when the menu opens,
//! drawn as answered.
//!
//! A client renders the sections in order with a separator between them, draws each item under
//! its action's label and icon, greys out what is not enabled, and on a pick dispatches the
//! single-message form for an item with a `target` and the batch over the rows it asked about
//! for one without. It decides nothing else: which message a conversation's item is about, and
//! whether a move would take anything, are the core's.

use mailcal_app::RowRef;
use mailcal_viewmodel::{
    MenuMessage as AppMenuMessage, MessageKind as AppMessageKind, RowAction as AppRowAction,
    RowMenu as AppRowMenu, RowMenuItem as AppRowMenuItem,
};

use crate::{MailcalApp, SelectedRow};

/// What a message is to the account holding it.
#[derive(Clone, Copy, Debug, PartialEq, Eq, uniffi::Enum)]
pub enum MessageKind {
    /// Somebody else's message to this account.
    Received,
    /// The account owner's own message, sent.
    Sent,
    /// The account owner's own message, written and not sent: it opens in a composer.
    Draft,
}

/// One action a row menu offers. Each has one label and one icon on every platform.
#[derive(Clone, Copy, Debug, PartialEq, Eq, uniffi::Enum)]
pub enum RowAction {
    /// Open in the reading pane; a draft resumes in a composer.
    Open,
    /// Open in a reading window of its own (desktops only).
    OpenInWindow,
    /// Reply to the sender.
    Reply,
    /// Reply to everyone.
    ReplyAll,
    /// Forward.
    Forward,
    /// Mark read.
    MarkRead,
    /// Mark unread.
    MarkUnread,
    /// Flag.
    Flag,
    /// Clear the flag.
    Unflag,
    /// Report as junk.
    MarkAsSpam,
    /// Report as not junk.
    MarkAsNotSpam,
    /// Move to a folder the user picks.
    MoveToFolder,
    /// Archive.
    Archive,
    /// Move to Trash.
    Delete,
    /// Delete for good. Offered in Delete's place, never beside it, once everything it takes is
    /// in Trash already or the account has no Trash; its item's `confirm` is set.
    PermanentlyDelete,
}

/// The one message an item is about.
#[derive(Clone, Debug, PartialEq, Eq, uniffi::Record)]
pub struct MenuMessage {
    /// The account holding it.
    pub account: String,
    /// Its provider key.
    pub key: String,
    /// What it is, which decides how it opens.
    pub kind: MessageKind,
}

/// One item of a row menu.
#[derive(Clone, Debug, PartialEq, Eq, uniffi::Record)]
pub struct RowMenuItem {
    /// What the item does.
    pub action: RowAction,
    /// Whether it responds; a disabled item is still drawn.
    pub enabled: bool,
    /// The one message the item acts on, through the single-message intent or composer. `None`:
    /// it acts on the rows the menu was asked about, through `Intent::ActOnSelection` (or
    /// `FolderIntent::MoveMessages` for Move to folder…).
    pub target: Option<MenuMessage>,
    /// Whether a client asks before acting: set for what cannot be undone, and only for that.
    pub confirm: bool,
}

/// One section of a row menu: the items between two separators.
#[derive(Clone, Debug, PartialEq, Eq, uniffi::Record)]
pub struct RowMenuSection {
    /// The items, in order; never empty.
    pub items: Vec<RowMenuItem>,
}

/// A row menu. Empty when none of the rows is still listed.
#[derive(Clone, Debug, Default, PartialEq, Eq, uniffi::Record)]
pub struct RowMenu {
    /// The sections, in order.
    pub sections: Vec<RowMenuSection>,
}

#[uniffi::export]
impl MailcalApp {
    /// The menu over `rows`: the row under the pointer, or the selection it belongs to
    /// (`docs/list-selection.md`, rule 12). Answered from the current mailbox snapshot without
    /// waiting on anything, so a client calls it as the menu opens.
    pub fn row_menu(&self, rows: Vec<SelectedRow>) -> RowMenu {
        let rows: Vec<RowRef> = rows.into_iter().filter_map(row_ref).collect();
        self.app.row_menu(&rows).into()
    }

    /// The message the reading pane opens once `account`'s `key` leaves the list: the next one
    /// down, else the one above, else none (`docs/reading-actions.md`, "Where the pane goes next").
    /// It is the message clicking that row opens, so a client opens it exactly as a click would: a
    /// `Draft` resumes in a composer. `expanded` names the conversation rows the list shows
    /// expanded, as `SelectedRow::Thread`; a message row there is ignored. Asked before the archive
    /// or delete is dispatched, while `key` is still listed.
    pub fn stop_after_removing(
        &self,
        account: String,
        key: String,
        expanded: Vec<SelectedRow>,
    ) -> Option<MenuMessage> {
        let removed = mailcal_app::MessageRef::from_parts(&account, key)?;
        let expanded: Vec<mailcal_app::ThreadRef> = expanded
            .into_iter()
            .filter_map(|row| match row_ref(row)? {
                RowRef::Thread(thread) => Some(thread),
                RowRef::Message(_) => None,
            })
            .collect();
        self.app
            .stop_after_removing(&removed, &expanded)
            .map(MenuMessage::from)
    }
}

/// A selected row as the core names it; a malformed one is left out of the menu.
pub(crate) fn row_ref(row: SelectedRow) -> Option<RowRef> {
    match row {
        SelectedRow::Message { account, key } => {
            mailcal_app::MessageRef::from_parts(&account, key).map(RowRef::Message)
        }
        SelectedRow::Thread { account, thread_id } => {
            mailcal_app::ThreadRef::from_parts(&account, thread_id).map(RowRef::Thread)
        }
    }
}

impl From<AppMessageKind> for MessageKind {
    fn from(kind: AppMessageKind) -> Self {
        match kind {
            AppMessageKind::Received => Self::Received,
            AppMessageKind::Sent => Self::Sent,
            AppMessageKind::Draft => Self::Draft,
        }
    }
}

impl From<AppRowAction> for RowAction {
    fn from(action: AppRowAction) -> Self {
        match action {
            AppRowAction::Open => Self::Open,
            AppRowAction::OpenInWindow => Self::OpenInWindow,
            AppRowAction::Reply => Self::Reply,
            AppRowAction::ReplyAll => Self::ReplyAll,
            AppRowAction::Forward => Self::Forward,
            AppRowAction::MarkRead => Self::MarkRead,
            AppRowAction::MarkUnread => Self::MarkUnread,
            AppRowAction::Flag => Self::Flag,
            AppRowAction::Unflag => Self::Unflag,
            AppRowAction::MarkAsSpam => Self::MarkAsSpam,
            AppRowAction::MarkAsNotSpam => Self::MarkAsNotSpam,
            AppRowAction::MoveToFolder => Self::MoveToFolder,
            AppRowAction::Archive => Self::Archive,
            AppRowAction::Delete => Self::Delete,
            AppRowAction::PermanentlyDelete => Self::PermanentlyDelete,
        }
    }
}

/// Whether `action` is asked about first: it cannot be undone.
pub(crate) const fn asks_first(action: AppRowAction) -> bool {
    matches!(action, AppRowAction::PermanentlyDelete)
}

impl From<AppMenuMessage> for MenuMessage {
    fn from(message: AppMenuMessage) -> Self {
        Self {
            account: message.account,
            key: message.key,
            kind: message.kind.into(),
        }
    }
}

impl From<AppRowMenuItem> for RowMenuItem {
    fn from(item: AppRowMenuItem) -> Self {
        Self {
            action: item.action.into(),
            enabled: item.enabled,
            target: item.target.map(MenuMessage::from),
            confirm: asks_first(item.action),
        }
    }
}

impl From<AppRowMenu> for RowMenu {
    fn from(menu: AppRowMenu) -> Self {
        Self {
            sections: menu
                .sections
                .into_iter()
                .map(|items| RowMenuSection {
                    items: items.into_iter().map(RowMenuItem::from).collect(),
                })
                .collect(),
        }
    }
}

#[cfg(test)]
#[path = "row_menu_tests.rs"]
mod tests;
