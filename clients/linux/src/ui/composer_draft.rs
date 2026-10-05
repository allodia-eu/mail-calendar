//! Leaving a composer, and the rule both ways out of one turn on: whether anything was written
//! in it (`docs/drafts.md`, "Leaving a composer").
//!
//! The composer is an inline pane, so a click on another message stays reachable while you write.
//! Leaving it that way, or by closing a composer window, keeps the draft and asks nothing: a
//! composer something was written in is saved and closed in one call, and one nothing was
//! written in is simply closed. Discard is the composer's own button, and asks first only when
//! there is something to lose.
//!
//! The dirtiness rule is every client's: header fields are compared against what the composer
//! **opened** with, and the body against the seed captured once the quote and signature were in.
//! A reply nobody typed into is not a draft. The comparison happens here and yields one boolean:
//! the document is never logged or stored by it (`docs/composer-security.md`).
//!
//! The body half needs a round trip because the editor has no bridge back into the host (that is
//! a security gate, not an oversight), so the host reads the document the same way Send does.
//!
//! The question Discard asks, the window with Discard and Keep editing in it, is
//! [`super::composer_discard`].

use std::{cell::RefCell, rc::Rc};

use gtk::{gio, prelude::EditableExt};
use mailcal_bindings::{ComposeRequest as CoreComposeRequest, Intent};
use webkit6::prelude::WebViewExt;

use super::{
    AppInput, AppModel, PrimaryView,
    composer_fields::{ComposerFields, read_document},
    composer_model::{ComposeContext, ComposerSubmission},
    model::OpenedMessage,
    reader::ComposerHost,
};

/// The four header fields the guard compares, in one value so the comparison reads as one rule.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(crate) struct HeaderValues {
    pub(crate) to: String,
    pub(crate) cc: String,
    pub(crate) bcc: String,
    pub(crate) subject: String,
}

impl HeaderValues {
    /// What the header fields show, read the same way when the composer opens and when it is
    /// asked again.
    ///
    /// Never the request the composer was opened with: a seeded recipient field normalises what
    /// it was handed, so one opened with `ada@example.test` shows `ada@example.test, `, and a
    /// baseline taken from the request would count every reply as written in.
    pub(crate) fn on_screen(fields: &ComposerFields) -> Self {
        Self {
            to: fields.to.text(),
            cc: fields.cc.text(),
            bcc: fields.bcc.text(),
            subject: fields.subject.text().to_string(),
        }
    }
}

/// A surface change waiting for the open composer to be left: saved and closed, or closed.
pub(crate) enum PendingNavigation {
    Message(OpenedMessage),
    Composer(ComposeContext),
}

/// Whether the header fields hold anything the user put there: the half of "is there a draft to
/// lose?" that needs no round trip into the editor.
///
/// Compared against what the composer opened with, never against empty: the core pre-fills a
/// reply's To and a reply-all's Cc. Stopping someone to ask about a message they never typed into
/// is exactly the noise this guard must not create. Typing something and deleting it again lands
/// back on the opening values and counts as clean, which is true; there is nothing left to lose.
///
/// The attachment count is measured the same way, against `opened_with`, which is **not** simply
/// what the composer opened holding. A forward's staged originals are still in the mailbox, so
/// abandoning one loses nothing and must not be worth a prompt; a share's files the user chose in
/// their file manager and would have to share again, so those count from the start. Removing a
/// forwarded file, like adding any file, changes the count and does count.
pub(crate) fn headers_edited(
    current: &HeaderValues,
    opening: &HeaderValues,
    attachments: usize,
    opened_with: usize,
) -> bool {
    current != opening || attachments != opened_with
}

/// Whether the editor holds anything beyond what was seeded into it.
pub(crate) fn body_edited(seed: Option<&str>, current: &str) -> bool {
    // The seed is read the moment the seeding script returns, so its absence means that read
    // failed, not that the draft is empty. Count it as written: a save or a question costs
    // little, and the alternative is losing what was typed.
    seed.is_none_or(|seed| seed != current)
}

