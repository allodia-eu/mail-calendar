//! A list row's menu as the core answers it (`docs/row-actions.md`): which items, in which
//! sections, which respond, and which one message an item is about. This file gives each action
//! its words and says what picking it does; it keeps no rule of its own about either, and is
//! tested without a display.

use std::{cell::RefCell, rc::Rc};

use mailcal_bindings::{BulkAction, MenuMessage, RowAction, RowMenu, RowMenuItem, SelectedRow};

use super::{
    composer_model::ComposeKind,
    mail_actions::{ActionKind, MailActionRequest, MessageTarget},
};
use crate::l10n;

/// How a menu asks the core what it holds.
pub(crate) type MenuAnswer = Rc<dyn Fn(Vec<SelectedRow>) -> RowMenu>;

/// Where every row's menu asks, as it opens, for what it holds. Shared by the rows and set once
/// the core exists; until then a menu holds nothing.
#[derive(Clone, Default)]
pub(crate) struct RowMenuSource(Rc<RefCell<Option<MenuAnswer>>>);

impl RowMenuSource {
    pub(crate) fn answer_with(&self, answer: MenuAnswer) {
        self.0.replace(Some(answer));
    }

    pub(crate) fn is_answering(&self) -> bool {
        self.0.borrow().is_some()
    }

    /// The menu over `rows`, empty while no core is answering.
    pub(crate) fn menu(&self, rows: Vec<SelectedRow>) -> RowMenu {
        let answer = self.0.borrow().clone();
        answer.map_or_else(RowMenu::default, |answer| answer(rows))
    }
}

/// What picking a menu item does.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum RowPick {
    /// Open the one message, resuming it when it is a draft.
    Open(MenuMessage),
    OpenInWindow(MenuMessage),
    /// Reply, reply all or forward the one message.
    Answer(ComposeKind, MenuMessage),
    /// A write on the one message.
    Message(MailActionRequest),
    /// Delete the one message for good, once the user confirms.
    DeleteMessage(MessageTarget),
    /// The batch over the rows the menu was asked about.
    Rows {
        rows: Vec<SelectedRow>,
        action: BulkAction,
    },
    /// Ask for a folder, then move the rows the menu was asked about.
    Move,
}

/// What picking `item` does, on the one message it names or else on `rows`. `None` for an item
/// that names no message and has no batch form, which the core never enables.
pub(crate) fn pick(item: &RowMenuItem, rows: &[SelectedRow]) -> Option<RowPick> {
    if item.action == RowAction::MoveToFolder {
        return Some(RowPick::Move);
    }
    let Some(message) = item.target.clone() else {
        return batch(item.action).map(|action| RowPick::Rows {
            rows: rows.to_vec(),
            action,
        });
    };
    let target = MessageTarget {
        account: message.account.clone(),
        key: message.key.clone(),
    };
    let write = |action| {
        Some(RowPick::Message(MailActionRequest::new(
            target.clone(),
            action,
        )))
    };
    match item.action {
        RowAction::Open => Some(RowPick::Open(message)),
        RowAction::OpenInWindow => Some(RowPick::OpenInWindow(message)),
        RowAction::Reply => Some(RowPick::Answer(ComposeKind::Reply, message)),
        RowAction::ReplyAll => Some(RowPick::Answer(ComposeKind::ReplyAll, message)),
        RowAction::Forward => Some(RowPick::Answer(ComposeKind::Forward, message)),
        RowAction::MarkRead => write(ActionKind::MarkRead(true)),
        RowAction::MarkUnread => write(ActionKind::MarkRead(false)),
        RowAction::Flag => write(ActionKind::SetFlagged(true)),
        RowAction::Unflag => write(ActionKind::SetFlagged(false)),
        RowAction::MarkAsSpam => write(ActionKind::MarkAsSpam),
        RowAction::MarkAsNotSpam => write(ActionKind::MarkAsNotSpam),
        RowAction::Archive => write(ActionKind::Archive),
        RowAction::Delete => write(ActionKind::MoveToTrash),
        RowAction::PermanentlyDelete => Some(RowPick::DeleteMessage(target)),
        RowAction::MoveToFolder => Some(RowPick::Move),
    }
}

