//! The context menu a folder-pane row carries (`docs/folder-pane.md`, rule 23): right click, a
//! long press, or the Menu key and Shift+F10 on a focused row.
//!
//! Which items it holds is `folder_actions::folder_menu`; this file only offers them and reports
//! the one chosen.

use std::rc::Rc;

use adw::prelude::*;

use super::folder_actions::FolderMenuItem;

/// Gives `row` a context menu of `items`, calling `chosen` with the one picked. A row with no items
/// gets no menu at all, so right click there does nothing rather than open an empty popover.
pub(crate) fn attach(
    row: &adw::ActionRow,
    items: Vec<FolderMenuItem>,
    chosen: impl Fn(FolderMenuItem, &adw::ActionRow) + 'static,
) {
    if items.is_empty() {
        return;
    }
    let chosen = Rc::new(chosen);
    let menu = gtk::Box::new(gtk::Orientation::Vertical, 0);
    menu.set_margin_top(6);
    menu.set_margin_bottom(6);
    menu.set_margin_start(6);
    menu.set_margin_end(6);
    let popover = gtk::Popover::new();
    popover.set_has_arrow(false);
    popover.set_child(Some(&menu));
    popover.set_parent(row);
    for item in items {
        let button = gtk::Button::with_label(item.label());
        button.add_css_class("flat");
        let (popover, chosen, row) = (popover.clone(), chosen.clone(), row.clone());
        button.connect_clicked(move |_| {
            popover.popdown();
            chosen(item, &row);
        });
        menu.append(&button);
    }

    let open_at = {
        let popover = popover.clone();
        move |x: f64, y: f64| {
            // Truncating a pointer position to a whole pixel is what a pointing rectangle is.
            #[allow(clippy::cast_possible_truncation)]
            popover.set_pointing_to(Some(&gtk::gdk::Rectangle::new(x as i32, y as i32, 1, 1)));
            popover.popup();
        }
    };
    let click = gtk::GestureClick::new();
    click.set_button(gtk::gdk::BUTTON_SECONDARY);
    let at = open_at.clone();
    click.connect_pressed(move |gesture, _, x, y| {
        gesture.set_state(gtk::EventSequenceState::Claimed);
        at(x, y);
    });
    row.add_controller(click);
    let press = gtk::GestureLongPress::new();
    press.set_touch_only(true);
    let at = open_at.clone();
    press.connect_pressed(move |_, x, y| at(x, y));
    row.add_controller(press);
    let keys = gtk::ShortcutController::new();
    let from_keyboard = {
        let popover = popover.clone();
        gtk::CallbackAction::new(move |_, _| {
            popover.set_pointing_to(None);
            popover.popup();
            gtk::glib::Propagation::Stop
        })
    };
    for trigger in ["Menu", "<Shift>F10"] {
        if let Some(trigger) = gtk::ShortcutTrigger::parse_string(trigger) {
            keys.add_shortcut(gtk::Shortcut::new(
                Some(trigger),
                Some(from_keyboard.clone()),
            ));
        }
    }
    row.add_controller(keys);
    row.connect_destroy(move |_| {
        popover.popdown();
        popover.unparent();
    });
}
