//! The unsent-draft guard for the Linux composer.
//!
//! The composer is an inline pane, so a click on another message stays reachable while you write.
//! Without this the draft went silently; the loss the macOS confirmation, the Windows
//! `ContentDialog` and Android's back-gesture dialog all exist to prevent.
//!
//! The dirtiness rule is theirs, deliberately: header fields are compared against what the
//! composer **opened** with, and the body against the seed captured once the quote and signature
//! were in. A reply nobody typed into is not a draft. The comparison happens here and yields one
//! boolean: the document is never logged, stored or sent (`docs/composer-security.md`).
//!
//! The body half needs a round trip because the editor has no bridge back into the host (that is
//! a security gate, not an oversight), so the host reads the document the same way Send does.

use std::{cell::RefCell, rc::Rc};

use gtk::{
    gio,
    prelude::{BoxExt, ButtonExt, EditableExt, GtkWindowExt, IsA, WidgetExt},
};
use mailcal_bindings::Intent;
use webkit6::prelude::WebViewExt;

use super::{
    AppInput, AppModel, PrimaryView,
    composer_header::RecipientRows,
    composer_model::{ComposeKind, ComposeRequest, PickedFile, initial_sender, quote_seed},
    model::OpenedMessage,
};
use crate::l10n;

/// The four header fields the guard compares, in one value so the comparison reads as one rule.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(crate) struct HeaderValues {
    pub(crate) to: String,
    pub(crate) cc: String,
    pub(crate) bcc: String,
    pub(crate) subject: String,
}

/// A surface change that must wait until the open draft says whether it would be lost.
pub(crate) enum PendingNavigation {
    Message(OpenedMessage),
    Composer(ComposeRequest),
}

/// Whether the header fields hold anything the user put there: the half of "is there a draft to
/// lose?" that needs no round trip into the editor.
///
/// Compared against what the composer opened with, never against empty: the core pre-fills a
/// reply's To and a reply-all's Cc. Stopping someone to ask about a message they never typed into
/// is exactly the noise this guard must not create. Typing something and deleting it again lands
/// back on the opening values and counts as clean, which is true; there is nothing left to lose.
pub(crate) fn headers_edited(
    current: &HeaderValues,
    opening: &HeaderValues,
    attachments: usize,
) -> bool {
    current != opening || attachments > 0
}

/// Whether the editor holds anything beyond what was seeded into it.
pub(crate) fn body_edited(seed: Option<&str>, current: &str) -> bool {
    // The seed is read the moment the seeding script returns, so its absence means that read
    // failed; not that the draft is empty. Ask: a prompt costs a click, and the alternative is
    // the loss this guard exists to prevent.
    seed.is_none_or(|seed| seed != current)
}

/// The live draft, held by the composer pane for as long as one is open.
pub(crate) struct DraftGuard {
    editor: webkit6::WebView,
    rows: RecipientRows,
    subject: gtk::Entry,
    files: Rc<RefCell<Vec<PickedFile>>>,
    opening: HeaderValues,
    seed: Rc<RefCell<Option<String>>>,
}

impl DraftGuard {
    pub(crate) fn new(
        editor: webkit6::WebView,
        rows: RecipientRows,
        subject: gtk::Entry,
        files: Rc<RefCell<Vec<PickedFile>>>,
        opening: HeaderValues,
        seed: Rc<RefCell<Option<String>>>,
    ) -> Self {
        Self {
            editor,
            rows,
            subject,
            files,
            opening,
            seed,
        }
    }

