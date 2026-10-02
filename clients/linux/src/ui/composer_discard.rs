//! The composer's Discard: when it asks first, the "Discard draft?" question it asks, and what
//! each answer does (`docs/drafts.md`, "Leaving a composer").
//!
//! Split from [`super::composer_draft`], which is at the 500-line limit and holds the rule the
//! question turns on: whether anything the user put in the composer would be lost.

use gtk::prelude::{BoxExt, ButtonExt, GtkWindowExt, IsA, WidgetExt};

use super::{AppInput, AppModel, reader::ComposerHost};
use crate::l10n;

impl AppModel {
    /// The composer's Discard button, with the composer's answer to whether it was written in.
    ///
    /// Asks first whenever there is something to lose: words in the composer, or a copy in
    /// Drafts that Discard would remove. With neither it discards at once.
    pub(super) fn discard_composer(&mut self, host: ComposerHost, edited: bool) {
        let Some(composition) = self.composition_of(host) else {
            return;
        };
        let stored = self
            .app
            .as_ref()
            .is_some_and(|app| app.draft_is_stored(composition).unwrap_or(false));
        if edited || stored {
            self.discard_prompt = Some(host);
        } else {
            self.discard_from(host);
        }
    }

    /// The question's Discard: the composer it was asked about goes, and its stored copy with it.
    pub(super) fn discard_draft(&mut self) {
        if let Some(host) = self.discard_prompt.take() {
            self.discard_from(host);
        }
    }

    /// Closes the composer hosted by `host` and removes its stored draft: the one path that
    /// takes a draft off the server (`docs/drafts.md`).
    ///
    /// The discard forgets the composition itself, so it is not closed as well.
    fn discard_from(&mut self, host: ComposerHost) {
        let composition = match host {
            ComposerHost::Pane => {
                self.composer_error = None;
                self.composer.take().map(|request| request.composition)
            }
            ComposerHost::Window(id) => {
                let composition = self.composition_of(host);
                self.forget_composer_window(id);
                composition
            }
        };
        if let Some(composition) = composition {
            self.draft_status.remove(&composition);
            self.discard_stored_draft(&composition);
        }
    }

    pub(super) fn keep_editing(&mut self) {
        self.discard_prompt = None;
    }
}

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

    /// Takes the question away with the composer window it was asked over. Closing it answers
    /// Keep editing, so the model stops waiting for an answer.
    pub(crate) fn dismiss(&mut self) {
        self.open = false;
        if let Some(window) = self.window.take() {
            window.close();
        }
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