/// The batch an item without a target runs, or `None` for an action that always names one
/// message.
fn batch(action: RowAction) -> Option<BulkAction> {
    match action {
        RowAction::MarkRead => Some(BulkAction::MarkRead),
        RowAction::MarkUnread => Some(BulkAction::MarkUnread),
        RowAction::Flag => Some(BulkAction::Flag),
        RowAction::Unflag => Some(BulkAction::Unflag),
        RowAction::Archive => Some(BulkAction::Archive),
        RowAction::Delete => Some(BulkAction::Delete),
        RowAction::PermanentlyDelete => Some(BulkAction::PermanentlyDelete),
        RowAction::Open
        | RowAction::OpenInWindow
        | RowAction::Reply
        | RowAction::ReplyAll
        | RowAction::Forward
        | RowAction::MarkAsSpam
        | RowAction::MarkAsNotSpam
        | RowAction::MoveToFolder => None,
    }
}

/// The item's label, in the user's language: the selection bar's words for the actions the two
/// share.
pub(crate) fn label(action: RowAction) -> &'static str {
    match action {
        RowAction::Open => l10n::action_open(),
        RowAction::OpenInWindow => l10n::action_open_in_window(),
        RowAction::Reply => l10n::action_reply(),
        RowAction::ReplyAll => l10n::action_reply_all(),
        RowAction::Forward => l10n::action_forward(),
        RowAction::MarkRead => l10n::action_mark_read(),
        RowAction::MarkUnread => l10n::action_mark_unread(),
        RowAction::Flag => l10n::action_flag(),
        RowAction::Unflag => l10n::action_unflag(),
        RowAction::MarkAsSpam => l10n::action_mark_as_spam(),
        RowAction::MarkAsNotSpam => l10n::action_mark_as_not_spam(),
        RowAction::MoveToFolder => l10n::action_move_to_folder(),
        RowAction::Archive => l10n::action_archive(),
        RowAction::Delete => l10n::action_move_to_trash(),
        RowAction::PermanentlyDelete => l10n::action_delete_permanently(),
    }
}

/// The selection bar's buttons, read off the core's menu over the selection
/// (`docs/list-selection.md`, rule 5): each toggle names the half that changes something, and a
/// button the menu would draw disabled is disabled on the bar too.
///
/// With nothing selected the core answers no menu, and the bar names the affirmative halves, all
/// disabled: "Mark as unread" over nothing describes an action nobody asked for.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct BarActions {
    pub(crate) read: (BulkAction, bool),
    pub(crate) flag: (BulkAction, bool),
    pub(crate) archive: bool,
    pub(crate) delete: bool,
    pub(crate) permanently_delete: bool,
}

impl BarActions {
    pub(crate) fn of(menu: &RowMenu) -> Self {
        // The first item doing one of `actions`, which for a toggle is the half on offer.
        let find = |actions: &[RowAction]| {
            menu.sections
                .iter()
                .flat_map(|section| &section.items)
                .find(|item| actions.contains(&item.action))
                .map_or((actions[0], false), |item| (item.action, item.enabled))
        };
        let toggle = |actions: &[RowAction]| {
            let (action, enabled) = find(actions);
            (batch(action).unwrap_or(BulkAction::MarkRead), enabled)
        };
        Self {
            read: toggle(&[RowAction::MarkRead, RowAction::MarkUnread]),
            flag: toggle(&[RowAction::Flag, RowAction::Unflag]),
            archive: find(&[RowAction::Archive]).1,
            delete: find(&[RowAction::Delete]).1,
            permanently_delete: find(&[RowAction::PermanentlyDelete]).1,
        }
    }
}

#[cfg(test)]
#[path = "row_menu_tests.rs"]
mod tests;