/// The live draft, held by the composer pane for as long as one is open.
pub(crate) struct DraftGuard {
    editor: webkit6::WebView,
    fields: ComposerFields,
    opening: HeaderValues,
    /// How many of the files the composer opened holding are **not** the user's own work: a
    /// forward's staged originals, which are still in the mailbox. The baseline the live count is
    /// measured against. Zero for a share, whose files the user did choose and would have to
    /// share again.
    opening_files: usize,
    seed: Rc<RefCell<Option<String>>>,
}

impl DraftGuard {
    pub(crate) fn new(
        editor: webkit6::WebView,
        fields: ComposerFields,
        opening: HeaderValues,
        opening_files: usize,
        seed: Rc<RefCell<Option<String>>>,
    ) -> Self {
        Self {
            editor,
            fields,
            opening,
            opening_files,
            seed,
        }
    }

    /// Leaves the composer: one something was written in reports its whole message, to be saved
    /// and closed in one call; one nothing was written in reports that it can simply close.
    ///
    /// A document that cannot be read is closed rather than kept open: leaving is never something
    /// a composer refuses, and the autosaved copy is still in Drafts.
    pub(crate) fn leave(&self, sender: &relm4::Sender<AppInput>) {
        let host = self.fields.request.host;
        let editor = self.editor.clone();
        let fields = self.fields.clone();
        let sender = sender.clone();
        self.edited(move |edited| {
            if !edited {
                sender.emit(AppInput::ComposerUntouched(host));
                return;
            }
            let unread = sender.clone();
            read_document(
                &editor,
                &fields,
                &sender,
                AppInput::LeaveComposer,
                move || {
                    unread.emit(AppInput::ComposerUntouched(host));
                },
            );
        });
    }

    /// The composer's Discard button: reports whether anything was written, which with whether a
    /// copy is in Drafts decides whether Discard asks first.
    pub(crate) fn discard(&self, sender: &relm4::Sender<AppInput>) {
        let host = self.fields.request.host;
        let sender = sender.clone();
        self.edited(move |edited| sender.emit(AppInput::DiscardComposer(host, edited)));
    }

    /// The half of [`edited`](Self::edited) the header fields answer on their own.
    pub(crate) fn header_edited(&self) -> bool {
        headers_edited(
            &HeaderValues::on_screen(&self.fields),
            &self.opening,
            self.fields.files.borrow().len(),
            self.opening_files,
        )
    }

    /// Answers "has anything been written here?", handing `answer` the one boolean.
    ///
    /// The header half settles it on its own when it is dirty, so a draft with a typed recipient
    /// never pays for the round trip.
    fn edited(&self, answer: impl FnOnce(bool) + 'static) {
        if self.header_edited() {
            answer(true);
            return;
        }
        let seed = Rc::clone(&self.seed);
        self.editor.evaluate_javascript(
            "composerDocument()",
            None,
            None,
            None::<&gio::Cancellable>,
            move |result| {
                answer(match result {
                    Ok(value) => body_edited(seed.borrow().as_deref(), value.to_str().as_ref()),
                    // A failed read cannot say the draft is empty; as above.
                    Err(_) => true,
                });
            },
        );
    }
}

impl AppModel {
    /// Opens a message, leaving the open composer first.
    ///
    /// The composer is an inline pane, so this click stays reachable while the user writes.
    /// Whether it was written in cannot be answered synchronously (the editor holds the body and
    /// has no bridge back), so the requested navigation waits until the composer reports.
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
        self.clear_pane_composer();
        self.primary = PrimaryView::Mail;
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
        let request = ComposeContext::from_mailto(prefill, initial_from);
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
        let request = ComposeContext::from_agent(draft, initial_from);
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

    pub(super) fn commit_composer(&mut self, request: ComposeContext) {
        self.primary = PrimaryView::Mail;
        self.composer_generation = self.composer_generation.wrapping_add(1);
        // The composer being replaced is gone, whatever takes its place.
        self.clear_pane_composer();
        self.composer = Some(request);
    }

