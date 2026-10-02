//! The menus a message row and a conversation row carry, and the one item every message row must
//! offer by name.
//!
//! Split from [`super::mail_actions`], which decides what an action *is* and what it dispatches;
//! this file is only how a row offers one. Each file stays under the 500-line limit.
//!
//! A row's menu opens from its ⋯ button, and from the row itself by right click, a long press, or
//! the Menu key and Shift+F10, as a folder row's does.

use adw::prelude::*;
use gtk::accessible::Property as AccessibleProperty;
use mailcal_bindings::{FlatRow, SelectedRow};

use super::{
    AppInput, folder_dialogs, folder_menu,
    mail_actions::{ActionKind, MailActionRequest, MessageTarget, actions_for},
    message_move::{self, SharedMoveContext},
    model::OpenedMessage,
};
use crate::{l10n, ui::icons};

/// What a mail row's menu needs to know beyond the row itself.
#[derive(Clone, Default)]
pub(crate) struct RowMenus {
    /// Whether the list is the Junk folder, where the spam item reads "Not spam".
    pub(crate) in_junk_folder: bool,
    /// What Move to folder… reads when the menu opens.
    pub(crate) moves: SharedMoveContext,
}

pub(super) fn message_menu_button(
    widget: &impl IsA<gtk::Widget>,
    row: &FlatRow,
    opened: &OpenedMessage,
    menus: &RowMenus,
    sender: &relm4::Sender<AppInput>,
) -> gtk::Box {
    let target = MessageTarget {
        account: row.account.clone(),
        key: row.key.clone(),
    };
    let menu = menu_box();
    menu.append(&open_in_window_item(opened, sender));
    let actions = actions_for(row.unread, row.flagged, menus.in_junk_folder);
    // Move to folder… sits with the other moves, after Move to Trash.
    let split = actions
        .iter()
        .position(|action| *action == ActionKind::MoveToTrash)
        .map_or(actions.len(), |at| at + 1);
    let labelled = |actions: &[ActionKind]| {
        actions
            .iter()
            .map(|action| (action_label(*action), target.clone(), *action))
            .collect::<Vec<_>>()
    };
    append_actions(&menu, labelled(&actions[..split]), sender);
    let moving = SelectedRow::Message {
        account: row.account.clone(),
        key: row.key.clone(),
    };
    append_move(&menu, moving, &menus.moves, sender);
    append_actions(&menu, labelled(&actions[split..]), sender);
    menu_button(widget, &menu)
}

/// The menu on one message of an expanded conversation.
///
/// It offers the window and nothing else: every message row must name that route
/// (`docs/reading-window.md`), and nothing else about a conversation's own rows changes
/// with it.
pub(super) fn message_window_menu_button(
    widget: &impl IsA<gtk::Widget>,
    opened: &OpenedMessage,
    sender: &relm4::Sender<AppInput>,
) -> gtk::Box {
    let menu = menu_box();
    menu.append(&open_in_window_item(opened, sender));
    menu_button(widget, &menu)
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
    widget: &impl IsA<gtk::Widget>,
    account: &str,
    thread_id: &str,
    menus: &RowMenus,
    sender: &relm4::Sender<AppInput>,
) -> gtk::Box {
    let menu = menu_box();
    let moving = SelectedRow::Thread {
        account: account.to_owned(),
        thread_id: thread_id.to_owned(),
    };
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
    append_move(&menu, moving, &menus.moves, sender);
    menu_button(widget, &menu)
}

/// "Move to folder…" for `row`, or for the selection when `row` is one of the selected rows
/// (`docs/folder-pane.md`, rule 24). Whether it is offered, and what it lists, is decided each time
/// the menu opens, since the row outlives the selection and the folders it was drawn against.
fn append_move(
    menu: &gtk::Box,
    row: SelectedRow,
    moves: &SharedMoveContext,
    sender: &relm4::Sender<AppInput>,
) {
    let item = gtk::Button::with_label(l10n::action_move_to_folder());
    item.add_css_class("flat");
    let (shown, asked, context) = (item.clone(), row.clone(), moves.clone());
    menu.connect_map(move |_| {
        shown.set_visible(message_move::plan(&context.borrow(), &asked).is_some());
    });
    let (input, context) = (sender.clone(), moves.clone());
    item.connect_clicked(move |button| {
        let plan = message_move::plan(&context.borrow(), &row);
        close_menu(button);
        if let Some(plan) = plan {
            folder_dialogs::message_move_dialog(button, row.clone(), plan, &input);
        }
    });
    menu.append(&item);
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

/// The ⋯ button holding `menu`, which `row` also opens by right click, a long press or the
/// keyboard.
fn menu_button(row: &impl IsA<gtk::Widget>, menu: &gtk::Box) -> gtk::Box {
    let popover = gtk::Popover::new();
    popover.set_child(Some(menu));
    let button = gtk::Button::from_icon_name(icons::MORE);
    button.set_tooltip_text(Some(l10n::a11y_more_actions()));
    button.update_property(&[AccessibleProperty::Label(l10n::a11y_more_actions())]);
    button.add_css_class("flat");
    button.set_valign(gtk::Align::Center);
    popover.set_parent(&button);
    let menu = popover.clone();
    button.connect_clicked(move |_| {
        menu.set_pointing_to(None);
        menu.popup();
    });
    let (at_pointer, anchor) = (popover.clone(), button.clone());
    // Weak: the closure lives on the row's own controller, and a strong reference back to the row
    // would keep every row the list replaces alive.
    let from_row = row.as_ref().downgrade();
    let from_keyboard = popover.clone();
    folder_menu::on_menu_request(
        row,
        move |x, y| {
            // The popover hangs off the button, so the press is placed in the button's own
            // coordinates. Truncating to a whole pixel is what a pointing rectangle is.
            #[allow(clippy::cast_possible_truncation)]
            let point = from_row
                .upgrade()
                .and_then(|row| {
                    row.compute_point(&anchor, &gtk::graphene::Point::new(x as f32, y as f32))
                })
                .map(|p| gtk::gdk::Rectangle::new(p.x() as i32, p.y() as i32, 1, 1));
            at_pointer.set_pointing_to(point.as_ref());
            at_pointer.popup();
        },
        move || {
            from_keyboard.set_pointing_to(None);
            from_keyboard.popup();
        },
    );
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
