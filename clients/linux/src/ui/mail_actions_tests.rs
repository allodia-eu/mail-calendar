//! Mail-action regressions. GTK functions run inside the mailbox module's single display test.

use std::rc::Rc;

use adw::prelude::*;
use mailcal_bindings::{
    BulkAction, Intent, MenuMessage, MessageKind, RowAction, RowMenu, RowMenuItem, RowMenuSection,
    SelectedRow,
};

use super::{ActionKind, DeleteTarget, MailActionRequest, MessageTarget, PermanentDeleteDialog};
use crate::{
    l10n,
    ui::{
        AppInput,
        mail_actions_menu::{RowMenus, fill},
        row_menu::RowPick,
    },
};

fn target() -> MessageTarget {
    MessageTarget {
        account: "account-a".to_owned(),
        key: "message-a".to_owned(),
    }
}

#[test]
fn every_message_action_maps_to_the_shared_intent_with_its_owner() {
    let cases = [
        ActionKind::MarkRead(true),
        ActionKind::SetFlagged(false),
        ActionKind::Archive,
        ActionKind::MoveToTrash,
        ActionKind::MarkAsSpam,
        ActionKind::MarkAsNotSpam,
        ActionKind::PermanentlyDelete,
    ];
    for action in cases {
        let intent = MailActionRequest::new(target(), action).into_intent();
        match (action, intent) {
            (
                ActionKind::MarkRead(read),
                Intent::MarkRead {
                    account,
                    key,
                    read: actual,
                },
            ) => {
                assert_eq!((account.as_str(), key.as_str()), ("account-a", "message-a"));
                assert_eq!(actual, read);
            }
            (
                ActionKind::SetFlagged(flagged),
                Intent::SetFlagged {
                    account,
                    key,
                    flagged: actual,
                },
            ) => {
                assert_eq!((account.as_str(), key.as_str()), ("account-a", "message-a"));
                assert_eq!(actual, flagged);
            }
            (ActionKind::Archive, Intent::Archive { account, key })
            | (ActionKind::MoveToTrash, Intent::Delete { account, key })
            | (ActionKind::MarkAsSpam, Intent::MarkAsSpam { account, key })
            | (ActionKind::MarkAsNotSpam, Intent::MarkAsNotSpam { account, key })
            | (ActionKind::PermanentlyDelete, Intent::PermanentlyDelete { account, key }) => {
                assert_eq!((account.as_str(), key.as_str()), ("account-a", "message-a"));
            }
            _ => panic!("mail action mapped to the wrong shared intent"),
        }
    }
}

fn item(action: RowAction, enabled: bool, target: Option<MenuMessage>) -> RowMenuItem {
    RowMenuItem {
        action,
        enabled,
        target,
    }
}

/// A menu answering `menu` whatever it is asked about, recording what it was asked.
fn answering(menu: RowMenu) -> (RowMenus, Rc<std::cell::RefCell<Vec<Vec<SelectedRow>>>>) {
    let menus = RowMenus::default();
    let asked = Rc::new(std::cell::RefCell::new(Vec::new()));
    let record = Rc::clone(&asked);
    menus.source.answer_with(Rc::new(move |rows| {
        record.borrow_mut().push(rows);
        menu.clone()
    }));
    (menus, asked)
}

