//! Joining a composition to the message its composer opens on: the one step every way into a
//! composer ends with ([`super::open`], and the Outbox's Edit, [`super::from_outbox`]).
//!
//! A child of [`super`], whose `impl App` block this one continues.
//!
//! **Everything the composer does not open with, its next save deletes** (`docs/drafts.md`). So
//! the composer is handed all of the message the editor can hold, and the composition keeps what
//! the editor cannot: the conversation the message belongs to, and the parts its quoted original's
//! pictures came from. Both are put back on every save and on the send, which is what makes a reply
//! saved, closed and resumed the same reply it was when it opened.

use std::sync::Arc;

use engine_api::{AccountId, Draft, InlinePart, MessageIdHeader, Provider, ProviderKey};

use super::{Composition, Threading};
use crate::{
    App, ComposeKind, ComposeOpening, CompositionId, QuoteSource, protocol::StagedAttachment,
};

/// A composer about to open, gathered by whichever path found what it opens on.
pub(super) struct Opening {
    /// The account the message goes out from, and whose Drafts folder its saves go to.
    pub(super) account: AccountId,
    /// The header every save of the composition carries.
    pub(super) message_id: MessageIdHeader,
    /// What the server stores it under now, when it is already stored.
    pub(super) key: Option<ProviderKey>,
    pub(super) kind: ComposeKind,
    /// The conversation it belongs to.
    pub(super) threading: Option<Threading>,
    /// The parts the pictures in its quoted original came from, `cid:`-addressed from its HTML.
    pub(super) pictures: Vec<InlinePart>,
    /// The `To`, `Cc` and `Bcc` fields, each comma-joined.
    pub(super) to: String,
    pub(super) cc: String,
    pub(super) bcc: String,
    pub(super) subject: String,
    /// A stored message's `text/html`, unsanitised; `None` when it has none or is not stored.
    pub(super) html: Option<String>,
    pub(super) text: String,
    pub(super) quote: Option<QuoteSource>,
    /// Its files, already staged where the host's composer reads them.
    pub(super) attachments: Vec<StagedAttachment>,
    pub(super) attachments_failed: bool,
    pub(super) seeds_signature: bool,
}

impl<P: Provider> App<P> {
    /// Joins `composition` to the message and answers with what its composer opens from.
    ///
    /// Opening into an id that is already open replaces what was known about it, which is what
    /// a host reusing an id means by it.
    pub(super) fn open_composition(
        &self,
        composition: &CompositionId,
        opening: Opening,
    ) -> ComposeOpening {
        let Opening {
            account,
            message_id,
            key,
            kind,
            threading,
            pictures,
            to,
            cc,
            bcc,
            subject,
            html,
            text,
            quote,
            attachments,
            attachments_failed,
            seeds_signature,
        } = opening;
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
                kind,
                ..Composition::new(account.clone(), message_id)
            },
        );
        ComposeOpening {
            composition: composition.as_str().to_owned(),
            kind,
            account: account.as_str().to_owned(),
            to,
            cc,
            bcc,
            subject,
            body_html,
            body_text: text,
            quote,
            attachments,
            attachments_failed,
            seeds_signature,
        }
    }

    /// Puts on a draft built for `composition` what its composer could not hold: the conversation
    /// the message belongs to.
    pub(crate) fn with_threading(
        &self,
        composition: Option<&CompositionId>,
        draft: Draft,
    ) -> Draft {
        let threading = self
            .joined(composition, |open| open.threading.clone())
            .flatten();
        match threading {
            Some(Threading {
                in_reply_to: Some(parent),
                references,
            }) => draft.in_reply_to(parent, references),
            Some(Threading {
                in_reply_to: None,
                references,
            }) => draft.with_references(references),
            None => draft,
        }
    }

    /// What `composition`'s composer is doing: a new message for one the core was never told of.
    pub(crate) fn composition_kind(&self, composition: Option<&CompositionId>) -> ComposeKind {
        self.joined(composition, |open| open.kind)
            .unwrap_or(ComposeKind::New)
    }

    /// The parts the pictures in `composition`'s quoted original came from: empty for a
    /// composition that quotes nothing.
    pub(crate) fn composition_pictures(
        &self,
        composition: Option<&CompositionId>,
    ) -> Arc<[InlinePart]> {
        self.joined(composition, |open| Arc::clone(&open.pictures))
            .unwrap_or_else(|| Arc::from([]))
    }

    /// `read` of what the core knows of `composition`, or `None` for a composition it was never
    /// told of: the one lock and lookup every accessor of an open composer goes through.
    pub(super) fn joined<T>(
        &self,
        composition: Option<&CompositionId>,
        read: impl FnOnce(&super::state::Composition) -> T,
    ) -> Option<T> {
        let composition = composition?;
        self.drafts
            .lock()
            .expect("drafts mutex poisoned")
            .open
            .get(composition)
            .map(read)
    }
}

