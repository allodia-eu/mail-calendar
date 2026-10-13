//! Opening a composer: one call for every way into one, answered with one record.
//!
//! A new message, a reply, a reply to all, a forward and a stored draft all open through
//! [`MailcalApp::open_composer`], and a queued send the user edits raises the same record
//! (`MailcalApp::compose_request`). A host therefore has one way to build its composer, and every
//! composer sends through [`MailcalApp::submit_composition`] on the composition it opened on
//! (`docs/drafts.md`).

use std::sync::Arc;

use mailcal_app::{
    ComposeKind as AppComposeKind, ComposeOpening as AppComposeOpening,
    ComposeOrigin as AppComposeOrigin, ComposePrefill as AppComposePrefill, CompositionId,
    QuoteSource as AppQuoteSource,
};

use crate::{
    ComposerFileAttachment, MailcalApp, MailcalError,
    composer::{message_ref, send_account},
};

/// What a composer is opened on.
#[derive(Clone, Debug, PartialEq, Eq, uniffi::Enum)]
pub enum ComposeOrigin {
    /// A new message.
    New {
        /// The account it is written from, or `None` for the core to decide: the selected
        /// account, else the default send account, else the first.
        account: Option<String>,
    },
    /// A reply.
    Reply {
        /// The account holding the message replied to.
        account: String,
        /// The message replied to.
        key: String,
        /// Whether everyone on it is answered rather than its sender alone.
        all: bool,
    },
    /// A forward, holding the original's files.
    Forward {
        /// The account holding the message forwarded.
        account: String,
        /// The message forwarded.
        key: String,
    },
    /// A draft already on the server, reopened into a composer that saves over it.
    Draft {
        /// The account whose Drafts folder holds it.
        account: String,
        /// The stored draft.
        key: String,
    },
}

/// What someone other than the user put in a composer before it opened: a mail link, a share, an
/// assistant's draft. Laid over what the origin opens with, field by field; an empty field leaves
/// the origin's alone.
#[derive(Clone, Debug, Default, PartialEq, Eq, uniffi::Record)]
pub struct ComposePrefill {
    /// The `To` field, comma-joined.
    pub to: String,
    /// The `Cc` field, comma-joined.
    pub cc: String,
    /// The `Bcc` field, comma-joined.
    pub bcc: String,
    /// The subject.
    pub subject: String,
    /// The body, as text: the editor assigns it as text, never as markup.
    pub body_text: String,
    /// Whether the body brings its own sign-off, so the account's signature is not seeded under
    /// it: an assistant's draft, whose author chose how to sign it.
    pub signed: bool,
    /// The account to write from, laid over the origin's: an assistant may answer a message in one
    /// account from another. `None` leaves the origin's.
    #[uniffi(default = None)]
    pub account: Option<String>,
}

/// What a composer is doing: its heading, and the signature slot it seeds from.
#[derive(Clone, Copy, Debug, PartialEq, Eq, uniffi::Enum)]
pub enum ComposeKind {
    /// A new message.
    New,
    /// A reply to the sender.
    Reply,
    /// A reply to everyone on the original.
    ReplyAll,
    /// A forward.
    Forward,
}

/// The message a reply or forward quotes, for the host to build the quote's attribution from in
/// the user's language: the one-line "On … wrote:" and the labelled header block both come from
/// it, so a quote can be switched between the two styles.
#[derive(Clone, Debug, PartialEq, Eq, uniffi::Record)]
pub struct QuoteSource {
    /// The sender, as the reading header names them.
    pub from: String,
    /// When it arrived, as an RFC 3339 instant in UTC; empty when unknown. Formatted by the host.
    pub date: String,
    /// The `To` recipients, as the reading header lists them.
    pub to: String,
    /// The `Cc` recipients, as the reading header lists them.
    pub cc: String,
    /// Its subject.
    pub subject: String,
    /// Its sanitised HTML, pictures inline; empty when it has none.
    pub body_html: String,
    /// Its text, what the quote holds when there is no HTML.
    pub body_plain: String,
}

/// A composer to open, on the composition the core has just joined to it.
///
/// One record for every way into a composer. Seed the editor from it in one order:
/// `body_html` (a stored message) through `setComposerBody`, else `quote` through
/// `setComposerQuote` with `body_text` as its `initial_text`, else `body_text` through
/// `setPlainText`; then the account's signature only when `seeds_signature`; then, for a stored
/// message with a `quote`, `completeComposerQuoteAttribution` with the attribution built from it.
#[derive(Clone, Debug, PartialEq, Eq, uniffi::Record)]
pub struct ComposeOpening {
    /// The composition the composer opens on. Its saves, its discard and its send all name it.
    pub composition: String,
    /// What the composer is doing.
    pub kind: ComposeKind,
    /// The account the message goes out from unless the user picks another; for a stored
    /// message, the account whose Drafts folder holds it.
    pub account: String,
    /// The `To` field, comma-joined.
    pub to: String,
    /// The `Cc` field, comma-joined.
    pub cc: String,
    /// The `Bcc` field, comma-joined.
    pub bcc: String,
    /// The subject.
    pub subject: String,
    /// A stored message's HTML, sanitised, its pictures inline: for `setComposerBody`. Empty
    /// for a message that is not stored yet.
    pub body_html: String,
    /// A stored message's text, or the text a prefill brought.
    pub body_text: String,
    /// The quoted original: a reply's or forward's, and a stored one's when the message it quotes
    /// is still here.
    pub quote: Option<QuoteSource>,
    /// The files the composer opens holding, already in the staging directory the host named.
    /// It must open holding all of them: a stored message's next save replaces it, so a file left
    /// out is taken off it.
    pub attachments: Vec<ComposerFileAttachment>,
    /// Whether a forward's originals could not be staged, so the composer opens saying so.
    pub attachments_failed: bool,
    /// Whether to seed the account's signature. A stored message already carries its own.
    pub seeds_signature: bool,
}

