//! Feedback on a drafted reply (`docs/ai.md`, "Feedback"): a rating kept in a local outbox, built
//! from what the session remembers of the draft, until Allodia's service has taken it.
//!
//! What goes in is the rating and what made the draft; the message answered and what the draft
//! said only when the person asked for them to be included. Never a header, the intent or the
//! style. Sending is the binding layer's, behind the jurisdiction gate; this keeps the outbox.

use std::{path::PathBuf, sync::Mutex};

use engine_api::Provider;
use mailcal_account::{
    AiFeedbackItem, AiFeedbackOutbox, load_ai_feedback_outbox, save_ai_feedback_outbox,
};
use mailcal_ai::{DraftExport, RatedDraft, Rating};

use crate::App;

/// The outbox and where it is kept.
pub(super) struct Outbox {
    items: Mutex<AiFeedbackOutbox>,
    path: Option<PathBuf>,
}

impl Outbox {
    pub(super) fn new(path: Option<PathBuf>) -> Self {
        Self {
            items: Mutex::new(
                path.as_ref()
                    .map(load_ai_feedback_outbox)
                    .unwrap_or_default(),
            ),
            path,
        }
    }

    /// Applies `edit` and writes the outbox back; returns how many items wait afterwards.
    fn edit(&self, edit: impl FnOnce(&mut AiFeedbackOutbox)) -> usize {
        let mut outbox = self.items.lock().expect("feedback outbox poisoned");
        edit(&mut outbox);
        if let Some(path) = &self.path
            && let Err(err) = save_ai_feedback_outbox(path, &outbox)
        {
            log::warn!("ai: the feedback waiting to be sent could not be saved; {err}");
        }
        outbox.items.len()
    }
}

impl<P: Provider> App<P> {
    /// Keeps `rating` of the draft `draft_id` in the outbox, with the message it answered and what
    /// it said when `include_content`. Returns whether this session issued that draft; nothing is
    /// kept when it did not.
    pub fn rate_draft(&self, draft_id: &str, rating: Rating, include_content: bool) -> bool {
        let Some((record, message)) = self.writing_style.observed.record_of(draft_id) else {
            return false;
        };
        let id = super::random_id();
        let (verdict, reasons) = (rating.verdict, rating.reasons.len());
        let document = DraftExport {
            id: Some(id.clone()),
            message: include_content.then_some(message),
            drafts: vec![RatedDraft {
                record: if include_content {
                    record
                } else {
                    record.without_content()
                },
                failure: None,
                rating: Some(rating),
            }],
        };
        let waiting = self.writing_style.feedback.edit(|outbox| {
            outbox.push(AiFeedbackItem {
                id,
                created_at: time::OffsetDateTime::now_utc().unix_timestamp(),
                with_content: include_content,
                body: document.to_json(),
            });
        });
        log::info!(
            "ai: feedback on a draft was kept: {}, {reasons} reason(s), {}; {waiting} waiting",
            verdict.label(),
            if include_content {
                "with the message and the draft"
            } else {
                "without the message or the draft"
            }
        );
        true
    }

    /// The feedback waiting to be sent, oldest first.
    #[must_use]
    pub fn ai_feedback_waiting(&self) -> Vec<AiFeedbackItem> {
        self.writing_style
            .feedback
            .items
            .lock()
            .expect("feedback outbox poisoned")
            .items
            .clone()
    }

    /// Takes the item `id` out of the outbox once Allodia's service has accepted it.
    pub fn ai_feedback_delivered(&self, id: &str) {
        self.writing_style.feedback.edit(|outbox| {
            outbox.remove(id);
        });
    }

    /// Empties the outbox: nothing is left to send once nobody is signed in to Allodia.
    pub fn forget_ai_feedback(&self) {
        let mut forgotten = 0;
        self.writing_style.feedback.edit(|outbox| {
            forgotten = outbox.items.len();
            outbox.items.clear();
        });
        if forgotten > 0 {
            log::info!("ai: {forgotten} piece(s) of feedback waiting to be sent were forgotten");
        }
    }
}