    /// Answers "would anything the user put here be lost?", emitting the one boolean.
    ///
    /// The header half settles it on its own when it is dirty, so a draft with a typed recipient
    /// never pays for the round trip.
    pub(crate) fn check(&self, sender: &relm4::Sender<AppInput>) {
        let current = HeaderValues {
            to: self.rows.to.text(),
            cc: self.rows.cc.text(),
            bcc: self.rows.bcc.text(),
            subject: self.subject.text().to_string(),
        };
        if headers_edited(&current, &self.opening, self.files.borrow().len()) {
            sender.emit(AppInput::ComposerDraftChecked(true));
            return;
        }
        let seed = Rc::clone(&self.seed);
        let sender = sender.clone();
        self.editor.evaluate_javascript(
            "composerDocument()",
            None,
            None,
            None::<&gio::Cancellable>,
            move |result| {
                let edited = match result {
                    Ok(value) => body_edited(seed.borrow().as_deref(), value.to_str().as_ref()),
                    // A failed read cannot say the draft is empty, so it asks; as above.
                    Err(_) => true,
                };
                sender.emit(AppInput::ComposerDraftChecked(edited));
            },
        );
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

impl AppModel {
    /// Opens a message, asking first when a draft is open.
    ///
    /// The composer is an inline pane, so this click stays reachable while the user writes; and
    /// clearing the composer here is what used to throw the draft away without a word. The
    /// question cannot be answered synchronously (the editor holds the body and has no bridge
    /// back), so the requested navigation waits until the guard reports.
    pub(super) fn open_message(&mut self, message: OpenedMessage) {
        if self.composer.is_some() {
            self.queue_navigation(PendingNavigation::Message(message));
            return;
        }
        self.commit_open(message);
    }

    fn commit_open(&mut self, message: OpenedMessage) {
        self.dispatch(Intent::OpenMessage {
            account: message.account.clone(),
            key: message.key.clone(),
        });
        self.reading.open(message);
        self.composer = None;
        self.primary = PrimaryView::Mail;
    }

    /// Opens the composer for a new message, reply, reply-all or forward: the routes that
    /// start from the window rather than from the OS.
    pub(super) fn begin_compose(&mut self, kind: ComposeKind) {
        let Some(app) = &self.app else {
            return;
        };
        let opened = self.reading.opened.as_ref();
        if kind != ComposeKind::New && opened.is_none() {
            return;
        }
        let (initial_to, initial_cc) = match (kind, opened) {
            (ComposeKind::Reply | ComposeKind::ReplyAll, Some(message)) => {
                let recipients = app.reply_recipients(
                    message.account.clone(),
                    message.key.clone(),
                    kind == ComposeKind::ReplyAll,
                );
                (recipients.to, recipients.cc)
            }
            _ => (String::new(), String::new()),
        };
        let quote = opened.and_then(|message| {
            let settings = app.quote_settings();
            // Only a showcase reply arrives pre-written, so the store screenshot shows a written
            // reply rather than an empty body. Every real reply passes `None`.
            #[cfg(any(debug_assertions, feature = "dev-harness"))]
            let initial_text = (kind == ComposeKind::Reply || kind == ComposeKind::ReplyAll)
                .then(|| crate::showcase::reply_text(&message.account, &message.key))
                .flatten();
            #[cfg(not(any(debug_assertions, feature = "dev-harness")))]
            let initial_text: Option<String> = None;
            quote_seed(
                message,
                &self.reading.snapshot,
                &settings.style,
                kind == ComposeKind::Forward,
                initial_text.as_deref(),
                self.calendar.display_zone(),
            )
        });
        let subject = match (kind, opened) {
            (ComposeKind::Reply | ComposeKind::ReplyAll, Some(message)) => {
                l10n::subject_reply(&message.subject)
            }
            (ComposeKind::Forward, Some(message)) => l10n::subject_forward(&message.subject),
            _ => String::new(),
        };
        self.composer_generation = self.composer_generation.wrapping_add(1);
        self.composer_error = false;
        self.composer = Some(ComposeRequest {
            kind,
            account: opened.map(|message| message.account.clone()),
            key: opened.map(|message| message.key.clone()),
            initial_to,
            initial_cc,
            initial_bcc: String::new(),
            subject,
            initial_body: None,
            quote,
            initial_from: initial_sender(
                opened,
                self.snapshot.selected_account.as_deref(),
                app.default_send_account(),
            ),
            seeds_signature: true,
            // Nothing pre-attached: only a share opens a composer already holding files.
            files: Vec::new(),
        });
    }

    pub(super) fn open_mailto(&mut self, prefill: mailcal_bindings::MailtoPrefill) {
        if self.snapshot.accounts.is_empty() {
            self.pending_mailto = Some(prefill);
            self.primary = PrimaryView::Mail;
            return;
        }
        let initial_from = self.app.as_ref().and_then(|app| {
            super::composer_model::initial_sender(
                None,
                self.snapshot.selected_account.as_deref(),
                app.default_send_account(),
            )
        });
        let request = ComposeRequest::from_mailto(prefill, initial_from);
        if self.composer.is_some() {
            self.queue_navigation(PendingNavigation::Composer(request));
        } else {
            self.commit_composer(request);
        }
    }

    pub(super) fn open_agent_draft(&mut self, draft: mailcal_bindings::AgentDraft) {
        let initial_from = draft.account.clone().or_else(|| {
            self.app.as_ref().and_then(|app| {
                super::composer_model::initial_sender(
                    None,
                    self.snapshot.selected_account.as_deref(),
                    app.default_send_account(),
                )
            })
        });
        let request = ComposeRequest::from_agent(draft, initial_from);
        if self.composer.is_some() {
            self.queue_navigation(PendingNavigation::Composer(request));
        } else {
            self.commit_composer(request);
        }
    }

    pub(super) fn try_open_pending_mailto(&mut self) {
        if !self.snapshot.accounts.is_empty()
            && let Some(prefill) = self.pending_mailto.take()
        {
            self.open_mailto(prefill);
        }
    }

    pub(super) fn queue_navigation(&mut self, navigation: PendingNavigation) {
        self.pending_navigation = Some(navigation);
        self.draft_check_seq = self.draft_check_seq.wrapping_add(1);
        self.draft_check = Some(self.draft_check_seq);
    }

    pub(super) fn commit_composer(&mut self, request: ComposeRequest) {
        self.primary = PrimaryView::Mail;
        self.composer_generation = self.composer_generation.wrapping_add(1);
        self.composer_error = false;
        self.composer = Some(request);
    }

    /// The guard's answer: a clean draft is nothing to lose, so the navigation just happens.
    pub(super) fn draft_checked(&mut self, edited: bool) {
        self.draft_check = None;
        if edited {
            self.discard_prompt = self.pending_navigation.is_some();
            return;
        }
        self.take_pending_navigation();
    }

    pub(super) fn take_pending_navigation(&mut self) {
        self.discard_prompt = false;
        match self.pending_navigation.take() {
            Some(PendingNavigation::Message(message)) => self.commit_open(message),
            Some(PendingNavigation::Composer(request)) => self.commit_composer(request),
            None => {}
        }
    }

    pub(super) fn keep_editing(&mut self) {
        self.discard_prompt = false;
        self.pending_navigation = None;
    }
}

#[cfg(test)]
#[path = "composer_draft_tests.rs"]
pub(crate) mod widget_tests;

#[cfg(test)]
mod tests {
    use super::{HeaderValues, body_edited, headers_edited};

    fn reply_opened_with() -> HeaderValues {
        HeaderValues {
            to: "alice@test.local".to_owned(),
            cc: String::new(),
            bcc: String::new(),
            subject: "Re: Lunch".to_owned(),
        }
    }

    /// The core pre-fills a reply's recipients, so comparing against empty would stop the user on
    /// every message they opened a reply to and thought better of.
    #[test]
    fn a_reply_nobody_typed_into_is_not_a_draft() {
        assert!(!headers_edited(
            &reply_opened_with(),
            &reply_opened_with(),
            0
        ));
    }

    #[test]
    fn every_header_field_counts_as_a_draft() {
        for edited in [
            HeaderValues {
                to: "bob@test.local".to_owned(),
                ..reply_opened_with()
            },
            HeaderValues {
                cc: "carol@test.local".to_owned(),
                ..reply_opened_with()
            },
            HeaderValues {
                bcc: "dan@test.local".to_owned(),
                ..reply_opened_with()
            },
            HeaderValues {
                subject: "Re: Lunch tomorrow".to_owned(),
                ..reply_opened_with()
            },
        ] {
            assert!(
                headers_edited(&edited, &reply_opened_with(), 0),
                "{edited:?} should count as edited"
            );
        }
    }

    /// An attached file is work that would be lost even with every field untouched.
    #[test]
    fn an_attachment_alone_is_a_draft() {
        assert!(headers_edited(
            &reply_opened_with(),
            &reply_opened_with(),
            1
        ));
    }

    /// Typing and deleting again leaves nothing to lose.
    #[test]
    fn returning_to_the_opening_values_is_clean_again() {
        let typed = HeaderValues {
            to: "bob@test.local".to_owned(),
            ..reply_opened_with()
        };
        assert!(headers_edited(&typed, &reply_opened_with(), 0));
        assert!(!headers_edited(
            &reply_opened_with(),
            &reply_opened_with(),
            0
        ));
    }

    #[test]
    fn the_body_is_measured_against_the_seed_not_against_empty() {
        let seeded = r#"{"blocks":[{"text":"> lunch?"}]}"#;
        assert!(!body_edited(Some(seeded), seeded));
        assert!(body_edited(
            Some(seeded),
            r#"{"blocks":[{"text":"yes"},{"text":"> lunch?"}]}"#
        ));
    }

    /// The read that would settle it failed, so the guard asks rather than assuming empty.
    #[test]
    fn a_missing_seed_asks() {
        assert!(body_edited(None, r#"{"blocks":[]}"#));
    }
}
