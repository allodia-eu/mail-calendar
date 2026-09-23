//! Keeping learning honest: every draft handed out is remembered for the session, and a reply
//! sent from one is logged with what the person changed (`docs/ai.md`, "Keeps learning").
//!
//! The composer carries the draft's id back on submit (`ai_draft` in its document), and the reply
//! path hands it here with the text the person actually sent above the signature and the quote. The
//! sent message's `Message-ID` goes into the log, which keeps it out of every later learning run:
//! a style learned from a model's words would drift towards the model.
//!
//! A draft id the session did not issue, one issued before a restart, or a draft saved and
//! resumed later, logs nothing: the reply is then treated as the person's own.

use std::{collections::VecDeque, path::PathBuf, sync::Mutex};

use engine_api::Provider;
use mailcal_account::{
    AiAssistedSend, WritingStyleId, WritingStyleObservations, load_writing_style_observations,
    save_writing_style_observations,
};
use mailcal_composer::{Block, ComposerDocument};

use crate::App;

/// The most drafts remembered at once; an older one sent later counts as the person's own.
const ISSUED_CAP: usize = 32;

/// One draft handed to a composer.
struct Issued {
    id: String,
    style: WritingStyleId,
    language: String,
    text: String,
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
    pub(super) fn issue(&self, style: WritingStyleId, language: String, text: String) -> String {
        let id = super::random_id();
        let mut issued = self.issued.lock().expect("issued drafts poisoned");
        issued.push_back(Issued {
            id: id.clone(),
            style,
            language,
            text,
        });
        while issued.len() > ISSUED_CAP {
            issued.pop_front();
        }
        id
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
    /// quote, `message_id` the `Message-ID` it went out under. A draft this session did not issue
    /// logs nothing.
    pub(crate) fn note_ai_draft_sent(
        &self,
        account: &str,
        draft_id: &str,
        sent: &str,
        message_id: &str,
    ) {
        let observed = &self.writing_style.observed;
        let issued = {
            let mut issued = observed.issued.lock().expect("issued drafts poisoned");
            let Some(at) = issued.iter().position(|draft| draft.id == draft_id) else {
                return;
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
                message_id: message_id.to_owned(),
                account: account.to_owned(),
                style: issued.style.as_str().to_owned(),
                language: issued.language,
                sent_at: time::OffsetDateTime::now_utc().unix_timestamp(),
                changed: correction.changed,
                added: correction.added,
                removed: correction.removed,
            });
        });
    }
}
