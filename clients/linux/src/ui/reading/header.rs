//! The action row above an open message (`../../../docs/reading-actions.md`), and the two things
//! about it that differ between the hosts it is drawn in.
//!
//! Split from [`super`] so each file stays under the 500-line limit. The row is the same in both
//! hosts, in the same order, overflow last of all; what a detached window changes is the chrome
//! around it, because there the bar *is* the window's titlebar
//! (`../../../docs/reading-window.md`).

use std::{cell::RefCell, rc::Rc};

use adw::prelude::*;
use gtk::accessible::Property as AccessibleProperty;

use super::{
    ActionKind, AppInput, ReadingSource,
    overflow::{ExportName, overflow_menu},
    print::PrintSource,
};
use crate::l10n;

/// The built row: the bar to mount, the buttons whose sensitivity follows the open message, the
/// file name the export offers, and what Print prints with the item that offers it.
pub(super) struct ActionRow {
    pub(super) header: adw::HeaderBar,
    pub(super) actions: [gtk::Button; 6],
    pub(super) export_name: ExportName,
    pub(super) print_source: PrintSource,
    pub(super) print_item: gtk::Button,
}

/// Builds the row for one reader. Every button names `source`, so a window acts on its own
/// message and never on whatever the pane behind it is showing.
pub(super) fn action_row(
    window: &gtk::Window,
    source: &ReadingSource,
    sender: &relm4::Sender<AppInput>,
) -> ActionRow {
    let detached = source.window().is_some();
    let header = adw::HeaderBar::new();
    if detached {
        // A detached window has no chrome of its own: this bar *is* its titlebar, so it keeps
        // the toolkit's controls and draws the window's title, which its host sets to the
        // message's subject (`super::super::detached`).
        header.set_title_widget(None::<&gtk::Widget>);
    } else {
        header.set_show_start_title_buttons(false);
        // The selection bar above this pane is the mail surface's top row and carries the
        // window's controls; a second set here would sit a row short of the window's own
        // corner.
        header.set_show_end_title_buttons(false);
        // An empty title, or the header falls back to the window's, standing the application's
        // own name over the message being read.
        header.set_title_widget(Some(&gtk::Label::new(None)));
    }
    let reply = action_button("mail-reply-sender-symbolic", l10n::action_reply());
    let reply_all = action_button("mail-reply-all-symbolic", l10n::action_reply_all());
    let forward = action_button("mail-forward-symbolic", l10n::action_forward());
    let archive = action_button("mailcal-archive-symbolic", l10n::action_archive());
    let trash = action_button("user-trash-symbolic", l10n::action_move_to_trash());
    let input_sender = sender.clone();
    let reader = source.clone();
    reply.connect_clicked(move |_| {
        input_sender.emit(AppInput::BeginReply {
            source: reader.clone(),
            all: false,
        });
    });
    header.pack_start(&reply);
    let input_sender = sender.clone();
    let reader = source.clone();
    reply_all.connect_clicked(move |_| {
        input_sender.emit(AppInput::BeginReply {
            source: reader.clone(),
            all: true,
        });
    });
    header.pack_start(&reply_all);
    let input_sender = sender.clone();
    let reader = source.clone();
    forward.connect_clicked(move |_| {
        input_sender.emit(AppInput::BeginForward(reader.clone()));
    });
    header.pack_start(&forward);
    // Packed before archive and trash: `pack_end` fills from the end inward, so the first
    // widget packed is the rightmost, and the overflow belongs last of all
    // (`../../../docs/reading-actions.md`).
    let export_name: ExportName = Rc::new(RefCell::new(String::new()));
    let print_source: PrintSource = Rc::new(RefCell::new(None));
    let (overflow, overflow_button, print_item) =
        overflow_menu(window, source, &export_name, &print_source, sender);
    header.pack_end(&overflow);
    let input_sender = sender.clone();
    let reader = source.clone();
    archive.connect_clicked(move |_| {
        input_sender.emit(AppInput::PerformOpenedMailAction {
            source: reader.clone(),
            action: ActionKind::Archive,
        });
    });
    header.pack_end(&archive);
    let input_sender = sender.clone();
    let reader = source.clone();
    trash.connect_clicked(move |_| {
        input_sender.emit(AppInput::PerformOpenedMailAction {
            source: reader.clone(),
            action: ActionKind::MoveToTrash,
        });
    });
    header.pack_end(&trash);

    ActionRow {
        header,
        actions: [reply, reply_all, forward, archive, trash, overflow_button],
        export_name,
        print_source,
        print_item,
    }
}

fn action_button(icon: &str, tooltip: &str) -> gtk::Button {
    let button = gtk::Button::from_icon_name(icon);
    button.set_tooltip_text(Some(tooltip));
    button.update_property(&[AccessibleProperty::Label(tooltip)]);
    button
}
