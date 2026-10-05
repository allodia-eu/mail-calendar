//! Opening a message that already exists in a composer that saves over it: the one path a draft
//! resumed from Drafts ([`super::resume`]) and a queued send moved back out of the Outbox
//! ([`super::from_outbox`]) share.
//!
//! A child of [`super`], whose `impl App` block this one continues.
//!
//! **Everything the composer does not open with, its next save deletes** (`docs/drafts.md`). So
//! the composer is handed all of the message the editor can hold, and the composition keeps what
//! the editor cannot: the conversation the message answers, and the parts its quoted original's
//! pictures came from. Both are put back on every save and on the send.

use std::sync::Arc;

use engine_api::{AccountId, Draft, InlinePart, MessageIdHeader, Provider, ProviderKey};

use super::{Composition, Threading};
use crate::{App, ComposeRequest, CompositionId, protocol::StagedAttachment};

/// A message about to be opened in a composer, gathered by whichever path found it.
pub(super) struct Reopening {
    /// The account whose Drafts folder the message is, or is about to be, in.
    pub(super) account: AccountId,
    /// The header every save of the composition carries.
    pub(super) message_id: MessageIdHeader,
    /// What the server stores it under now, when it is already stored.
    pub(super) key: Option<ProviderKey>,
    /// The message it answers, when it is a reply.
    pub(super) threading: Option<Threading>,
    /// The pictures it carries as parts, `cid:`-addressed from its HTML.
    pub(super) pictures: Vec<InlinePart>,
    /// The `To`, `Cc` and `Bcc` fields, each comma-joined.
    pub(super) to: String,
    pub(super) cc: String,
    pub(super) bcc: String,
    pub(super) subject: String,
    /// Its `text/html`, unsanitised, or `None` when it has none.
    pub(super) html: Option<String>,
    pub(super) text: String,
    /// Its files, already staged where the host's composer reads them.
    pub(super) attachments: Vec<StagedAttachment>,
}

impl<P: Provider> App<P> {
    /// Joins `composition` to the message and answers with the request its composer opens from.
    ///
    /// Resuming into an id that is already open replaces what was known about it, which is
    /// what a host reusing an id means by it.
    pub(super) fn open_composition(
        &self,
        composition: &CompositionId,
        reopening: Reopening,
    ) -> ComposeRequest {
        let Reopening {
            account,
            message_id,
            key,
            threading,
            pictures,
            to,
            cc,
            bcc,
            subject,
            html,
            text,
            attachments,
        } = reopening;
        let body_html = html
            .filter(|html| !html.trim().is_empty())
            .map(|html| crate::html::sanitize_for_composer(&html, &pictures))
            .unwrap_or_default();
        let mut state = self.drafts.lock().expect("drafts mutex poisoned");
        // A composer that has just opened has saved nothing, whatever a composition of this
        // name did before it: the hint is per composer, and one reading "Saved" on a draft
        // this composer has not touched is the state of the one before it
        // (`docs/reading-window.md`).
        state.status.remove(composition);
        state.open.insert(
            composition.clone(),
            Composition {
                key,
                threading,
                pictures: Arc::from(pictures),
                ..Composition::new(account.clone(), message_id)
            },
        );
        ComposeRequest {
            account: account.as_str().to_owned(),
            composition: composition.as_str().to_owned(),
            to,
            cc,
            bcc,
            subject,
            body_html,
            body_text: text,
            attachments,
        }
    }

    /// Puts back on a draft built for `composition` what its composer could not hold: the
    /// conversation the message answers.
    pub(crate) fn with_threading(
        &self,
        composition: Option<&CompositionId>,
        draft: Draft,
    ) -> Draft {
        let threading = composition.and_then(|composition| {
            self.drafts
                .lock()
                .expect("drafts mutex poisoned")
                .open
                .get(composition)
                .and_then(|open| open.threading.clone())
        });
        match threading {
            Some(threading) => draft.in_reply_to(threading.in_reply_to, threading.references),
            None => draft,
        }
    }

    /// The parts the pictures in `composition`'s quoted original came from: empty for a
    /// composition that was not reopened on a message.
    pub(crate) fn composition_pictures(
        &self,
        composition: Option<&CompositionId>,
    ) -> Arc<[InlinePart]> {
        composition
            .and_then(|composition| {
                self.drafts
                    .lock()
                    .expect("drafts mutex poisoned")
                    .open
                    .get(composition)
                    .map(|open| Arc::clone(&open.pictures))
            })
            .unwrap_or_else(|| Arc::from([]))
    }
}

/// The conversation a stored message answers, read off its headers: its `In-Reply-To`, and
/// its `References`.
pub(super) fn threading_of(
    in_reply_to: Option<&MessageIdHeader>,
    references: &[MessageIdHeader],
) -> Option<Threading> {
    in_reply_to.map(|in_reply_to| Threading {
        in_reply_to: in_reply_to.clone(),
        references: references.to_vec(),
    })
}
