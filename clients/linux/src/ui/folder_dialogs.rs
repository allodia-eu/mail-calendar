//! The three dialogs a folder's row menu opens: a name (new or renamed folder), a destination
//! (Move to…), and the delete confirmation (`docs/folder-pane.md`, rules 24 to 26).
//!
//! Each is a transient modal over the window the row sits in, and each answers with one
//! `FolderIntent`; none keeps state after it closes.

use adw::prelude::*;
use mailcal_bindings::{FolderIntent, FolderNameCheck};

use super::{
    AppInput,
    folder_actions::{FolderInput, MoveCandidate, NameCheck, NameQuery, delete_copy, name_problem},
    modal,
};
use crate::l10n;

/// What a name dialog is for.
pub(crate) enum NamePurpose {
    /// A new folder inside `parent`, or at the top of the account's tree.
    Create { parent: Option<String> },
    /// A new name for `key`, which sits inside `parent`.
    Rename {
        key: String,
        parent: Option<String>,
        current: String,
    },
}

fn window_of(widget: &impl IsA<gtk::Widget>) -> Option<gtk::Window> {
    widget.root().and_downcast::<gtk::Window>()
}

fn content_box() -> gtk::Box {
    let content = gtk::Box::new(gtk::Orientation::Vertical, 12);
    content.set_margin_top(24);
    content.set_margin_bottom(24);
    content.set_margin_start(24);
    content.set_margin_end(24);
    content
}

/// Cancel and a confirm button, trailing. The confirm is returned so the caller can wire it.
fn action_bar(window: &gtk::Window, confirm: &str, destructive: bool) -> (gtk::Box, gtk::Button) {
    let actions = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    actions.set_halign(gtk::Align::End);
    let cancel = gtk::Button::with_label(l10n::action_cancel());
    let dialog = window.clone();
    cancel.connect_clicked(move |_| dialog.close());
    actions.append(&cancel);
    let confirm = gtk::Button::with_label(confirm);
    confirm.add_css_class(if destructive {
        "destructive-action"
    } else {
        "suggested-action"
    });
    actions.append(&confirm);
    (actions, confirm)
}

/// Asks for a folder name, checking it with the core as it is typed (rule 25).
pub(crate) fn name_dialog(
    anchor: &impl IsA<gtk::Widget>,
    account: &str,
    purpose: NamePurpose,
    check: &NameCheck,
    sender: &relm4::Sender<AppInput>,
) {
    let Some(parent_window) = window_of(anchor) else {
        return;
    };
    let (title, confirm_label, initial) = match &purpose {
        NamePurpose::Create { .. } => (l10n::folder_new_title(), l10n::action_create(), ""),
        NamePurpose::Rename { current, .. } => (
            l10n::folder_rename_title(),
            l10n::action_save(),
            current.as_str(),
        ),
    };
    let (window, _) = modal::new(&parent_window, title, 400, None);
    window.set_resizable(false);
    let content = content_box();
    let entry = gtk::Entry::new();
    entry.set_text(initial);
    entry.set_placeholder_text(Some(l10n::folder_name_label()));
    entry.update_property(&[gtk::accessible::Property::Label(l10n::folder_name_label())]);
    content.append(&entry);
    let problem = gtk::Label::new(None);
    problem.set_xalign(0.0);
    problem.set_wrap(true);
    problem.add_css_class("error");
    problem.set_visible(false);
    content.append(&problem);
    let (actions, confirm) = action_bar(&window, confirm_label, false);
    content.append(&actions);
    window.set_child(Some(&content));

    let account = account.to_owned();
    let (parent, renaming) = match &purpose {
        NamePurpose::Create { parent } => (parent.clone(), None),
        NamePurpose::Rename { key, parent, .. } => (parent.clone(), Some(key.clone())),
    };
    let checked = {
        let check = check.clone();
        let (account, parent, renaming) = (account.clone(), parent.clone(), renaming.clone());
        move |name: &str| {
            check(&NameQuery {
                account: &account,
                parent: parent.as_deref(),
                name,
                renaming: renaming.as_deref(),
            })
        }
    };
    let show = {
        let (confirm, problem) = (confirm.clone(), problem.clone());
        move |answer: FolderNameCheck| {
            confirm.set_sensitive(answer == FolderNameCheck::Valid);
            problem.set_text(name_problem(answer).unwrap_or_default());
            problem.set_visible(name_problem(answer).is_some());
        }
    };
    show(checked(initial));
    {
        let (checked, show) = (checked.clone(), show.clone());
        entry.connect_changed(move |entry| show(checked(&entry.text())));
    }
    let submit = {
        let (entry, window, sender) = (entry.clone(), window.clone(), sender.clone());
        move || {
            let name = entry.text().to_string();
            if checked(&name) != FolderNameCheck::Valid {
                return;
            }
            let intent = match &purpose {
                NamePurpose::Create { parent } => FolderIntent::Create {
                    account: account.clone(),
                    parent: parent.clone(),
                    name,
                },
                NamePurpose::Rename { key, .. } => FolderIntent::Rename {
                    account: account.clone(),
                    key: key.clone(),
                    name,
                },
            };
            sender.emit(AppInput::Folder(FolderInput::Change(intent)));
            window.close();
        }
    };
    let submit = std::rc::Rc::new(submit);
    {
        let submit = submit.clone();
        confirm.connect_clicked(move |_| submit());
    }
    entry.connect_activate(move |_| submit());
    window.present();
    entry.grab_focus();
}

