//! Keeping learning honest: every draft handed out is remembered for the session, and a reply
//! sent from one is logged with what the person changed (`docs/ai.md`, "Keeps learning").
//!
//! The composer carries the draft's id back on submit (`ai_draft` in its document), and the reply
//! path hands it here with the text the person actually sent above the signature and the quote. The
//! sent message's `Message-ID` goes into the log, which keeps it out of every later learning run:
//! a style learned from a model's words would drift towards the model. The log is this device's;
//! the [`WRITING_ASSISTANT`] keyword on the filed Sent copy is what tells the person's other
//! devices, where the provider keeps one.
//!
//! A draft id the session did not issue, one issued before a restart, or a draft saved and
//! resumed later, logs nothing: the reply is then treated as the person's own.
//!
//! The same memory is what feedback on a draft is built from (`feedback.rs`): what made it, what
//! it said and the message it answered. It stays in memory and leaves only as feedback the person
//! gives.

use std::{collections::VecDeque, path::PathBuf, sync::Mutex};

use engine_api::{Draft, Keyword, Message, Provider};
use mailcal_account::{
    AiAssistedSend, WritingStyleId, WritingStyleObservations, load_writing_style_observations,
    save_writing_style_observations,
};
use mailcal_ai::DraftRecord;
use mailcal_composer::{Block, ComposerDocument};

use crate::App;

/// The keyword a reply sent from a draft carries on its filed Sent copy.
///
/// Fixed and untranslated: every device has to recognise it, whatever language it runs in.
pub(crate) const WRITING_ASSISTANT: &str = "writing-assistant";

/// Whether `message` carries [`WRITING_ASSISTANT`].
pub(crate) fn marked_as_drafted(message: &Message) -> bool {
    Keyword::new(WRITING_ASSISTANT).is_ok_and(|keyword| message.has_keyword(&keyword))
}

/// The most drafts remembered at once; an older one sent later counts as the person's own.
const ISSUED_CAP: usize = 32;

/// One draft handed to a composer.
struct Issued {
    id: String,
    style: WritingStyleId,
    /// The draft as the composer's plain text renders it, which a sent reply is compared with.
    text: String,
    /// What made it and what it said.
    record: DraftRecord,
    /// The body of the message it answered.
    message: String,
}

/// The drafts handed out this session, and the log of replies sent from them.
pub(super) struct Observed {
    issued: Mutex<VecDeque<Issued>>,
    log: Mutex<WritingStyleObservations>,
    path: Option<PathBuf>,
}

impl Observed {
    pub(super) fn new(path: Option<PathBuf>) -> Self {
        Self {
            issued: Mutex::new(VecDeque::new()),
            log: Mutex::new(
                path.as_ref()
                    .map(load_writing_style_observations)
                    .unwrap_or_default(),
            ),
            path,
        }
    }

    /// Remembers a draft and returns the id the composer carries back.
    pub(super) fn issue(
        &self,
        style: WritingStyleId,
        text: String,
        record: DraftRecord,
        message: String,
    ) -> String {
        let id = super::random_id();
        let mut issued = self.issued.lock().expect("issued drafts poisoned");
        issued.push_back(Issued {
            id: id.clone(),
            style,
            text,
            record,
            message,
        });
        while issued.len() > ISSUED_CAP {
            issued.pop_front();
        }
        id
    }

    /// What made the draft `draft_id`, what it said and the message it answered, while the session
    /// remembers it.
    pub(super) fn record_of(&self, draft_id: &str) -> Option<(DraftRecord, String)> {
        self.issued
            .lock()
            .expect("issued drafts poisoned")
            .iter()
            .find(|draft| draft.id == draft_id)
            .map(|draft| (draft.record.clone(), draft.message.clone()))
    }

    /// Whether the message with this `Message-ID` was sent from a draft.
    pub(super) fn is_assisted(&self, message_id: &str) -> bool {
        self.log
            .lock()
            .expect("observations poisoned")
            .is_assisted(message_id)
    }

    /// The log as it stands, for tests.
    #[cfg(test)]
    pub(super) fn sends(&self) -> Vec<AiAssistedSend> {
        self.log
            .lock()
            .expect("observations poisoned")
            .sends
            .clone()
    }

    pub(super) fn edit(&self, edit: impl FnOnce(&mut WritingStyleObservations)) {
        let mut log = self.log.lock().expect("observations poisoned");
        edit(&mut log);
        if let Some(path) = &self.path {
            let _ = save_writing_style_observations(path, &log);
        }
    }
}

/// The plain text of what the person wrote above the signature and the quote: the part of a reply
/// a draft stood in for.
pub(crate) fn lead_text(document: &ComposerDocument) -> String {
    let blocks = document
        .blocks
        .iter()
        .take_while(|block| !matches!(block, Block::Quote(_) | Block::Signature(_)))
        .cloned()
        .collect();
    mailcal_composer::render(&ComposerDocument {
        blocks,
        attachments: document.attachments.clone(),
    })
    .map(|output| output.plain_text)
    .unwrap_or_default()
}

impl<P: Provider> App<P> {
    /// Logs a reply sent from the draft `draft_id`: `sent` is the text above its signature and
    /// quote, and `draft` the message about to go out. Returns `draft` asking for
    /// [`WRITING_ASSISTANT`] on its Sent copy when it was logged; a draft this session did not
    /// issue logs nothing and comes back as it was.
    pub(crate) fn note_ai_draft_sent(
        &self,
        account: &str,
        draft_id: &str,
        sent: &str,
        draft: Draft,
    ) -> Draft {
        let observed = &self.writing_style.observed;
        let issued = {
            let mut issued = observed.issued.lock().expect("issued drafts poisoned");
            let Some(at) = issued.iter().position(|draft| draft.id == draft_id) else {
                return draft;
            };
            issued.remove(at).expect("the position was just found")
        };
        let correction = mailcal_ai::correction(&issued.text, sent);
        log::info!(
            "ai: a reply was sent from a draft; {} run(s) added, {} taken out",
            correction.added.len(),
            correction.removed.len()
        );
        observed.edit(|log| {
            log.record(AiAssistedSend {
                message_id: draft.message_id.as_str().to_owned(),
                account: account.to_owned(),
                style: issued.style.as_str().to_owned(),
                language: issued.record.language,
                sent_at: time::OffsetDateTime::now_utc().unix_timestamp(),
                changed: correction.changed,
                added: correction.added,
                removed: correction.removed,
            });
        });
        match Keyword::new(WRITING_ASSISTANT) {
            Ok(keyword) => draft.with_sent_copy_keyword(keyword),
            Err(_) => draft,
        }
    }
}
