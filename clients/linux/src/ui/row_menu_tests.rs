//! What a row menu's items do, and what the selection bar reads off the same answer.

use mailcal_bindings::{
    BulkAction, MenuMessage, MessageKind, RowAction, RowMenu, RowMenuItem, RowMenuSection,
    SelectedRow,
};

use super::{BarActions, RowMenuSource, RowPick, pick};
use crate::ui::{
    composer_model::ComposeKind,
    mail_actions::{ActionKind, MailActionRequest, MessageTarget},
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

#[test]
fn the_bar_reads_its_toggles_and_what_responds_off_the_selections_menu() {
    let menu = RowMenu {
        sections: vec![
            RowMenuSection {
                items: vec![
                    item(RowAction::MarkUnread, true, None),
                    item(RowAction::Unflag, true, None),
                ],
            },
            RowMenuSection {
                items: vec![
                    item(RowAction::MoveToFolder, true, None),
                    item(RowAction::Archive, false, None),
                    item(RowAction::Delete, true, None),
                    item(RowAction::PermanentlyDelete, true, None),
                ],
            },
        ],
    };
    assert_eq!(
        BarActions::of(&menu),
        BarActions {
            read: (BulkAction::MarkUnread, true),
            flag: (BulkAction::Unflag, true),
            archive: false,
            delete: true,
            permanently_delete: true,
        }
    );
}

/// The bar stands whether or not anything is picked. "Mark as unread" over nothing is the wrong
/// sentence to leave on screen: the affirmative action is the one it names until a selection
/// says otherwise, and nothing responds.
#[test]
fn an_empty_menu_names_the_affirmative_actions_and_enables_none() {
    assert_eq!(
        BarActions::of(&RowMenu::default()),
        BarActions {
            read: (BulkAction::MarkRead, false),
            flag: (BulkAction::Flag, false),
            archive: false,
            delete: false,
            permanently_delete: false,
        }
    );
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
