//! The composer's Draft a reply control: a reply drafted in the person's writing style, put into
//! the open composer above the signature and the quote (`docs/ai.md`, "Drafting a reply").
//!
//! Offered on a reply and a reply all only, and only while AI has somewhere to go. The draft is
//! written off the main thread, so the composer stays usable while it is; nothing on this path
//! sends. The rules it follows are `super::writing_style`.

use std::{
    cell::Cell,
    rc::{Rc, Weak},
    sync::Arc,
};

use adw::prelude::*;
use gtk::{AccessibleProperty, accessible::Property, gio};
use mailcal_bindings::{AiRoute, DraftReply, MailcalApp, WritingStyleFailure};
use webkit6::prelude::WebViewExt;

use super::{
    blocking::off_main_thread,
    composer_model::{ComposeContext, ComposeKind},
    writing_style::{changed_sender, draft_script, failure_text, intent_of, lead_is_empty},
};
use crate::l10n;

/// The live control. The composer pane owns the only strong reference; every handler reaches it
/// weakly, so tearing the composer down frees it and a draft finishing afterwards lands nowhere.
pub(super) struct DraftReplyControl {
    app: Arc<MailcalApp>,
    route: AiRoute,
    /// The message being answered.
    account: String,
    key: String,
    /// The From account the composer opened on, and the picker's rows in its own order.
    opened_from: Option<String>,
    accounts: Vec<String>,
    from: gtk::DropDown,
    editor: webkit6::WebView,
    root: gtk::Box,
    button: gtk::MenuButton,
    intent: gtk::Entry,
    spinner: gtk::Spinner,
    /// "Drafting…", the reminder about bracketed gaps, or why no draft came.
    status: gtk::Label,
    drafting: Cell<bool>,
}

impl DraftReplyControl {
    /// Builds the control, or `None` where it is not offered: a new message or a forward, or AI
    /// with nowhere to go.
    pub(super) fn new(
        app: Option<&Arc<MailcalApp>>,
        request: &ComposeContext,
        accounts: &[(String, String)],
        from: &gtk::DropDown,
        editor: &webkit6::WebView,
        send: &gtk::Button,
    ) -> Option<Rc<Self>> {
        if !matches!(request.kind, ComposeKind::Reply | ComposeKind::ReplyAll) {
            return None;
        }
        let app = app?;
        let route = app.writing_styles().route?;
        let intent = gtk::Entry::new();
        intent.set_placeholder_text(Some(l10n::composer_draft_intent_hint()));
        intent.update_property(&[Property::Label(l10n::composer_draft_intent_hint())]);
        let control = Rc::new(Self {
            app: Arc::clone(app),
            route,
            account: request.account.clone()?,
            key: request.key.clone()?,
            opened_from: request.initial_from.clone(),
            accounts: accounts.iter().map(|(id, _)| id.clone()).collect(),
            from: from.clone(),
            editor: editor.clone(),
            root: gtk::Box::new(gtk::Orientation::Horizontal, 8),
            button: gtk::MenuButton::new(),
            intent,
            spinner: gtk::Spinner::new(),
            status: gtk::Label::new(None),
            drafting: Cell::new(false),
        });
        control.build();
        control.refresh();
        let weak = Rc::downgrade(&control);
        from.connect_selected_notify(move |_| {
            if let Some(control) = weak.upgrade() {
                control.refresh();
            }
        });
        // The reminder about gaps holds until the next draft or the send.
        let weak = Rc::downgrade(&control);
        send.connect_clicked(move |_| {
            if let Some(control) = weak.upgrade()
                && !control.drafting.get()
            {
                control.status.set_visible(false);
            }
        });
        Some(control)
    }

    pub(super) fn widget(&self) -> &gtk::Box {
        &self.root
    }

    fn build(self: &Rc<Self>) {
        self.button.set_label(l10n::composer_draft_reply());
        let popover = gtk::Popover::new();
        let content = gtk::Box::new(gtk::Orientation::Vertical, 8);
        content.set_margin_top(6);
        content.set_margin_bottom(6);
        content.set_margin_start(6);
        content.set_margin_end(6);
        content.append(&self.intent);
        // Each chip puts its words in the field, where they can still be changed.
        let chips = [
            l10n::composer_draft_chip_yes(),
            l10n::composer_draft_chip_no(),
            l10n::composer_draft_chip_more_info(),
            l10n::composer_draft_chip_later(),
        ];
        for pair in chips.chunks(2) {
            let line = gtk::Box::new(gtk::Orientation::Horizontal, 6);
            for words in pair {
                let chip = gtk::Button::with_label(words);
                chip.add_css_class("pill");
                let (intent, words) = (self.intent.clone(), *words);
                chip.connect_clicked(move |_| {
                    intent.set_text(words);
                    intent.set_position(-1);
                    intent.grab_focus();
                });
                line.append(&chip);
            }
            content.append(&line);
        }
        let create = gtk::Button::with_label(l10n::composer_draft_create());
        create.add_css_class("suggested-action");
        create.set_halign(gtk::Align::End);
        let weak = Rc::downgrade(self);
        create.connect_clicked(move |_| create_from(&weak));
        let weak = Rc::downgrade(self);
        self.intent.connect_activate(move |_| create_from(&weak));
        content.append(&create);
        popover.set_child(Some(&content));
        self.button.set_popover(Some(&popover));

        self.spinner.set_visible(false);
        self.status.set_wrap(true);
        self.status.set_xalign(0.0);
        self.status.set_visible(false);
        self.root.append(&self.button);
        self.root.append(&self.spinner);
        self.root.append(&self.status);
    }