/// The menu draws what the core answers, in its sections and order, disabled where the core says
/// so, and an item reaches the one message it names or else the row it was opened on
/// (`docs/row-actions.md`, rules 1 and 6).
pub(crate) fn a_row_menu_draws_the_cores_answer_and_runs_what_is_picked() {
    let (sender, receiver) = relm4::channel::<AppInput>();
    let draft = MenuMessage {
        account: "account-a".to_owned(),
        key: "draft".to_owned(),
        kind: MessageKind::Draft,
    };
    let inbound = MenuMessage {
        key: "inbound".to_owned(),
        kind: MessageKind::Received,
        ..draft.clone()
    };
    let menu = RowMenu {
        sections: vec![
            RowMenuSection {
                items: vec![
                    item(RowAction::Open, true, Some(draft.clone())),
                    item(RowAction::OpenInWindow, true, Some(draft.clone())),
                ],
            },
            RowMenuSection {
                items: vec![
                    item(RowAction::Reply, false, Some(draft.clone())),
                    item(RowAction::ReplyAll, false, Some(draft.clone())),
                    item(RowAction::Forward, false, Some(draft.clone())),
                ],
            },
            RowMenuSection {
                items: vec![
                    item(RowAction::MarkRead, true, None),
                    item(RowAction::Flag, true, Some(inbound.clone())),
                ],
            },
            RowMenuSection {
                items: vec![item(RowAction::MarkAsSpam, true, Some(inbound.clone()))],
            },
            RowMenuSection {
                items: vec![
                    item(RowAction::MoveToFolder, true, None),
                    item(RowAction::Archive, true, None),
                    item(RowAction::Delete, false, None),
                    item(RowAction::PermanentlyDelete, true, None),
                ],
            },
        ],
    };
    let (menus, asked) = answering(menu);
    let conversation = SelectedRow::Thread {
        account: "account-a".to_owned(),
        thread_id: "thread-a".to_owned(),
    };
    let content = gtk::Box::new(gtk::Orientation::Vertical, 0);
    fill(&content, &conversation, &menus, &sender);
    assert_eq!(*asked.borrow(), vec![vec![conversation.clone()]]);

    // The sections in order, a separator between each two.
    let drawn: Vec<String> = children(&content)
        .iter()
        .map(|child| match child.downcast_ref::<gtk::Button>() {
            Some(button) => button.label().map(String::from).unwrap_or_default(),
            None => "—".to_owned(),
        })
        .collect();
    assert_eq!(
        drawn,
        [
            l10n::action_open(),
            l10n::action_open_in_window(),
            "—",
            l10n::action_reply(),
            l10n::action_reply_all(),
            l10n::action_forward(),
            "—",
            l10n::action_mark_read(),
            l10n::action_flag(),
            "—",
            l10n::action_mark_as_spam(),
            "—",
            l10n::action_move_to_folder(),
            l10n::action_archive(),
            l10n::action_move_to_trash(),
            l10n::action_delete_permanently(),
        ]
    );
    let root = content.upcast_ref::<gtk::Widget>();
    let pressed = |label: &str| {
        let button = button(root, label).unwrap_or_else(|| panic!("no {label:?} item"));
        assert!(button.is_sensitive(), "{label:?} responds");
        button.emit_clicked();
        match receiver.recv_sync() {
            Some(AppInput::RowMenuPick(pick)) => *pick,
            other => panic!("{label:?} sent {other:?}"),
        }
    };

    // Disabled, but still there: the menu keeps its shape from one row to the next.
    for label in [l10n::action_reply(), l10n::action_move_to_trash()] {
        assert!(
            !button(root, label).expect("drawn").is_sensitive(),
            "{label}"
        );
    }
    // Nothing to move to: the menu was opened outside any folder tree.
    assert!(
        !button(root, l10n::action_move_to_folder())
            .expect("drawn")
            .is_sensitive()
    );

    assert_eq!(pressed(l10n::action_open()), RowPick::Open(draft.clone()));
    assert_eq!(
        pressed(l10n::action_open_in_window()),
        RowPick::OpenInWindow(draft)
    );
    assert_eq!(
        pressed(l10n::action_flag()),
        RowPick::Message(MailActionRequest::new(
            MessageTarget {
                account: "account-a".to_owned(),
                key: "inbound".to_owned(),
            },
            ActionKind::SetFlagged(true),
        ))
    );
    assert_eq!(
        pressed(l10n::action_mark_read()),
        RowPick::Rows {
            rows: vec![conversation.clone()],
            action: BulkAction::MarkRead,
        }
    );
    assert_eq!(
        pressed(l10n::action_delete_permanently()),
        RowPick::Rows {
            rows: vec![conversation.clone()],
            action: BulkAction::PermanentlyDelete,
        }
    );
    assert!(
        button(root, l10n::action_delete_permanently())
            .expect("drawn")
            .has_css_class("destructive-action")
    );

    // Asked again as it opens again, so a row kept across renders never shows a stale menu.
    fill(&content, &conversation, &menus, &sender);
    assert_eq!(asked.borrow().len(), 2);
    assert_eq!(children(&content).len(), drawn.len());
}

