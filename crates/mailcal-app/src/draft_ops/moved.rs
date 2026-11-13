//! Moving an open composer to another view of it: the inline composer popped out into a window of
//! its own (`docs/reading-window.md`, "Popping a composer out").
//!
//! A child of [`super`], whose `impl App` block this one continues.
//!
//! **The composition does not move; only its view does.** The core already holds everything about
//! the message the editor cannot: the key its last save is stored under, the conversation it
//! belongs to, the parts its quote's pictures came from. So the new view opens on the same
//! composition, and its first save supersedes what the old view saved rather than storing a second
//! copy. What the core does not hold is what only the host held, and that is what
//! [`ComposerState`] carries across. The files are the composition's, so they stay where they are.

use engine_api::{AccountId, Provider};
use mailcal_composer::ComposerDocument;

use crate::{App, ComposeOpening, CompositionId};

/// Everything a composer holds that its composition does not, at the moment it moves.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ComposerState {
    /// The From dropdown, or `None` where it follows the default.
    pub from: Option<AccountId>,
    /// The `To` field, comma-joined.
    pub to: String,
    /// The `Cc` field, comma-joined.
    pub cc: String,
    /// The `Bcc` field, comma-joined.
    pub bcc: String,
    /// The subject.
    pub subject: String,
    /// The editor's document, as its `composerDocument()` wrote it.
    pub document: String,
    /// What the composer was doing with the signature.
    pub signature: MovedSignature,
    /// Whether anything was written in it, by the host's own reckoning against what it opened
    /// with.
    pub written: bool,
}

/// What a composer moved from another view of it opens with, beyond its fields and its files: the
/// editor's own document, whether anything was written in it, and its signature choice
/// (`docs/reading-window.md`, "Popping a composer out").
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MovedComposer {
    /// The editor's document as the composer it left wrote it (`composerDocument()`), for the
    /// editor's `setComposerDocument`: the message exactly as it stood, quote and signature
    /// included.
    pub document: String,
    /// Whether anything was written in the composer it left. Leaving this one then keeps the draft
    /// even when nothing more is typed: its baseline is where it opened, which is not where the
    /// message started.
    pub written: bool,
    /// What the composer it left did with the signature, which this one carries on doing.
    pub signature: MovedSignature,
}

/// What a moved composer does with the message's signature: what the one it left was doing.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MovedSignature {
    /// Leaves the signature region alone, offering no picker: a stored message's, or an
    /// assistant's own sign-off (`ComposeOpening::seeds_signature` was false).
    Untouched,
    /// The From account's signature, swapped when the From changes.
    FollowsAccount,
    /// The user chose no signature for this message.
    NoSignature,
    /// The user chose this signature for this message, by id.
    Chosen(String),
}

impl<P: Provider> App<P> {
    /// The opening for a composer moved from another view onto `composition`, which stays as it
    /// is: not re-joined, its stored key and its save status kept.
    ///
    /// # Errors
    ///
    /// When `composition` is not open, because a composer opened on a composition the core does
    /// not know would save beside the draft its predecessor saved rather than over it; and when the
    /// document is not one, because the composer would open on an empty body and its next save
    /// would replace the message with nothing.
    pub(super) fn moved_composer(
        &self,
        composition: &CompositionId,
        state: ComposerState,
    ) -> Result<ComposeOpening, String> {
        let ComposerState {
            from,
            to,
            cc,
            bcc,
            subject,
            document,
            signature,
            written,
        } = state;
        serde_json::from_str::<ComposerDocument>(&document)
            .map_err(|_| "the moved composer's document is not one".to_owned())?;
        let drafts = self.drafts.lock().expect("drafts mutex poisoned");
        let open = drafts
            .open
            .get(composition)
            .ok_or_else(|| "the composer being moved is not open".to_owned())?;
        Ok(ComposeOpening {
            composition: composition.as_str().to_owned(),
            kind: open.kind,
            account: from
                .unwrap_or_else(|| open.account.clone())
                .as_str()
                .to_owned(),
            to,
            cc,
            bcc,
            subject,
            body_html: String::new(),
            body_text: String::new(),
            quote: None,
            // The files stay with the composition, which the move keeps
            // (`docs/composer-layout.md`, rule 3).
            attachments: Vec::new(),
            attachments_failed: false,
            seeds_signature: false,
            moved: Some(MovedComposer {
                document,
                written,
                signature,
            }),
        })
    }
}
