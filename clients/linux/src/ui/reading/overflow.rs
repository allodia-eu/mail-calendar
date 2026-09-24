//! The reading pane's overflow menu: the actions on the open message as a *document*, at the
//! end of the header (`../../../docs/reading-actions.md`).

use std::{cell::RefCell, rc::Rc};

use adw::prelude::*;
use gtk::{accessible::Property as AccessibleProperty, glib};
use mailcal_bindings::save::message_export_file_name;

use super::{
    AppInput, ReadingSource,
    print::{PrintSource, print_item},
};
use crate::l10n;

/// The file name the export offers, kept in step with the message on screen.
///
/// The menu is built once and the open message changes under it, so the name cannot be captured
/// when the item is made. The pane writes this on every render from the subject it *draws*,
/// which is how an untitled message exports as "(no subject).eml" in the user's own language
/// rather than under a second, English name.
pub(super) type ExportName = Rc<RefCell<String>>;

/// The overflow button, with its menu attached; the button itself, for sensitivity; and the Print
/// item, whose sensitivity follows whether the open message's body has arrived.
///
/// The container is a `gtk::Box` because the popover is parented to the button and has to be
/// unparented before the button is disposed of, so the two have to travel together.
pub(super) fn overflow_menu(
    window: &gtk::Window,
    source: &ReadingSource,
    export_name: &ExportName,
    print_source: &PrintSource,
    sender: &relm4::Sender<AppInput>,
) -> (gtk::Box, gtk::Button, gtk::Button) {
    let menu = gtk::Box::new(gtk::Orientation::Vertical, 0);
    menu.set_margin_top(6);
    menu.set_margin_bottom(6);
    menu.set_margin_start(6);
    menu.set_margin_end(6);
    menu.append(&export_item(window, source, export_name, sender));
    let print = print_item(window, print_source, sender);
    menu.append(&print);

    let popover = gtk::Popover::new();
    popover.set_child(Some(&menu));
    let button = gtk::Button::from_icon_name("view-more-symbolic");
    button.set_tooltip_text(Some(l10n::a11y_more_actions()));
    button.update_property(&[AccessibleProperty::Label(l10n::a11y_more_actions())]);
    button.add_css_class("flat");
    popover.set_parent(&button);
    let menu_popover = popover.clone();
    button.connect_clicked(move |_| menu_popover.popup());
    button.connect_destroy(move |_| {
        popover.popdown();
        popover.unparent();
    });

    let container = gtk::Box::new(gtk::Orientation::Horizontal, 0);
    container.append(&button);
    (container, button, print)
}

/// The "Save as .eml" item: closes the menu, asks where, then hands the destination to the
/// export.
fn export_item(
    window: &gtk::Window,
    source: &ReadingSource,
    export_name: &ExportName,
    sender: &relm4::Sender<AppInput>,
) -> gtk::Button {
    let item = gtk::Button::with_label(l10n::action_save_as_eml());
    item.add_css_class("flat");
    let input_sender = sender.clone();
    let parent = window.clone();
    let source = source.clone();
    let export_name = Rc::clone(export_name);
    item.connect_clicked(move |button| {
        if let Some(popover) = button
            .ancestor(gtk::Popover::static_type())
            .and_downcast::<gtk::Popover>()
        {
            popover.popdown();
        }
        let dialog = gtk::FileDialog::builder()
            .initial_name(export_name.borrow().as_str())
            .build();
        let input_sender = input_sender.clone();
        let parent = parent.clone();
        let source = source.clone();
        glib::MainContext::default().spawn_local(async move {
            if let Ok(file) = dialog.save_future(Some(&parent)).await
                && let Some(path) = file.path()
            {
                input_sender.emit(AppInput::ExportMessage {
                    source,
                    destination: path,
                });
            }
        });
    });
    item
}

/// The name to offer for `subject`, which is the subject the pane draws rather than the stored
/// one, so a blank subject is named in the app's language.
pub(super) fn export_name_for(subject: &str) -> String {
    message_export_file_name(subject.to_owned())
}
