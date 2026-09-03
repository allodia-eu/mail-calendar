//! Mail-action regressions. GTK functions run inside the mailbox module's single display test.

use adw::prelude::*;
use mailcal_bindings::{FolderRole, FolderRow, Intent};

use super::{
    ActionKind, DeleteTarget, MailActionRequest, MessageTarget, PermanentDeleteDialog, actions_for,
    in_junk_folder, message_menu_button, thread_menu_button,
};
use crate::{
    l10n,
    ui::{AppInput, model},
};

fn target() -> MessageTarget {
    MessageTarget {
        account: "account-a".to_owned(),
        key: "message-a".to_owned(),
    }
}

#[test]
fn a_message_menu_toggles_state_and_names_the_current_spam_direction() {
    assert_eq!(
        actions_for(true, false, false),
        vec![
            ActionKind::MarkRead(true),
            ActionKind::SetFlagged(true),
            ActionKind::Archive,
            ActionKind::MoveToTrash,
            ActionKind::MarkAsSpam,
            ActionKind::PermanentlyDelete,
        ]
    );
    assert_eq!(
        actions_for(false, true, true),
        vec![
            ActionKind::MarkRead(false),
            ActionKind::SetFlagged(false),
            ActionKind::Archive,
            ActionKind::MoveToTrash,
            ActionKind::MarkAsNotSpam,
            ActionKind::PermanentlyDelete,
        ]
    );
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

#[test]
fn only_the_selected_junk_role_reverses_the_spam_action() {
    let mut snapshot = model::empty_mailbox();
    snapshot.selected = Some("folder-a".to_owned());
    snapshot.folders = vec![FolderRow {
        key: "folder-a".to_owned(),
        name: "Anything the server calls it".to_owned(),
        role: Some(FolderRole::Junk),
        unread: 0,
    }];
    assert!(in_junk_folder(&snapshot));

    snapshot.folders[0].role = None;
    assert!(!in_junk_folder(&snapshot));
    snapshot.selected = Some("another-folder".to_owned());
    snapshot.folders[0].role = Some(FolderRole::Junk);
    assert!(!in_junk_folder(&snapshot));
}

pub(crate) fn the_action_menus_dispatch_the_message_and_thread_the_user_chose() {
    let (sender, receiver) = relm4::channel::<AppInput>();
    let row = mailcal_bindings::FlatRow {
        avatar: crate::ui::model::blank_avatar(),
        account: "account-a".to_owned(),
        key: "message-a".to_owned(),
        subject: "Action fixture".to_owned(),
        from: "Sender".to_owned(),
        date: "2026-08-20".to_owned(),
        unread: true,
        flagged: false,
        has_attachment: false,
        preview: String::new(),
    };
    let menu = message_menu_button(&row, false, &sender);
    let menu_root = popover_child(&menu);
    assert!(button(&menu_root, l10n::action_mark_read()).is_some());
    assert!(button(&menu_root, l10n::action_flag()).is_some());
    assert!(button(&menu_root, l10n::action_archive()).is_some());
    assert!(button(&menu_root, l10n::action_move_to_trash()).is_some());
    assert!(button(&menu_root, l10n::action_mark_as_spam()).is_some());
    assert!(button(&menu_root, l10n::action_delete_permanently()).is_some());
    assert!(button(&menu_root, l10n::action_mark_unread()).is_none());
    assert!(button(&menu_root, l10n::action_unflag()).is_none());
    assert!(button(&menu_root, l10n::action_mark_as_not_spam()).is_none());

    button(&menu_root, l10n::action_mark_read())
        .expect("mark-read action")
        .emit_clicked();
    let Some(AppInput::PerformMailAction(request)) = receiver.recv_sync() else {
        panic!("mark-read button did not request a mail action");
    };
    assert_eq!(
        *request,
        MailActionRequest::new(target(), ActionKind::MarkRead(true))
    );

    button(&menu_root, l10n::action_delete_permanently())
        .expect("permanent-delete action")
        .emit_clicked();
    let Some(AppInput::RequestPermanentDelete(request)) = receiver.recv_sync() else {
        panic!("permanent delete bypassed its confirmation request");
    };
    assert_eq!(request, target());

    let junk_menu = message_menu_button(&row, true, &sender);
    let junk_root = popover_child(&junk_menu);
    assert!(button(&junk_root, l10n::action_mark_as_not_spam()).is_some());
    assert!(button(&junk_root, l10n::action_mark_as_spam()).is_none());

    let thread_menu = thread_menu_button("account-a", "thread-a", &sender);
    button(&popover_child(&thread_menu), l10n::thread_archive())
        .expect("archive-conversation action")
        .emit_clicked();
    assert!(matches!(
        receiver.recv_sync(),
        Some(AppInput::ArchiveThread { account, thread_id })
            if account == "account-a" && thread_id == "thread-a"
    ));
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
    parent.close();
}

fn popover_child(menu: &gtk::Box) -> gtk::Widget {
    popover(menu.upcast_ref::<gtk::Widget>())
        .expect("menu popover")
        .child()
        .expect("popover content")
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