    /// The account picked in From.
    fn selected(&self) -> Option<String> {
        usize::try_from(self.from.selected())
            .ok()
            .and_then(|index| self.accounts.get(index))
            .cloned()
    }

    /// The account the reply goes out from now: the one picked in From, else the message's own.
    fn sender(&self) -> String {
        self.selected().unwrap_or_else(|| self.account.clone())
    }

    /// Offered while the sending account drafts in a style; otherwise the button says why not.
    fn refresh(&self) {
        let styled = self.app.resolve_writing_style(self.sender()).is_some();
        self.button.set_sensitive(styled && !self.drafting.get());
        if styled {
            self.button.set_tooltip_text(None);
            self.button.reset_property(AccessibleProperty::Description);
        } else {
            self.button
                .set_tooltip_text(Some(l10n::ai_error_no_style()));
            self.button
                .update_property(&[Property::Description(l10n::ai_error_no_style())]);
        }
    }

    /// Asks the editor whether the person has written anything above the signature and the
    /// quote, and asks them before a draft replaces it.
    fn create(self: &Rc<Self>) {
        self.button.popdown();
        let weak = Rc::downgrade(self);
        self.editor.evaluate_javascript(
            "window.composerLeadHasText()",
            None,
            None,
            None::<&gio::Cancellable>,
            move |answer| {
                let Some(control) = weak.upgrade() else {
                    return;
                };
                let answer = answer.ok().map(|value| value.to_str().to_string());
                if lead_is_empty(answer.as_deref()) {
                    control.draft();
                } else {
                    control.confirm_replace();
                }
            },
        );
    }

    fn confirm_replace(self: &Rc<Self>) {
        let Some(parent) = self.root.root().and_downcast::<gtk::Window>() else {
            return;
        };
        let (dialog, _) =
            super::modal::new(&parent, l10n::composer_draft_replace_title(), 420, None);
        dialog.set_resizable(false);
        let content = gtk::Box::new(gtk::Orientation::Vertical, 18);
        content.set_margin_top(24);
        content.set_margin_bottom(24);
        content.set_margin_start(24);
        content.set_margin_end(24);
        let message = gtk::Label::new(Some(l10n::composer_draft_replace_message()));
        message.set_wrap(true);
        message.set_xalign(0.0);
        content.append(&message);
        let actions = gtk::Box::new(gtk::Orientation::Horizontal, 8);
        actions.set_halign(gtk::Align::End);
        let cancel = gtk::Button::with_label(l10n::action_cancel());
        let window = dialog.clone();
        cancel.connect_clicked(move |_| window.close());
        actions.append(&cancel);
        let replace = gtk::Button::with_label(l10n::composer_draft_replace());
        replace.add_css_class("destructive-action");
        let (window, weak) = (dialog.clone(), Rc::downgrade(self));
        replace.connect_clicked(move |_| {
            window.close();
            if let Some(control) = weak.upgrade() {
                control.draft();
            }
        });
        actions.append(&replace);
        content.append(&actions);
        dialog.set_child(Some(&content));
        dialog.present();
    }

    fn draft(self: &Rc<Self>) {
        if self.drafting.replace(true) {
            return;
        }
        self.button.set_sensitive(false);
        self.spinner.set_visible(true);
        self.spinner.set_spinning(true);
        self.show_status(l10n::composer_drafting(), false);
        let app = Arc::clone(&self.app);
        let (account, key) = (self.account.clone(), self.key.clone());
        let from = changed_sender(self.selected().as_deref(), self.opened_from.as_deref());
        let intent = intent_of(&self.intent.text());
        let weak = Rc::downgrade(self);
        off_main_thread(
            move || app.draft_reply(account, key, from, None, intent, None),
            move |drafted| {
                if let Some(control) = weak.upgrade() {
                    control.drafted(drafted);
                }
            },
        );
    }

    fn drafted(&self, drafted: Result<DraftReply, WritingStyleFailure>) {
        self.drafting.set(false);
        self.spinner.set_spinning(false);
        self.spinner.set_visible(false);
        match drafted {
            Ok(draft) => {
                self.editor.evaluate_javascript(
                    &draft_script(&draft.text, &draft.draft_id),
                    None,
                    None,
                    None::<&gio::Cancellable>,
                    |_| {},
                );
                if draft.gaps.is_empty() {
                    self.status.set_visible(false);
                } else {
                    self.show_status(l10n::composer_draft_check_brackets(), false);
                }
            }
            Err(failure) => self.show_status(&failure_text(&failure, Some(self.route)), true),
        }
        self.refresh();
    }

    fn show_status(&self, text: &str, failed: bool) {
        self.status.set_text(text);
        if failed {
            self.status.add_css_class("error");
        } else {
            self.status.remove_css_class("error");
        }
        self.status.set_visible(true);
    }
}

fn create_from(control: &Weak<DraftReplyControl>) {
    if let Some(control) = control.upgrade() {
        control.create();
    }
}