/// The conversation a stored message belongs to, read off its headers: its `In-Reply-To`, and
/// its `References`.
pub(super) fn threading_of(
    in_reply_to: Option<&MessageIdHeader>,
    references: &[MessageIdHeader],
) -> Option<Threading> {
    (in_reply_to.is_some() || !references.is_empty()).then(|| Threading {
        in_reply_to: in_reply_to.cloned(),
        references: references.to_vec(),
    })
}

/// What a stored message was. One in a conversation is a forward when its subject says so, or when
/// it only continues the conversation without answering a message; otherwise one that answers a
/// message is a reply. No header says which reply it was.
///
/// The subject is asked because the headers alone cannot tell: Thunderbird and Outlook both set
/// `In-Reply-To` on a forward, to the message forwarded. Only a message already in a conversation
/// is asked, so a new message whose subject happens to open with one of these words stays new.
pub(super) fn kind_of(threading: Option<&Threading>, subject: &str) -> ComposeKind {
    match threading {
        Some(_) if opens_as_forward(subject) => ComposeKind::Forward,
        Some(Threading {
            in_reply_to: Some(_),
            ..
        }) => ComposeKind::Reply,
        Some(_) => ComposeKind::Forward,
        None => ComposeKind::New,
    }
}

/// The prefixes a forward's subject opens with in the clients a draft may come from: `Fwd:` from
/// this app and Thunderbird, `FW:` from Outlook, and Outlook's own translations of it.
const FORWARD_PREFIXES: [&str; 7] = ["fwd", "fw", "wg", "tr", "rv", "enc", "doorst"];

fn opens_as_forward(subject: &str) -> bool {
    subject.split_once(':').is_some_and(|(prefix, _)| {
        FORWARD_PREFIXES.contains(&prefix.trim().to_ascii_lowercase().as_str())
    })
}

#[cfg(test)]
mod tests {
    use engine_api::MessageIdHeader;

    use super::{ComposeKind, Threading, kind_of};

    fn threading(answers: bool) -> Threading {
        let parent = MessageIdHeader::new("parent@remote").unwrap();
        Threading {
            in_reply_to: answers.then(|| parent.clone()),
            references: vec![parent],
        }
    }

    #[test]
    fn a_stored_message_is_the_kind_its_headers_and_subject_say() {
        assert_eq!(
            kind_of(Some(&threading(true)), "Re: Plans"),
            ComposeKind::Reply
        );
        assert_eq!(
            kind_of(Some(&threading(false)), "Plans"),
            ComposeKind::Forward
        );
        assert_eq!(kind_of(None, "Plans"), ComposeKind::New);
    }

    /// Thunderbird and Outlook both answer the forwarded message in `In-Reply-To`.
    #[test]
    fn a_forward_another_client_stored_is_a_forward() {
        assert_eq!(
            kind_of(Some(&threading(true)), "Fwd: Plans"),
            ComposeKind::Forward
        );
        assert_eq!(
            kind_of(Some(&threading(true)), "FW: Plans"),
            ComposeKind::Forward
        );
        assert_eq!(
            kind_of(Some(&threading(true)), "WG: Pläne"),
            ComposeKind::Forward
        );
    }

    #[test]
    fn only_a_message_in_a_conversation_is_asked_its_subject() {
        assert_eq!(kind_of(None, "TR: quarterly numbers"), ComposeKind::New);
        assert_eq!(
            kind_of(Some(&threading(true)), "Re: Fwd: Plans"),
            ComposeKind::Reply
        );
    }
}
