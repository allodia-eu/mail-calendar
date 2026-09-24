//! The composer's Draft a reply control: a reply drafted in the person's writing style, put into
//! the open composer above the signature and the quote, with the card that says what the message
//! asks and what is left to do (`docs/ai.md`, "Drafting a reply").
//!
//! Offered on a reply and a reply all only, and only while AI has somewhere to go. The draft is
//! written off the main thread, so the composer stays usable while it is; nothing on this path
//! sends, and Send only asks, once, while an item on the card is open. The rules it follows are
//! `super::writing_style` and `super::draft_checklist`.

use std::{
    cell::{Cell, RefCell},
    rc::{Rc, Weak},
    sync::Arc,
};

use adw::prelude::*;
use gtk::{AccessibleProperty, accessible::Property, gio, glib};
use mailcal_bindings::{AiRoute, DraftReply, DraftTask, MailcalApp, WritingStyleFailure};
use webkit6::prelude::WebViewExt;

use super::{
    blocking::off_main_thread,
    composer_draft_card::{DraftCard, OnTick, confirm_send},
    composer_model::{ComposeContext, ComposeKind, PickedFile},
    draft_checklist::{DraftChecklist, placeholders_answer, placeholders_script},
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
    /// The files attached to the reply, which an attach item follows.
    files: Rc<RefCell<Vec<PickedFile>>>,
    card: DraftCard,
    checklist: RefCell<DraftChecklist>,
    /// Whether the card's timer is running; one at a time, whichever draft it is for.
    following: Cell<bool>,
    /// Set while Send reads the placeholders before asking, so a second click does not ask twice.
    asking: Cell<bool>,
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
        files: &Rc<RefCell<Vec<PickedFile>>>,
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
            files: Rc::clone(files),
            card: DraftCard::new(),
            checklist: RefCell::new(DraftChecklist::default()),
            following: Cell::new(false),
            asking: Cell::new(false),
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

    /// The card, which the composer puts between its buttons and the editor.
    pub(super) fn card(&self) -> &gtk::Box {
        self.card.widget()
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
        // The summary and the checklist are written in the language the app is shown in.
        let language = l10n::active_locale().to_owned();
        let weak = Rc::downgrade(self);
        off_main_thread(
            move || app.draft_reply(account, key, from, None, intent, None, language),
            move |drafted| {
                if let Some(control) = weak.upgrade() {
                    control.drafted(drafted);
                }
            },
        );
    }

    fn drafted(self: &Rc<Self>, drafted: Result<DraftReply, WritingStyleFailure>) {
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
                // The card's checklist names every gap, so the reminder stands only without one.
                if draft.gaps.is_empty() || !draft.tasks.is_empty() {
                    self.status.set_visible(false);
                } else {
                    self.show_status(l10n::composer_draft_check_brackets(), false);
                }
                self.show_checklist(&draft.summary, &draft.tasks);
            }
            Err(failure) => self.show_status(&failure_text(&failure, Some(self.route)), true),
        }
        self.refresh();
    }

    /// Replaces the card with a draft's, and follows the editor and the files while an item that
    /// ticks itself is open.
    fn show_checklist(self: &Rc<Self>, summary: &str, tasks: &[DraftTask]) {
        let attached = self.files.borrow().len();
        self.checklist.borrow_mut().show(summary, tasks, attached);
        let weak = Rc::downgrade(self);
        let on_tick: OnTick = Rc::new(move |index, ticked| {
            if let Some(control) = weak.upgrade() {
                control.ticked(index, ticked);
            }
        });
        self.card.show(&self.checklist.borrow(), &on_tick);
        self.follow();
    }

    /// The person's tick. A check the card set to what the checklist already says changes
    /// nothing.
    fn ticked(&self, index: usize, ticked: bool) {
        let agrees = self
            .checklist
            .borrow()
            .items()
            .get(index)
            .is_none_or(|item| item.ticked == ticked);
        if agrees {
            return;
        }
        self.checklist.borrow_mut().toggle(index);
        self.card.refresh(&self.checklist.borrow());
    }

    /// About once a second while an item that ticks itself is open: the files attached since the
    /// draft, and the placeholders still in the reply. The timer stops when nothing is left to
    /// follow or the composer has gone.
    fn follow(self: &Rc<Self>) {
        if !self.checklist.borrow().follows() || self.following.replace(true) {
            return;
        }
        let weak = Rc::downgrade(self);
        glib::timeout_add_seconds_local(1, move || {
            let Some(control) = weak.upgrade() else {
                return glib::ControlFlow::Break;
            };
            let attached = control.files.borrow().len();
            control.checklist.borrow_mut().attachments_changed(attached);
            control.card.refresh(&control.checklist.borrow());
            let (follows, placeholders) = {
                let checklist = control.checklist.borrow();
                (checklist.follows(), checklist.awaits_placeholders())
            };
            if !follows {
                control.following.set(false);
                return glib::ControlFlow::Break;
            }
            if placeholders {
                control.read_placeholders(|_| {});
            }
            glib::ControlFlow::Continue
        });
    }

    /// Asks the editor which placeholders are still in the reply, ticks the card by the answer,
    /// then runs `then`. No answer, or one about an earlier draft, ticks nothing.
    fn read_placeholders(self: &Rc<Self>, then: impl FnOnce(&Rc<Self>) + 'static) {
        let (placeholders, draft) = {
            let checklist = self.checklist.borrow();
            (checklist.placeholders(), checklist.draft())
        };
        if placeholders.is_empty() {
            then(self);
            return;
        }
        let weak = Rc::downgrade(self);
        self.editor.evaluate_javascript(
            &placeholders_script(&placeholders),
            None,
            None,
            None::<&gio::Cancellable>,
            move |answer| {
                let Some(control) = weak.upgrade() else {
                    return;
                };
                let json = answer.ok().and_then(|value| value.to_json(0));
                let current = control.checklist.borrow().draft() == draft;
                if current && let Some(left) = placeholders_answer(json.as_deref()) {
                    control.checklist.borrow_mut().placeholders_left(&left);
                    control.card.refresh(&control.checklist.borrow());
                }
                then(&control);
            },
        );
    }

    /// Send, asking first while an item on the card is open. The placeholders are read afresh,
    /// the question is asked once per composer, and neither answer stops a later Send.
    pub(super) fn before_send(self: &Rc<Self>, send: Rc<dyn Fn()>) {
        let settled = {
            let checklist = self.checklist.borrow();
            checklist.has_asked() || checklist.items().is_empty()
        };
        if settled {
            send();
            return;
        }
        if self.asking.replace(true) {
            return;
        }
        self.read_placeholders(move |control| {
            control.asking.set(false);
            if !control.checklist.borrow().asks_before_send() {
                send();
                return;
            }
            control.checklist.borrow_mut().send_asked();
            let open = control.checklist.borrow().open_count();
            match control.root.root().and_downcast::<gtk::Window>() {
                Some(parent) => confirm_send(&parent, open, send),
                None => send(),
            }
        });
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
