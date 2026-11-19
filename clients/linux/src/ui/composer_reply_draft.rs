//! Draft a reply in the composer (`docs/ai.md` rules 17 to 25), drawn from the core's
//! `composer_reply_draft`: the control beside Signature, the instruction's input in a popover from
//! it, what the draft is doing among the composer's notes, and the questions it asks. It decides
//! nothing: whether the control is held, what is ticked and when to ask are the record's.
//!
//! The record is read again on `Surface::ReplyDraft` and `Surface::WritingStyle`, when From
//! changes and when the editor has parsed. Every message the editor posts is handed to the core,
//! which takes the draft's report from among them; the draft's script runs once per serial in
//! this composer's editor, a composer moved into a window of its own included.

use std::{
    cell::{Cell, RefCell},
    rc::Rc,
    sync::Arc,
};

use adw::prelude::*;
use gtk::{gio, glib};
use mailcal_bindings::{
    AiIntent, AiRemedy, ComposerReplyDraft, ControlState, Intent, MailcalApp, ReplyDraftIntent,
};
use webkit6::{LoadEvent, prelude::WebViewExt};

use super::{
    AppInput, composer_reply_draft_input as input, composer_reply_draft_notes as notes, controls,
    settings::Category,
};
use crate::l10n;

/// The account a draft is sent from, as the From picker shows it now.
pub(crate) type From = Rc<dyn Fn() -> Option<String>>;

pub(crate) struct ReplyDraft {
    app: Arc<MailcalApp>,
    composition: String,
    from: From,
    editor: webkit6::WebView,
    sender: relm4::Sender<AppInput>,
    /// Where the control stands in the action row.
    slot: gtk::Box,
    /// Where the draft is said among the composer's notes.
    notes: gtk::Box,
    popover: gtk::Popover,
    record: RefCell<Option<ComposerReplyDraft>>,
    /// The control as drawn, so it is built again only when it moves.
    button: RefCell<Option<(ControlState, gtk::MenuButton)>>,
    /// The record the notes were drawn from.
    noted: RefCell<Option<ComposerReplyDraft>>,
    /// The input open in the popover, while it is.
    input: RefCell<Option<Rc<input::Input>>>,
    /// The serial of the last script run in the editor, and whether the editor has parsed.
    ran: Cell<u64>,
    loaded: Cell<bool>,
    /// Set while the arrived draft's question is on screen, so it is asked once.
    asking: Cell<bool>,
    /// The last of the model's reads followed.
    read: Cell<u64>,
}

impl ReplyDraft {
    pub(crate) fn new(
        app: &Arc<MailcalApp>,
        composition: &str,
        from: From,
        editor: &webkit6::WebView,
        sender: relm4::Sender<AppInput>,
    ) -> Rc<Self> {
        let slot = gtk::Box::new(gtk::Orientation::Horizontal, 0);
        let notes = gtk::Box::new(gtk::Orientation::Vertical, 8);
        let popover = gtk::Popover::new();
        let draft = Rc::new(Self {
            app: Arc::clone(app),
            composition: composition.to_owned(),
            from,
            editor: editor.clone(),
            sender,
            slot,
            notes,
            popover,
            record: RefCell::new(None),
            button: RefCell::new(None),
            noted: RefCell::new(None),
            input: RefCell::new(None),
            ran: Cell::new(0),
            loaded: Cell::new(false),
            asking: Cell::new(false),
            read: Cell::new(0),
        });
        let weak = Rc::downgrade(&draft);
        draft.popover.connect_show(move |_| {
            if let Some(draft) = weak.upgrade() {
                draft.open_input();
            }
        });
        let weak = Rc::downgrade(&draft);
        draft.popover.connect_closed(move |_| {
            if let Some(draft) = weak.upgrade() {
                draft.input.replace(None);
            }
        });
        let weak = Rc::downgrade(&draft);
        editor.connect_load_changed(move |_, event| {
            if event == LoadEvent::Finished
                && let Some(draft) = weak.upgrade()
            {
                draft.loaded.set(true);
                draft.follow();
            }
        });
        draft.follow();
        draft
    }

    pub(crate) fn slot(&self) -> &gtk::Box {
        &self.slot
    }

    pub(crate) fn notes(&self) -> &gtk::Box {
        &self.notes
    }