    /// A composer being left held nothing written: it closes, and the navigation happens.
    ///
    /// The pane's composer is closed by the navigation itself, which replaces it. With no
    /// navigation waiting it is left alone: the answer belongs to a request already taken.
    pub(super) fn composer_untouched(&mut self, host: ComposerHost) {
        match host {
            ComposerHost::Pane => {
                self.draft_check = None;
                self.take_pending_navigation();
            }
            ComposerHost::Window(id) => self.close_composer_window(id),
        }
    }

    /// A composer being left was written in: it is saved and closed in one call, then the
    /// navigation happens.
    ///
    /// The composition is not closed again here, or anywhere after: the core forgets it once the
    /// save has settled, and a close sent beside the save could land first and leave the server
    /// two copies. An answer for a composer no longer open is dropped, so a second click while
    /// the first was being read cannot save twice.
    pub(super) fn leave_composer(&mut self, submission: &ComposerSubmission) {
        let composition = &submission.request.composition;
        match submission.request.host {
            ComposerHost::Pane => {
                self.draft_check = None;
                if self
                    .composer
                    .as_ref()
                    .is_some_and(|open| &open.composition == composition)
                {
                    self.save_draft_and_close(submission);
                    self.composer = None;
                    self.composer_error = None;
                }
                self.take_pending_navigation();
            }
            ComposerHost::Window(id) => {
                if self
                    .composer_windows
                    .iter()
                    .any(|draft| draft.id == id && &draft.request.composition == composition)
                {
                    self.save_draft_and_close(submission);
                    self.forget_composer_window(id);
                }
            }
        }
    }

    pub(super) fn take_pending_navigation(&mut self) {
        match self.pending_navigation.take() {
            Some(PendingNavigation::Message(message)) => self.commit_open(message),
            Some(PendingNavigation::Composer(request)) => self.commit_composer(request),
            None => {}
        }
    }

    /// Takes a message the core withdrew from the Outbox into this client's composer.
    ///
    /// Dismisses the request as soon as it is held here, which is what the core waits for: it
    /// keeps the offer standing precisely because the message exists nowhere else until a host
    /// answers. Leaving the open composer refuses no navigation, so the withdrawn message always
    /// reaches the pane (`docs/sending.md`).
    pub(super) fn open_withdrawn_message(&mut self, request: CoreComposeRequest) {
        let context = ComposeContext::reopening(request);
        if self.composer.is_some() {
            self.queue_navigation(PendingNavigation::Composer(context));
        } else {
            self.commit_composer(context);
        }
        self.dispatch(Intent::DismissComposeRequest);
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
            0,
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
                headers_edited(&edited, &reply_opened_with(), 0, 0),
                "{edited:?} should count as edited"
            );
        }
    }

    /// A file the user attached is work that would be lost even with every field untouched.
    #[test]
    fn an_attachment_alone_is_a_draft() {
        assert!(headers_edited(
            &reply_opened_with(),
            &reply_opened_with(),
            1,
            0
        ));
    }

    /// A forward opens holding the original's files, and abandoning it loses nothing: they are
    /// still in the mailbox. Measured against zero instead, every forward the user thought better
    /// of would stop them with a prompt about work they never did.
    #[test]
    fn a_forward_nobody_touched_is_not_a_draft_for_carrying_the_originals_files() {
        assert!(!headers_edited(
            &reply_opened_with(),
            &reply_opened_with(),
            2,
            2
        ));
        // Taking one off is a decision about what goes out, and it would be lost.
        assert!(headers_edited(
            &reply_opened_with(),
            &reply_opened_with(),
            1,
            2
        ));
    }

    /// Typing and deleting again leaves nothing to lose.
    #[test]
    fn returning_to_the_opening_values_is_clean_again() {
        let typed = HeaderValues {
            to: "bob@test.local".to_owned(),
            ..reply_opened_with()
        };
        assert!(headers_edited(&typed, &reply_opened_with(), 0, 0));
        assert!(!headers_edited(
            &reply_opened_with(),
            &reply_opened_with(),
            0,
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