#[uniffi::export]
impl MailcalApp {
    /// Opens a composer on `origin` into `composition`, the host's own id for the composer it is
    /// about to show, used from then on by every save, the discard, the close and the send.
    ///
    /// **Open the composer with what this returns, not before it.** A forward's and a draft's
    /// files are staged into `staging_directory` before it answers, and a stored draft's composer
    /// must hold every one of them. `staging_directory` is the host's own private area, and the
    /// host owns what is left in it.
    ///
    /// **Call this off the UI thread.** It blocks on the internal runtime, and may fetch the
    /// message a reply quotes or the draft being resumed from the server.
    ///
    /// A reply carrying a prefill opens as a new message holding it when the message it answers
    /// cannot be found, so an assistant's words are never lost for want of the message they answer.
    ///
    /// # Errors
    ///
    /// Returns [`MailcalError::Engine`] if `composition` is blank, a reference or account id is
    /// malformed, the message cannot be resolved, there is no account to write from, or, for a
    /// draft, when it is not one, its body cannot be read or a file cannot be staged. Nothing is
    /// joined on an error: show it rather than opening an empty composer, which for a draft would
    /// save over it.
    #[uniffi::method(default(prefill = None))]
    pub fn open_composer(
        &self,
        composition: String,
        origin: ComposeOrigin,
        staging_directory: String,
        prefill: Option<ComposePrefill>,
    ) -> Result<ComposeOpening, MailcalError> {
        let composition = CompositionId::new(composition)
            .ok_or_else(|| MailcalError::Engine("composition id is blank".to_owned()))?;
        let origin = app_origin(origin)?;
        let prefill = prefill.map(app_prefill).transpose()?;
        let app = Arc::clone(&self.app);
        self.runtime
            .block_on(|| async move {
                app.open_composer(composition, origin, prefill, &staging_directory)
                    .await
            })
            .map(ComposeOpening::from)
            .map_err(MailcalError::Engine)
    }
}

fn app_origin(origin: ComposeOrigin) -> Result<AppComposeOrigin, MailcalError> {
    Ok(match origin {
        ComposeOrigin::New { account } => AppComposeOrigin::New {
            account: send_account(account)?,
        },
        ComposeOrigin::Reply { account, key, all } => AppComposeOrigin::Reply {
            message: message_ref(&account, key)?,
            all,
        },
        ComposeOrigin::Forward { account, key } => AppComposeOrigin::Forward {
            message: message_ref(&account, key)?,
        },
        ComposeOrigin::Draft { account, key } => AppComposeOrigin::Draft {
            message: message_ref(&account, key)?,
        },
    })
}

fn app_prefill(prefill: ComposePrefill) -> Result<AppComposePrefill, MailcalError> {
    Ok(AppComposePrefill {
        to: prefill.to,
        cc: prefill.cc,
        bcc: prefill.bcc,
        subject: prefill.subject,
        body_text: prefill.body_text,
        signed: prefill.signed,
        account: send_account(prefill.account)?,
    })
}

impl From<AppComposeKind> for ComposeKind {
    fn from(kind: AppComposeKind) -> Self {
        match kind {
            AppComposeKind::New => Self::New,
            AppComposeKind::Reply => Self::Reply,
            AppComposeKind::ReplyAll => Self::ReplyAll,
            AppComposeKind::Forward => Self::Forward,
        }
    }
}

impl From<AppQuoteSource> for QuoteSource {
    fn from(quote: AppQuoteSource) -> Self {
        Self {
            from: quote.from,
            date: quote.date,
            to: quote.to,
            cc: quote.cc,
            subject: quote.subject,
            body_html: quote.body_html,
            body_plain: quote.body_plain,
        }
    }
}

impl From<AppComposeOpening> for ComposeOpening {
    fn from(opening: AppComposeOpening) -> Self {
        Self {
            composition: opening.composition,
            kind: opening.kind.into(),
            account: opening.account,
            to: opening.to,
            cc: opening.cc,
            bcc: opening.bcc,
            subject: opening.subject,
            body_html: opening.body_html,
            body_text: opening.body_text,
            quote: opening.quote.map(QuoteSource::from),
            attachments: opening
                .attachments
                .into_iter()
                .map(|file| ComposerFileAttachment {
                    path: file.path,
                    file_name: file.file_name,
                    media_type: file.media_type,
                })
                .collect(),
            attachments_failed: opening.attachments_failed,
            seeds_signature: opening.seeds_signature,
        }
    }
}
