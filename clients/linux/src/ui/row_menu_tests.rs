//! What a row menu's items do, and how a command bar item is run as one.

use mailcal_bindings::{
    BulkAction, Command, CommandBar, CommandItem, CommandSection, MenuMessage, MessageKind,
    RowAction, RowMenu, RowMenuItem, RowMenuSection, SelectedRow,
};

use super::{BarAnswer, RowMenuSource, RowPick, as_menu_item, bar_label, label, pick};
use crate::{
    l10n,
    ui::{
        composer_model::ComposeKind,
        mail_actions::{ActionKind, MailActionRequest, MessageTarget},
    },
};

fn message(kind: MessageKind) -> MenuMessage {
    MenuMessage {
        account: "acct".to_owned(),
        key: "newest".to_owned(),
        kind,
    }
}

fn item(action: RowAction, enabled: bool, target: Option<MenuMessage>) -> RowMenuItem {
    RowMenuItem {
        action,
        enabled,
        target,
        confirm: action == RowAction::PermanentlyDelete,
    }
}

fn conversation() -> Vec<SelectedRow> {
    vec![SelectedRow::Thread {
        account: "acct".to_owned(),
        thread_id: "thread".to_owned(),
    }]
}

#[test]
fn an_item_naming_a_message_acts_on_that_message_alone() {
    let rows = conversation();
    let received = message(MessageKind::Received);
    let target = MessageTarget {
        account: "acct".to_owned(),
        key: "newest".to_owned(),
    };
    let on = |action| pick(&item(action, true, Some(received.clone())), &rows);

    assert_eq!(on(RowAction::Open), Some(RowPick::Open(received.clone())));
    assert_eq!(
        on(RowAction::ReplyAll),
        Some(RowPick::Answer(ComposeKind::ReplyAll, received.clone()))
    );
    assert_eq!(
        on(RowAction::Flag),
        Some(RowPick::Message(MailActionRequest::new(
            target.clone(),
            ActionKind::SetFlagged(true)
        )))
    );
    assert_eq!(
        on(RowAction::MarkAsSpam),
        Some(RowPick::Message(MailActionRequest::new(
            target.clone(),
            ActionKind::MarkAsSpam
        )))
    );
    assert_eq!(
        on(RowAction::Delete),
        Some(RowPick::Message(MailActionRequest::new(
            target.clone(),
            ActionKind::MoveToTrash
        )))
    );
    // Irreversible, so it asks first, about the one message.
    assert_eq!(
        on(RowAction::PermanentlyDelete),
        Some(RowPick::DeleteMessage(target))
    );
}

/// A conversation's move, its read state and its flag's removal reach members the core works
/// out when the write lands, so the item carries no target and the batch names the row.
#[test]
fn an_item_naming_no_message_acts_on_the_rows_the_menu_was_asked_about() {
    let rows = conversation();
    for (action, batch) in [
        (RowAction::MarkRead, BulkAction::MarkRead),
        (RowAction::MarkUnread, BulkAction::MarkUnread),
        (RowAction::Unflag, BulkAction::Unflag),
        (RowAction::Archive, BulkAction::Archive),
        (RowAction::Delete, BulkAction::Delete),
        (RowAction::PermanentlyDelete, BulkAction::PermanentlyDelete),
    ] {
        assert_eq!(
            pick(&item(action, true, None), &rows),
            Some(RowPick::Rows {
                rows: rows.clone(),
                action: batch,
            }),
            "{action:?}"
        );
    }
    assert_eq!(
        pick(&item(RowAction::MoveToFolder, true, None), &rows),
        Some(RowPick::Move)
    );
    // Report as junk with nothing received to report: drawn disabled, and it does nothing.
    assert_eq!(pick(&item(RowAction::MarkAsSpam, false, None), &rows), None);
}

