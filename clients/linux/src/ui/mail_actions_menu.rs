//! The menu every mail row carries: a message, a conversation, and a message inside one.
//!
//! What it holds is the core's (`docs/row-actions.md`), asked each time it opens, because the row
//! outlives the selection and the snapshot it was drawn against; [`super::row_menu`] says what
//! picking an item does. A row's menu opens from its ⋯ button, and from the row itself by right
//! click, a long press, or the Menu key and Shift+F10, as a folder row's does.

use std::rc::Rc;

use adw::prelude::*;
use gtk::accessible::Property as AccessibleProperty;
use mailcal_bindings::{RowAction, RowMenuItem, SelectedRow};

use super::{
    AppInput, folder_dialogs, folder_menu,
    message_move::{self, SharedMoveContext},
    row_menu::{self, RowMenuSource, RowPick},
};
use crate::{l10n, ui::icons};

/// What a mail row's menu reads when it opens, shared by every row and replaced on every render.
#[derive(Clone, Default)]
pub(crate) struct RowMenus {
    /// The selection, and what Move to folder… offers.
    pub(crate) moves: SharedMoveContext,
    /// The core, which answers what the menu holds.
    pub(crate) source: RowMenuSource,
}

/// The ⋯ button and row menu for `row`: the menu over the selection when `row` is one of the
/// selected rows (`docs/list-selection.md`, rule 12), and over `row` alone otherwise.
pub(super) fn row_menu_button(
    widget: &impl IsA<gtk::Widget>,
    row: SelectedRow,
    menus: &RowMenus,
    sender: &relm4::Sender<AppInput>,
) -> gtk::Box {
    let menu = menu_box();
    let (filled, menus, sender) = (menu.clone(), menus.clone(), sender.clone());
    menu_button(widget, &menu, move || fill(&filled, &row, &menus, &sender))
}

/// Draws into `menu` what the core answers for `row` now: its sections in order with a separator
/// between them, each item disabled where it does not respond.
pub(super) fn fill(
    menu: &gtk::Box,
    row: &SelectedRow,
    menus: &RowMenus,
    sender: &relm4::Sender<AppInput>,
) {
    while let Some(child) = menu.first_child() {
        menu.remove(&child);
    }
    let rows = menus.moves.borrow().rows_for(row);
    let answer = menus.source.menu(rows.clone());
    for (index, section) in answer.sections.iter().enumerate() {
        if index > 0 {
            menu.append(&gtk::Separator::new(gtk::Orientation::Horizontal));
        }
        for item in &section.items {
            menu.append(&item_button(item, &rows, row, &menus.moves, sender));
        }
    }
}

fn item_button(
    item: &RowMenuItem,
    rows: &[SelectedRow],
    row: &SelectedRow,
    moves: &SharedMoveContext,
    sender: &relm4::Sender<AppInput>,
) -> gtk::Button {
    let button = gtk::Button::with_label(row_menu::label(item.action));
    button.add_css_class("flat");
    if item.action == RowAction::PermanentlyDelete {
        button.add_css_class("destructive-action");
    }
    // The core says whether a move would take anything; whether there is a folder to offer, in
    // one account, is the picker's.
    let movable = item.action != RowAction::MoveToFolder
        || message_move::plan(&moves.borrow(), row).is_some();
    let picked = row_menu::pick(item, rows);
    button.set_sensitive(item.enabled && movable && picked.is_some());
    let (row, moves, input) = (row.clone(), moves.clone(), sender.clone());
    button.connect_clicked(move |button| {
        close_menu(button);
        match picked.clone() {
            Some(RowPick::Move) => {
                if let Some(plan) = message_move::plan(&moves.borrow(), &row) {
                    folder_dialogs::message_move_dialog(button, row.clone(), plan, &input);
                }
            }
            Some(pick) => input.emit(AppInput::RowMenuPick(Box::new(pick))),
            None => {}
        }
    });
    button
}

fn menu_box() -> gtk::Box {
    let menu = gtk::Box::new(gtk::Orientation::Vertical, 0);
    menu.set_margin_top(6);
    menu.set_margin_bottom(6);
    menu.set_margin_start(6);
    menu.set_margin_end(6);
    menu
}

/// The ⋯ button holding `menu`, which `row` also opens by right click, a long press or the
/// keyboard. `fill` draws the menu each time, just before it opens.
fn menu_button(
    row: &impl IsA<gtk::Widget>,
    menu: &gtk::Box,
    fill: impl Fn() + 'static,
) -> gtk::Box {
    let fill = Rc::new(fill);
    let popover = gtk::Popover::new();
    popover.set_child(Some(menu));
    let button = gtk::Button::from_icon_name(icons::MORE);
    button.set_tooltip_text(Some(l10n::a11y_more_actions()));
    button.update_property(&[AccessibleProperty::Label(l10n::a11y_more_actions())]);
    button.add_css_class("flat");
    button.set_valign(gtk::Align::Center);
    popover.set_parent(&button);
    let (menu, filled) = (popover.clone(), Rc::clone(&fill));
    button.connect_clicked(move |_| {
        filled();
        menu.set_pointing_to(None);
        menu.popup();
    });
    let (at_pointer, anchor) = (popover.clone(), button.clone());
    // Weak: the closure lives on the row's own controller, and a strong reference back to the row
    // would keep every row the list replaces alive.
    let from_row = row.as_ref().downgrade();
    let from_keyboard = popover.clone();
    let (filled_at_pointer, filled_from_keyboard) = (Rc::clone(&fill), fill);
    folder_menu::on_menu_request(
        row,
        move |x, y| {
            filled_at_pointer();
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
            filled_from_keyboard();
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