fn children(root: &gtk::Box) -> Vec<gtk::Widget> {
    let mut found = Vec::new();
    let mut child = root.first_child();
    while let Some(widget) = child {
        child = widget.next_sibling();
        found.push(widget);
    }
    found
}

pub(crate) fn permanent_delete_is_confirmed_before_it_dispatches() {
    let parent = gtk::Window::new();
    parent.present();
    let (sender, receiver) = relm4::channel::<AppInput>();
    let mut confirmation = PermanentDeleteDialog::default();
    confirmation.render(Some(&DeleteTarget::Message(target())), &parent, &sender);
    let window = confirmation.window.as_ref().expect("confirmation window");
    assert!(
        labels(window.upcast_ref::<gtk::Widget>())
            .iter()
            .any(|label| label == l10n::delete_permanently_message())
    );
    button(
        window.upcast_ref::<gtk::Widget>(),
        l10n::action_delete_permanently(),
    )
    .expect("confirmed delete action")
    .emit_clicked();
    let Some(AppInput::PerformMailAction(request)) = receiver.recv_sync() else {
        panic!("confirmed delete did not request a mail action");
    };
    assert_eq!(
        *request,
        MailActionRequest::new(target(), ActionKind::PermanentlyDelete)
    );

    // A conversation's menu, outside the selection: the rows it was opened on, by name.
    let rows = vec![SelectedRow::Thread {
        account: "account-a".to_owned(),
        thread_id: "thread-a".to_owned(),
    }];
    let (sender, receiver) = relm4::channel::<AppInput>();
    let mut confirmation = PermanentDeleteDialog::default();
    confirmation.render(Some(&DeleteTarget::Rows(rows.clone())), &parent, &sender);
    let window = confirmation.window.as_ref().expect("confirmation window");
    button(
        window.upcast_ref::<gtk::Widget>(),
        l10n::action_delete_permanently(),
    )
    .expect("confirmed delete action")
    .emit_clicked();
    assert!(matches!(
        receiver.recv_sync(),
        Some(AppInput::PerformRowsAction { rows: asked, action: BulkAction::PermanentlyDelete })
            if asked == rows
    ));
    parent.close();
}

/// What the popover under `root` holds, without popping it up.
///
/// A popover only enters the widget tree of a *realised* toplevel, and these tests build their
/// widgets outside one, so asking it to `popup()` crashes inside GDK. Its child is reachable
/// either way, which is where everything worth asserting on lives.
pub(crate) fn popover_content(root: &gtk::Widget) -> Option<gtk::Widget> {
    popover(root)?.child()
}

fn popover(root: &gtk::Widget) -> Option<gtk::Popover> {
    if let Some(popover) = root.downcast_ref::<gtk::Popover>() {
        return Some(popover.clone());
    }
    let mut child = root.first_child();
    while let Some(node) = child {
        if let Some(found) = popover(&node) {
            return Some(found);
        }
        child = node.next_sibling();
    }
    None
}

pub(crate) fn button(root: &gtk::Widget, label: &str) -> Option<gtk::Button> {
    if let Some(button) = root.downcast_ref::<gtk::Button>()
        && button.label().as_deref() == Some(label)
    {
        return Some(button.clone());
    }
    let mut child = root.first_child();
    while let Some(node) = child {
        if let Some(found) = button(&node, label) {
            return Some(found);
        }
        child = node.next_sibling();
    }
    None
}

pub(crate) fn labels(root: &gtk::Widget) -> Vec<String> {
    let mut found = Vec::new();
    if let Some(label) = root.downcast_ref::<gtk::Label>() {
        found.push(label.text().to_string());
    }
    let mut child = root.first_child();
    while let Some(node) = child {
        found.extend(labels(&node));
        child = node.next_sibling();
    }
    found
}
