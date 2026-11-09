//! A list row's menu as the core answers it (`docs/row-actions.md`): which items, in which
//! sections, which respond, and which one message an item is about. This file gives each action
//! its words and says what picking it does; it keeps no rule of its own about either, and is
//! tested without a display.

use std::{cell::RefCell, rc::Rc};

use mailcal_bindings::{
    BulkAction, Command, CommandBar, CommandItem, MenuMessage, RowAction, RowMenu, RowMenuItem,
    SelectedRow,
};

use super::{
    composer_model::ComposeKind,
    mail_actions::{ActionKind, MailActionRequest, MessageTarget},
};
use crate::{l10n, ui::icons};

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

/// The item's label in a row's menu, and in the reading pane, in the user's language.
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
        RowAction::Delete => l10n::action_delete(),
        RowAction::PermanentlyDelete => l10n::action_delete_permanently(),
    }
}

/// The item's label on the command bar: the menu's, except that Move opens its picker from a
/// button too narrow for the menu's "Move to folder…".
pub(crate) fn bar_label(command: Command) -> &'static str {
    match command {
        Command::Act {
            action: RowAction::MoveToFolder,
        } => l10n::action_move(),
        Command::Act { action } => label(action),
        Command::Sync => l10n::action_refresh(),
    }
}

/// The glyph a bar item draws beside its label.
pub(crate) fn bar_icon(command: Command) -> &'static str {
    let Command::Act { action } = command else {
        return icons::SYNC;
    };
    match action {
        RowAction::MarkRead => icons::MARK_READ,
        RowAction::MarkUnread => icons::MARK_UNREAD,
        RowAction::Flag => icons::FLAG,
        RowAction::Unflag => icons::UNFLAG,
        RowAction::Archive => icons::ARCHIVE,
        RowAction::Delete => icons::TRASH,
        // Its own glyph rather than a second bin: the label is not the only thing that should
        // say this one cannot be undone.
        RowAction::PermanentlyDelete => icons::DELETE_PERMANENTLY,
        RowAction::MoveToFolder => icons::FOLDER,
        RowAction::MarkAsSpam | RowAction::MarkAsNotSpam => icons::JUNK,
        RowAction::Open | RowAction::OpenInWindow => icons::MAIL,
        RowAction::Reply => icons::REPLY,
        RowAction::ReplyAll => icons::REPLY_ALL,
        RowAction::Forward => icons::FORWARD,
    }
}

/// The row menu's item a bar item stands for, so a pick from the bar runs exactly as the same
/// pick from a row's menu does (`docs/command-bar.md`, rule 1). `None` for Sync, which no menu
/// carries.
pub(crate) fn as_menu_item(item: &CommandItem) -> Option<RowMenuItem> {
    match item.command {
        Command::Act { action } => Some(RowMenuItem {
            action,
            enabled: item.enabled,
            target: item.target.clone(),
            confirm: item.confirm,
        }),
        Command::Sync => None,
    }
}

/// The command bar over the rows it was last asked about, with what it was asked about: the
/// published list, by its number, and the rows. A redraw that changed neither asks the core
/// nothing, since the view runs on every message and Select all can name the whole window.
#[derive(Default)]
pub(crate) struct BarAnswer(RefCell<Option<(u64, Vec<SelectedRow>, CommandBar)>>);

impl BarAnswer {
    /// The bar over `rows` in list `generation`, asking `bar` only when either differs from what
    /// was last asked.
    pub(crate) fn answer(
        &self,
        generation: u64,
        rows: Vec<SelectedRow>,
        bar: impl FnOnce(Vec<SelectedRow>) -> CommandBar,
    ) -> CommandBar {
        if let Some((asked_in, asked, answer)) = &*self.0.borrow()
            && *asked_in == generation
            && *asked == rows
        {
            return answer.clone();
        }
        let answer = bar(rows.clone());
        self.0.replace(Some((generation, rows, answer.clone())));
        answer
    }
}

#[cfg(test)]
#[path = "row_menu_tests.rs"]
mod tests;
