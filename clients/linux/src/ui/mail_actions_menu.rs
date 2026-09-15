//! The menus a message row and a conversation row carry, and the one item every message row must
//! offer by name.
//!
//! Split from [`super::mail_actions`], which decides what an action *is* and what it dispatches;
//! this file is only how a row offers one. Each file stays under the 500-line limit.

use adw::prelude::*;
use gtk::accessible::Property as AccessibleProperty;
use mailcal_bindings::FlatRow;

use super::{
    AppInput,
    mail_actions::{ActionKind, MailActionRequest, MessageTarget, actions_for},
    model::OpenedMessage,
};
use crate::l10n;

pub(super) fn message_menu_button(
    row: &FlatRow,
    opened: &OpenedMessage,
    in_junk_folder: bool,
    sender: &relm4::Sender<AppInput>,
) -> gtk::Box {
    let target = MessageTarget {
        account: row.account.clone(),
        key: row.key.clone(),
    };
    let menu = menu_box();
    menu.append(&open_in_window_item(opened, sender));
    append_actions(
        &menu,
        actions_for(row.unread, row.flagged, in_junk_folder)
            .into_iter()
            .map(|action| (action_label(action), target.clone(), action)),
        sender,
    );
    menu_button(&menu)
}

/// The menu on one message of an expanded conversation.
///
/// It offers the window and nothing else: every message row must name that route
/// (`docs/reading-window.md`), and nothing else about a conversation's own rows changes
/// with it.
pub(super) fn message_window_menu_button(
    opened: &OpenedMessage,
    sender: &relm4::Sender<AppInput>,
) -> gtk::Box {
    let menu = menu_box();
    menu.append(&open_in_window_item(opened, sender));
    menu_button(&menu)
}

/// "Open in new window": the named route to what a double-click does
/// (`docs/reading-window.md`).
///
/// A double-click cannot be reached from the keyboard or invoked by a screen reader, so this item,
/// not the gesture, is what the capability matrix claims.
fn open_in_window_item(opened: &OpenedMessage, sender: &relm4::Sender<AppInput>) -> gtk::Button {
    let item = gtk::Button::with_label(l10n::action_open_in_window());
    item.add_css_class("flat");
    let input = sender.clone();
    let opened = opened.clone();
    item.connect_clicked(move |button| {
        close_menu(button);
        input.emit(AppInput::OpenMessageInWindow(Box::new(opened.clone())));
    });
    item
}

pub(super) fn thread_menu_button(
    account: &str,
    thread_id: &str,
    sender: &relm4::Sender<AppInput>,
) -> gtk::Box {
    let menu = menu_box();
    let archive = gtk::Button::with_label(l10n::thread_archive());
    archive.add_css_class("flat");
    let input = sender.clone();
    let account = account.to_owned();
    let thread_id = thread_id.to_owned();
    archive.connect_clicked(move |button| {
        close_menu(button);
        input.emit(AppInput::ArchiveThread {
            account: account.clone(),
            thread_id: thread_id.clone(),
        });
    });
    menu.append(&archive);
    menu_button(&menu)
}

fn menu_box() -> gtk::Box {
    let menu = gtk::Box::new(gtk::Orientation::Vertical, 0);
    menu.set_margin_top(6);
    menu.set_margin_bottom(6);
    menu.set_margin_start(6);
    menu.set_margin_end(6);
    menu
}

fn append_actions(
    menu: &gtk::Box,
    actions: impl IntoIterator<Item = (&'static str, MessageTarget, ActionKind)>,
    sender: &relm4::Sender<AppInput>,
) {
    for (label, target, action) in actions {
        let item = gtk::Button::with_label(label);
        item.add_css_class("flat");
        if action == ActionKind::PermanentlyDelete {
            item.add_css_class("destructive-action");
        }
        let input = sender.clone();
        item.connect_clicked(move |button| {
            close_menu(button);
            if action == ActionKind::PermanentlyDelete {
                input.emit(AppInput::RequestPermanentDelete(target.clone()));
            } else {
                input.emit(AppInput::PerformMailAction(Box::new(
                    MailActionRequest::new(target.clone(), action),
                )));
            }
        });
        menu.append(&item);
    }
}

fn menu_button(menu: &gtk::Box) -> gtk::Box {
    let popover = gtk::Popover::new();
    popover.set_child(Some(menu));
    let button = gtk::Button::from_icon_name("view-more-symbolic");
    button.set_tooltip_text(Some(l10n::a11y_more_actions()));
    button.update_property(&[AccessibleProperty::Label(l10n::a11y_more_actions())]);
    button.add_css_class("flat");
    button.set_valign(gtk::Align::Center);
    popover.set_parent(&button);
    let menu = popover.clone();
    button.connect_clicked(move |_| menu.popup());
    button.connect_destroy(move |_| {
        popover.popdown();
        popover.unparent();
    });
    let container = gtk::Box::new(gtk::Orientation::Horizontal, 0);
    container.append(&button);
    container
}

fn close_menu(button: &gtk::Button) {
    if let Some(popover) = button
        .ancestor(gtk::Popover::static_type())
        .and_downcast::<gtk::Popover>()
    {
        popover.popdown();
    }
}

fn action_label(action: ActionKind) -> &'static str {
    match action {
        ActionKind::MarkRead(true) => l10n::action_mark_read(),
        ActionKind::MarkRead(false) => l10n::action_mark_unread(),
        ActionKind::SetFlagged(true) => l10n::action_flag(),
        ActionKind::SetFlagged(false) => l10n::action_unflag(),
        ActionKind::Archive => l10n::action_archive(),
        ActionKind::MoveToTrash => l10n::action_move_to_trash(),
        ActionKind::MarkAsSpam => l10n::action_mark_as_spam(),
        ActionKind::MarkAsNotSpam => l10n::action_mark_as_not_spam(),
        ActionKind::PermanentlyDelete => l10n::action_delete_permanently(),
    }
}
