//! "The organiser wasn't told": the question a calendar server raises when it stored the answer
//! and then reported it could not pass it on (RFC 6638 §3.2.9, `docs/invitations.md`).
//!
//! Nothing here decides anything. The core raises the question, clears it the instant it is
//! answered, and signals both times; so this mirrors what it holds and never dismisses on its own.
//!
//! Four rules from the contract are visible in this file:
//!
//! 1. **The RSVP worked, and the prompt says so first.** A prompt opening with "couldn't send"
//!    would invite the user to answer again: which writes the same `PARTSTAT` and fails the same
//!    way.
//! 2. **The recipient is named.** Consent to send mail on somebody's behalf is not informed without
//!    the address, so `prompt.organizer` is in the sentence rather than "the organiser".
//! 3. **The status code is not shown.** `5.2` explains nothing to the person reading a modal; it
//!    rides the prompt for the diagnostics log.
//! 4. **The choice can be remembered, in both directions.** Ticked beside "Don't send" it is a
//!    standing *no*; the half that is easy to drop and impossible to notice, because the symptom of
//!    dropping it is being asked again forever on exactly the server the setting exists for.
//!
//! **It cannot be dismissed without answering.** Escape and the window-manager close are both
//! refused, because dismissing would leave the core holding a question the user can no longer see.
//! A dialog rather than the banner the unfiled-copy question uses: this one needs two answers and a
//! tick, which a banner has nowhere to put.

use std::cell::{Cell, RefCell};

use adw::prelude::*;
use gtk::accessible::Property as AccessibleProperty;
use mailcal_bindings::ReplyPrompt;

use crate::{l10n, ui::AppInput};

/// The question's window, opened once per question the core raises.
pub(crate) struct ReplyPromptDialog {
    /// Which question is on screen. The core carries no id, so the host counts: a generation lets
    /// GTK open the window exactly once across model renders, the same way the calendar's dialogs
    /// are opened.
    shown: Cell<Option<u64>>,
    window: RefCell<Option<gtk::Window>>,
}

impl ReplyPromptDialog {
    pub(crate) fn new() -> Self {
        Self {
            shown: Cell::new(None),
            window: RefCell::new(None),
        }
    }

    /// Opens the question, or closes the one on screen once the core has cleared it.
    pub(crate) fn render(
        &self,
        prompt: Option<&ReplyPrompt>,
        generation: u64,
        parent: &adw::ApplicationWindow,
        sender: &relm4::Sender<AppInput>,
    ) {
        let Some(prompt) = prompt else {
            // `None` is how the core says *close it*: it clears the question the moment it is
            // answered, so a stale window cannot answer twice.
            self.shown.set(None);
            if let Some(window) = self.window.borrow_mut().take() {
                // `close()` runs the handler above, which refuses: `destroy()` is the only way
                // out, and the core having cleared the question is the one thing entitled to it.
                window.destroy();
            }
            return;
        };
        if self.shown.get() == Some(generation) {
            return;
        }
        self.shown.set(Some(generation));
        let window = present(prompt, parent, sender);
        if let Some(previous) = self.window.borrow_mut().replace(window) {
            previous.destroy();
        }
    }
}

#[cfg(test)]
impl ReplyPromptDialog {
    /// The question on screen, for the widget test: the only oracle for "a refused close leaves
    /// it standing" and for what the modal actually renders.
    pub(crate) fn window(&self) -> Option<gtk::Window> {
        self.window.borrow().clone()
    }
}

fn present(
    prompt: &ReplyPrompt,
    parent: &adw::ApplicationWindow,
    sender: &relm4::Sender<AppInput>,
) -> gtk::Window {
    let (window, _) = crate::ui::modal::new(
        parent,
        l10n::invitation_reply_undelivered_title(),
        460,
        Some(240),
    );
    window.set_deletable(false);
    // Nothing may dismiss it without answering, or the core goes on holding a question the user can
    // no longer see. Both routes out are refused: the title bar's close and Escape.
    window.connect_close_request(|_| gtk::glib::Propagation::Stop);

    let shell = gtk::Box::new(gtk::Orientation::Vertical, 12);
    shell.set_margin_start(24);
    shell.set_margin_end(24);
    shell.set_margin_top(24);
    shell.set_margin_bottom(24);

    // The meeting's title and the organiser's address are both attacker-controlled; they came from
    // mail somebody else wrote. `set_use_markup(false)` before the text, never a builder property:
    // `g_object_new` applies properties in its own order, so a builder could set the text while
    // markup was still on (`AGENTS.md`, the libadwaita markup trap).
    let body = gtk::Label::new(None);
    body.set_use_markup(false);
    body.set_text(&l10n::invitation_reply_undelivered_body(
        &prompt.summary,
        &prompt.organizer,
    ));
    body.set_xalign(0.0);
    body.set_wrap(true);
    body.set_wrap_mode(gtk::pango::WrapMode::WordChar);
    shell.append(&body);

    let remember = gtk::CheckButton::with_label(l10n::invitation_reply_undelivered_remember());
    shell.append(&remember);

    let actions = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    actions.set_halign(gtk::Align::End);
    actions.set_margin_top(6);
    for (label, send, suggested) in [
        (l10n::invitation_reply_undelivered_dismiss(), false, false),
        (l10n::invitation_reply_undelivered_send(), true, true),
    ] {
        let button = gtk::Button::with_label(label);
        if suggested {
            button.add_css_class("suggested-action");
        }
        button.update_property(&[AccessibleProperty::Label(label)]);
        let input = sender.clone();
        let remember = remember.clone();
        // The subject the organiser reads in their inbox, composed here because the core carries no
        // locale. `prompt.response` is where the answer comes from: a host that remembered which
        // button was pressed would be a second source of truth for a fact the core already holds.
        let subject = super::reply_subject(prompt.response, &prompt.summary);
        let dialog = window.clone();
        button.connect_clicked(move |_| {
            input.emit(AppInput::AnswerReplyPrompt {
                send,
                // Wired to whichever button was pressed: beside "Don't send" this is a standing
                // *no*, which is what stops a server that fails every reply asking at every
                // meeting.
                remember: remember.is_active(),
                reply_subject: subject.clone(),
            });
            dialog.destroy();
        });
        actions.append(&button);
    }
    shell.append(&actions);
    window.set_child(Some(&shell));
    window.present();
    window
}