/// A bar item runs as the same item in a row's menu: the one message it names, else the batch
/// over the selection, with the core's `confirm` carried through (`docs/command-bar.md`, rule 1).
#[test]
fn a_bar_item_is_picked_as_the_menu_item_it_stands_for() {
    let rows = conversation();
    let received = message(MessageKind::Received);
    let flag = CommandItem {
        command: Command::Act {
            action: RowAction::Flag,
        },
        enabled: true,
        target: Some(received.clone()),
        confirm: false,
    };
    let as_item = as_menu_item(&flag).expect("an action is a menu item");
    assert_eq!(as_item, item(RowAction::Flag, true, Some(received)));
    assert!(matches!(
        pick(&as_item, &rows),
        Some(RowPick::Message(request)) if request.action == ActionKind::SetFlagged(true)
    ));

    let purge = CommandItem {
        command: Command::Act {
            action: RowAction::PermanentlyDelete,
        },
        enabled: true,
        target: None,
        confirm: true,
    };
    let as_item = as_menu_item(&purge).expect("an action is a menu item");
    assert!(as_item.confirm, "a question first, as the core says");
    assert_eq!(
        pick(&as_item, &rows),
        Some(RowPick::Rows {
            rows,
            action: BulkAction::PermanentlyDelete,
        })
    );

    let sync = CommandItem {
        command: Command::Sync,
        enabled: true,
        target: None,
        confirm: false,
    };
    assert_eq!(as_menu_item(&sync), None, "no row menu carries Sync");
}

/// Delete reads "Delete" everywhere it moves to Trash; only Move is worded for the bar.
#[test]
fn the_bar_words_move_for_a_button_and_delete_as_delete() {
    let act = |action| Command::Act { action };
    assert_eq!(label(RowAction::Delete), l10n::action_delete());
    assert_eq!(bar_label(act(RowAction::Delete)), l10n::action_delete());
    assert_eq!(
        bar_label(act(RowAction::PermanentlyDelete)),
        l10n::action_delete_permanently()
    );
    assert_eq!(
        label(RowAction::MoveToFolder),
        l10n::action_move_to_folder()
    );
    assert_eq!(bar_label(act(RowAction::MoveToFolder)), l10n::action_move());
    assert_eq!(bar_label(Command::Sync), l10n::action_refresh());
}

#[test]
fn a_menu_asked_before_the_core_exists_holds_nothing() {
    let source = RowMenuSource::default();
    assert!(!source.is_answering());
    assert_eq!(source.menu(conversation()), RowMenu::default());

    let answered = RowMenu {
        sections: vec![RowMenuSection {
            items: vec![item(RowAction::Archive, true, None)],
        }],
    };
    let answer = answered.clone();
    source.answer_with(std::rc::Rc::new(move |rows| {
        assert_eq!(rows, conversation(), "asked about the rows it was given");
        answer.clone()
    }));
    assert_eq!(source.menu(conversation()), answered);
}

/// The view runs on every message, and the bar's question is a walk over the whole selection, so
/// it is asked again only for a different selection or a newly published list.
#[test]
fn the_bar_asks_again_only_when_the_selection_or_the_list_changes() {
    let answer = CommandBar {
        sections: vec![CommandSection {
            items: vec![CommandItem {
                command: Command::Sync,
                enabled: true,
                target: None,
                confirm: false,
            }],
        }],
        menu: Vec::new(),
    };
    let asked = std::cell::Cell::new(0);
    let bar = |_: Vec<SelectedRow>| {
        asked.set(asked.get() + 1);
        answer.clone()
    };
    let cache = BarAnswer::default();
    let other = vec![SelectedRow::Message {
        account: "acct".to_owned(),
        key: "other".to_owned(),
    }];

    assert_eq!(cache.answer(1, conversation(), bar), answer);
    assert_eq!(cache.answer(1, conversation(), bar), answer);
    assert_eq!(asked.get(), 1, "an unchanged redraw asks nothing");
    cache.answer(1, other.clone(), bar);
    assert_eq!(asked.get(), 2, "a different selection asks");
    cache.answer(2, other, bar);
    assert_eq!(asked.get(), 3, "a newly published list asks");
}