    /// Hands the core a message the editor posted; it takes the draft's report and nothing else.
    pub(crate) fn report(&self, message: String) {
        self.app
            .composer_reply_draft_report(self.composition.clone(), message);
    }

    /// Follows the model's count of the signals Draft a reply moves on, once for each.
    pub(crate) fn follow_read(self: &Rc<Self>, read: u64) {
        if self.read.replace(read) != read {
            self.follow();
        }
    }

    /// Brings everything to the core's record.
    pub(crate) fn follow(self: &Rc<Self>) {
        let record = self
            .app
            .composer_reply_draft(self.composition.clone(), (self.from)());
        let Some(record) = record else {
            self.slot.set_visible(false);
            self.notes.set_visible(false);
            self.record.replace(None);
            return;
        };
        self.slot.set_visible(true);
        self.notes.set_visible(true);
        self.record.replace(Some(record.clone()));
        self.draw_button(record.control);
        if let Some(input) = self.input.borrow().as_ref() {
            input.update(&record);
        }
        if self.noted.borrow().as_ref() != Some(&record) {
            notes::draw(&self.notes, &record, &self.note_actions());
            self.noted.replace(Some(record.clone()));
        }
        if let Some(script) = record
            .editor_script
            .as_ref()
            .filter(|script| self.loaded.get() && script.serial > self.ran.get())
        {
            self.ran.set(script.serial);
            self.editor.evaluate_javascript(
                &script.script,
                None,
                None,
                None::<&gio::Cancellable>,
                |_| {},
            );
        }
        if record.replace_arrived && !self.asking.replace(true) {
            self.ask_arrived();
        }
    }

    /// Asks before Send while the checklist has open items: Send anyway runs `send`, Keep
    /// editing runs `keep`, and both tell the core the question was answered.
    pub(crate) fn before_send(self: &Rc<Self>, send: Box<dyn FnOnce()>, keep: Box<dyn FnOnce()>) {
        let question = self
            .app
            .composer_reply_draft(self.composition.clone(), (self.from)())
            .and_then(|record| record.send_question);
        let Some(question) = question else {
            send();
            return;
        };
        let dialog = send_question(question.open);
        let draft = Rc::clone(self);
        glib::MainContext::default().spawn_local(async move {
            let answer = dialog.choose_future(Some(&draft.editor)).await;
            draft.dispatch(ReplyDraftIntent::SendQuestionAnswered {
                composition: draft.composition.clone(),
            });
            if answer == SEND {
                send();
            } else {
                keep();
            }
        });
    }

    fn dispatch(&self, intent: ReplyDraftIntent) {
        self.app.dispatch(Intent::Ai {
            intent: AiIntent::ReplyDraft { intent },
        });
    }

    fn draw_button(self: &Rc<Self>, control: ControlState) {
        if self
            .button
            .borrow()
            .as_ref()
            .is_some_and(|(drawn, _)| *drawn == control)
        {
            return;
        }
        if let Some((_, old)) = self.button.take() {
            old.set_popover(None::<&gtk::Popover>);
            self.slot.remove(&old);
        }
        let button = controls::menu_button(l10n::composer_draft_reply(), control, &self.popover);
        self.slot.append(&button);
        self.button.replace(Some((control, button)));
    }

    /// Fills the popover from the record as it opens: the line holding the last instruction, or
    /// why it cannot draft with its remedy.
    fn open_input(self: &Rc<Self>) {
        let Some(record) = self.record.borrow().clone() else {
            return;
        };
        let (draft, remedy) = (Rc::downgrade(self), Rc::downgrade(self));
        let input = input::Input::new(
            &record,
            input::Actions {
                draft: Rc::new(move |instruction| {
                    if let Some(draft) = draft.upgrade() {
                        draft.draft(instruction);
                    }
                }),
                remedy: Rc::new(move |chosen| {
                    if let Some(draft) = remedy.upgrade() {
                        draft.open_remedy(chosen);
                    }
                }),
            },
        );
        self.popover.set_child(Some(&input.widget()));
        self.input.replace(Some(input));
    }

    /// Draft: asked first while the core says the person's text would be replaced.
    fn draft(self: &Rc<Self>, instruction: String) {
        let asks = self
            .record
            .borrow()
            .as_ref()
            .is_some_and(|record| record.ask_before_replacing);
        self.popover.popdown();
        if !asks {
            self.start(instruction, false);
            return;
        }
        let dialog = replace_question();
        let draft = Rc::clone(self);
        glib::MainContext::default().spawn_local(async move {
            if dialog.choose_future(Some(&draft.editor)).await == REPLACE {
                draft.start(instruction, true);
            }
        });
    }

