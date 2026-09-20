//! The "Discard draft?" question: the window, and the two answers it emits.
//!
//! Split from [`super::composer_draft`], which is at the 500-line limit and holds the rule this
//! question turns on: whether anything the user put in the composer would be lost.

use gtk::prelude::{BoxExt, ButtonExt, GtkWindowExt, IsA, WidgetExt};

use super::AppInput;
use crate::l10n;

/// The "Discard draft?" question, held open until it is answered.
///
/// Its shape is the permanent-delete confirmation's, and its wording the other clients': "Keep
/// editing" rather than "Cancel", because beside "Discard" a button labelled Cancel reads as
/// "cancel the draft".
#[derive(Default)]
pub(crate) struct DiscardDraftDialog {
    open: bool,
    window: Option<gtk::Window>,
}

impl DiscardDraftDialog {
    pub(crate) fn render(
        &mut self,
        open: bool,
        parent: &impl IsA<gtk::Window>,
        sender: &relm4::Sender<AppInput>,
    ) {
        if self.open == open {
            return;
        }
        if let Some(window) = self.window.take() {
            window.close();
        }
        self.open = open;
        if !open {
            return;
        }
        let window = discard_confirmation(parent, sender);
        window.present();
        self.window = Some(window);
    }
}

fn discard_confirmation(
    parent: &impl IsA<gtk::Window>,
    sender: &relm4::Sender<AppInput>,
) -> gtk::Window {
    let (window, _) = crate::ui::modal::new(parent, l10n::compose_discard_title(), 420, Some(190));
    window.set_resizable(false);
    let content = gtk::Box::new(gtk::Orientation::Vertical, 18);
    content.set_margin_top(24);
    content.set_margin_bottom(24);
    content.set_margin_start(24);
    content.set_margin_end(24);
    let message = gtk::Label::new(Some(l10n::compose_discard_message()));
    message.set_wrap(true);
    message.set_xalign(0.0);
    content.append(&message);
    let actions = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    actions.set_halign(gtk::Align::End);
    let keep = gtk::Button::with_label(l10n::action_keep_editing());
    let dialog = window.clone();
    keep.connect_clicked(move |_| dialog.close());
    actions.append(&keep);
    let discard = gtk::Button::with_label(l10n::action_discard());
    discard.add_css_class("destructive-action");
    let input = sender.clone();
    let dialog = window.clone();
    discard.connect_clicked(move |_| {
        input.emit(AppInput::DiscardDraft);
        dialog.close();
    });
    actions.append(&discard);
    content.append(&actions);
    window.set_child(Some(&content));
    // Closing the window by any route; the keep button, Escape, the titlebar; keeps the draft.
    // The destructive answer is only ever the button that says so.
    let input = sender.clone();
    window.connect_close_request(move |_| {
        input.emit(AppInput::KeepEditing);
        gtk::glib::Propagation::Proceed
    });
    window
}

#[cfg(test)]
#[path = "composer_discard_tests.rs"]
pub(crate) mod widget_tests;