/// Lists where a folder may go (rule 24) and moves it to the one picked.
pub(crate) fn move_dialog(
    anchor: &impl IsA<gtk::Widget>,
    account: &str,
    key: &str,
    name: &str,
    candidates: Vec<MoveCandidate>,
    sender: &relm4::Sender<AppInput>,
) {
    let Some(parent_window) = window_of(anchor) else {
        return;
    };
    let (window, _) = modal::new(
        &parent_window,
        &l10n::folder_move_title(name),
        400,
        Some(420),
    );
    let content = content_box();
    let list = gtk::ListBox::new();
    list.add_css_class("boxed-list");
    list.set_selection_mode(gtk::SelectionMode::None);
    for candidate in candidates {
        let row = super::mailbox::plain_text_row();
        row.set_title(&candidate.label);
        row.set_activatable(true);
        let (sender, window) = (sender.clone(), window.clone());
        let (account, key) = (account.to_owned(), key.to_owned());
        row.connect_activated(move |_| {
            sender.emit(AppInput::Folder(FolderInput::Change(FolderIntent::Move {
                account: account.clone(),
                key: key.clone(),
                parent: candidate.parent.clone(),
            })));
            window.close();
        });
        list.append(&row);
    }
    let scroll = gtk::ScrolledWindow::new();
    scroll.set_vexpand(true);
    scroll.set_child(Some(&list));
    content.append(&scroll);
    let cancel = gtk::Button::with_label(l10n::action_cancel());
    cancel.set_halign(gtk::Align::End);
    let dialog = window.clone();
    cancel.connect_clicked(move |_| dialog.close());
    content.append(&cancel);
    window.set_child(Some(&content));
    window.present();
}

/// Confirms a delete, worded by whether the folder is already in Trash (rule 26).
pub(crate) fn delete_dialog(
    anchor: &impl IsA<gtk::Widget>,
    account: &str,
    key: &str,
    name: &str,
    in_trash: bool,
    sender: &relm4::Sender<AppInput>,
) {
    let Some(parent_window) = window_of(anchor) else {
        return;
    };
    let copy = delete_copy(name, in_trash);
    let (window, _) = modal::new(&parent_window, &copy.title, 420, None);
    window.set_resizable(false);
    let content = content_box();
    let message = gtk::Label::new(Some(copy.message));
    message.set_wrap(true);
    message.set_xalign(0.0);
    content.append(&message);
    let (actions, confirm) = action_bar(&window, copy.confirm, copy.destructive);
    content.append(&actions);
    window.set_child(Some(&content));
    let (sender, dialog) = (sender.clone(), window.clone());
    let (account, key) = (account.to_owned(), key.to_owned());
    confirm.connect_clicked(move |_| {
        sender.emit(AppInput::Folder(FolderInput::Change(
            FolderIntent::Delete {
                account: account.clone(),
                key: key.clone(),
            },
        )));
        dialog.close();
    });
    window.present();
}