    fn start(&self, instruction: String, replace: bool) {
        self.dispatch(ReplyDraftIntent::Draft {
            composition: self.composition.clone(),
            from: (self.from)(),
            instruction,
            language: l10n::active_locale().to_owned(),
            replace,
        });
    }

    /// A draft that came while the person wrote above the signature: put in, or dropped.
    fn ask_arrived(self: &Rc<Self>) {
        let dialog = replace_question();
        let draft = Rc::clone(self);
        glib::MainContext::default().spawn_local(async move {
            let replace = dialog.choose_future(Some(&draft.editor)).await == REPLACE;
            draft.dispatch(ReplyDraftIntent::AnswerArrived {
                composition: draft.composition.clone(),
                replace,
            });
            draft.asking.set(false);
        });
    }

    fn note_actions(self: &Rc<Self>) -> notes::Actions {
        let (stop, tick, remedy) = (
            Rc::downgrade(self),
            Rc::downgrade(self),
            Rc::downgrade(self),
        );
        notes::Actions {
            stop: Rc::new(move || {
                if let Some(draft) = stop.upgrade() {
                    draft.dispatch(ReplyDraftIntent::Stop {
                        composition: draft.composition.clone(),
                    });
                }
            }),
            tick: Rc::new(move |index, done| {
                if let Some(draft) = tick.upgrade() {
                    draft.dispatch(ReplyDraftIntent::Tick {
                        composition: draft.composition.clone(),
                        index,
                        done,
                    });
                }
            }),
            remedy: Rc::new(move |chosen| {
                if let Some(draft) = remedy.upgrade() {
                    draft.open_remedy(chosen);
                }
            }),
        }
    }

    /// Opens what a remedy names: the instruction's input again, Settings where a style or the
    /// endpoint is set up, or the Allodia sign-in.
    fn open_remedy(&self, remedy: AiRemedy) {
        match remedy {
            AiRemedy::TryAgain => {
                if let Some((_, button)) = self.button.borrow().as_ref() {
                    button.popup();
                }
            }
            AiRemedy::SignInToAllodia => {
                self.popover.popdown();
                self.sender.emit(AppInput::StartAllodiaSignIn);
            }
            AiRemedy::OpenAiEndpoint => {
                self.popover.popdown();
                self.sender
                    .emit(AppInput::OpenSettingsAt(Category::Advanced));
            }
            AiRemedy::LearnStyle
            | AiRemedy::ChooseStyle
            | AiRemedy::ChooseAccount
            | AiRemedy::ChooseRange => {
                self.popover.popdown();
                self.sender
                    .emit(AppInput::OpenSettingsAt(Category::WritingStyle));
            }
        }
    }
}

const KEEP: &str = "keep";
const SEND: &str = "send";
const CANCEL: &str = "cancel";
const REPLACE: &str = "replace";

/// Whether to send with `open` items left on the checklist.
fn send_question(open: u32) -> adw::AlertDialog {
    let dialog = adw::AlertDialog::new(
        Some(l10n::composer_send_open_title()),
        Some(&l10n::composer_send_open_message(i64::from(open))),
    );
    dialog.add_response(KEEP, l10n::composer_keep_editing());
    dialog.add_response(SEND, l10n::composer_send_anyway());
    dialog.set_default_response(Some(KEEP));
    dialog.set_close_response(KEEP);
    dialog
}

/// Whether the draft replaces what the person wrote above the signature and the quote.
fn replace_question() -> adw::AlertDialog {
    let dialog = adw::AlertDialog::new(
        Some(l10n::composer_draft_replace_title()),
        Some(l10n::composer_draft_replace_message()),
    );
    dialog.add_response(CANCEL, l10n::action_cancel());
    dialog.add_response(REPLACE, l10n::composer_draft_replace());
    dialog.set_response_appearance(REPLACE, adw::ResponseAppearance::Destructive);
    dialog.set_default_response(Some(CANCEL));
    dialog.set_close_response(CANCEL);
    dialog
}

#[cfg(test)]
#[path = "composer_reply_draft_tests.rs"]
pub(crate) mod tests;
