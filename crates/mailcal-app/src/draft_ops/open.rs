//! Opening a composer: every way into one, answered with the same record.
//!
//! A child of [`super`], whose `impl App` block this one continues.
//!
//! A new message, a reply, a reply to all, a forward and a stored draft each gather what their
//! composer opens with here, and all of them end in the same join ([`super::reopen`]), which is
//! where the composition takes the conversation the message belongs to and the parts its quoted
//! original's pictures came from. Doing that when the composer opens, for every one of them, is
//! what lets a save carry them: a reply saved before it was ever sent is still a reply, and still
//! one when it is resumed (`docs/drafts.md`).

use engine_api::{AccountId, MessageIdHeader, Provider};

use super::reopen::Opening;
use crate::{
    App, ComposeKind, ComposeOpening, CompositionId, QuoteSource,
    draft_ops::Threading,
    helpers::{forward_subject, new_message_id, reply_subject},
    reference::MessageRef,
};

/// What a composer is opened on.
#[derive(Debug, Clone)]
pub enum ComposeOrigin {
    /// A new message.
    New {
        /// The account it is written from, or `None` for the core to decide: the selected
        /// account, else the default send account, else the first.
        account: Option<AccountId>,
    },
    /// A reply.
    Reply {
        /// The message replied to.
        message: MessageRef,
        /// Whether everyone on it is answered rather than its sender alone.
        all: bool,
    },
    /// A forward, holding the original's files.
    Forward {
        /// The message forwarded.
        message: MessageRef,
    },
    /// A draft already on the server, reopened into a composer that saves over it.
    Draft {
        /// The stored draft.
        message: MessageRef,
    },
}

/// What someone other than the user put in the composer before it opened: a mail link, a share,
/// an assistant's draft. Laid over what the origin opens with, field by field; an empty field
/// leaves the origin's alone.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
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
    /// it: an assistant's draft, whose author chose how to sign it (`docs/mcp.md`).
    pub signed: bool,
    /// The account to write from, laid over the origin's: an assistant may answer a message in one
    /// account from another.
    pub account: Option<AccountId>,
}

impl<P: Provider> App<P> {
    /// Opens a composer on `origin` into `composition`, and answers with what it opens with.
    ///
    /// `composition` is the host's, minted for the composer it is about to open: the core never
    /// mints one. Opening into an id that is already open replaces what was known about it.
    ///
    /// `staging_directory` is where a forward's or a draft's files are written for the composer to
    /// hold; the host owns what is left in it, as for an attachment opened from the reading view.
    ///
    /// A reply carrying a prefill opens as a new message holding it when the message it answers
    /// cannot be found: the prefill is words someone else wrote (an assistant, a link), and it is
    /// not lost for want of the message they answer. A reply the user started has no such words,
    /// and refuses.
    ///
    /// # Errors
    ///
    /// A plain error string when the message cannot be resolved, there is no account to compose
    /// from, or, for a draft, when it is not one, its content cannot be read or its files cannot
    /// be staged. Nothing is joined on an error: a composer that opened without a draft's content
    /// must not be one whose next save replaces it.
    pub async fn open_composer(
        &self,
        composition: CompositionId,
        origin: ComposeOrigin,
        prefill: Option<ComposePrefill>,
        staging_directory: &str,
    ) -> Result<ComposeOpening, String> {
        let mut opening = match origin {
            ComposeOrigin::New { account } => self.new_message(account).await?,
            ComposeOrigin::Reply { message, all } => match self.reply_to(&message, all).await {
                Ok(opening) => opening,
                Err(err) if prefill.is_some() => {
                    log::info!(
                        "composer: the message a prefilled reply answers cannot be found ({err}); \
                         it opens as a new message"
                    );
                    self.new_message(None).await?
                }
                Err(err) => return Err(err),
            },
            ComposeOrigin::Forward { message } => {
                self.forward_of(&message, staging_directory).await?
            }
            ComposeOrigin::Draft { message } => {
                self.stored_draft(&message, staging_directory).await?
            }
        };
        if let Some(prefill) = prefill {
            lay_over(&mut opening, prefill);
        }
        Ok(self.open_composition(&composition, opening))
    }

    async fn new_message(&self, account: Option<AccountId>) -> Result<Opening, String> {
        let account = match account {
            Some(account) => account,
            None => self
                .compose_account()
                .await
                .ok_or_else(|| "there is no account to write from".to_owned())?,
        };
        Ok(Opening {
            kind: ComposeKind::New,
            ..blank(account, fresh_message_id()?)
        })
    }

    async fn reply_to(&self, message: &MessageRef, all: bool) -> Result<Opening, String> {
        let original = self
            .find_message_in(message)
            .await
            .ok_or_else(|| "message is not available".to_owned())?;
        let recipients = self.reply_recipients_of(message, &original, all).await;
        let (quote, pictures) = self.quoted(message, &original).await;
        Ok(Opening {
            kind: if all {
                ComposeKind::ReplyAll
            } else {
                ComposeKind::Reply
            },
            threading: conversation_of(&original, true),
            pictures,
            to: recipients.to,
            cc: recipients.cc,
            subject: reply_subject(original.envelope.subject.as_deref()),
            quote,
            ..blank(message.account.clone(), fresh_message_id()?)
        })
    }

    async fn forward_of(&self, message: &MessageRef, staging: &str) -> Result<Opening, String> {
        let original = self
            .find_message_in(message)
            .await
            .ok_or_else(|| "message is not available".to_owned())?;
        let (quote, pictures) = self.quoted(message, &original).await;
        // A forward opens even when its originals cannot be staged, saying so: the words are still
        // worth sending, and the user decides whether to without the files.
        let staged = self.stage_attachments_of(message, &original, staging).await;
        let attachments_failed = staged.is_err();
        Ok(Opening {
            kind: ComposeKind::Forward,
            threading: conversation_of(&original, false),
            pictures,
            subject: forward_subject(original.envelope.subject.as_deref()),
            quote,
            attachments: staged.unwrap_or_default(),
            attachments_failed,
            ..blank(message.account.clone(), fresh_message_id()?)
        })
    }

    /// The message a composer quotes, as the reading view shows it, and the parts its pictures
    /// came from. `None` when its body cannot be read: the composer still opens, quoting nothing,
    /// rather than refusing to answer a message because it is slow to load.
    async fn quoted(
        &self,
        message: &MessageRef,
        original: &engine_api::Message,
    ) -> (Option<QuoteSource>, Vec<engine_api::InlinePart>) {
        let Some(quote) = self.quote_source(message, original).await else {
            return (None, Vec::new());
        };
        let pictures = if quote.body_html.contains("src=\"data:") {
            self.original_inline_parts(message, original).await
        } else {
            Vec::new()
        };
        (Some(quote), pictures)
    }

    /// The message a composer quotes, as the reading view shows it, without the parts its
    /// pictures came from: what a reopened message needs, whose own parts already carry them.
    pub(super) async fn quote_source(
        &self,
        message: &MessageRef,
        original: &engine_api::Message,
    ) -> Option<QuoteSource> {
        let reading = self
            .fetch_reading_of(message.clone(), original.clone())
            .await;
        if reading.load_error {
            return None;
        }
        Some(QuoteSource {
            from: reading.from,
            date: original
                .received_at
                .or(original.sent_at)
                .map_or_else(String::new, crate::query::format_instant),
            to: reading.to,
            cc: reading.cc,
            subject: original.envelope.subject.clone().unwrap_or_default(),
            body_html: reading.html.unwrap_or_default(),
            body_plain: reading.plain.unwrap_or_default(),
        })
    }
}

/// The conversation a reply or forward of `original` joins: the original's chain followed by the
/// original itself, and for a reply the original as the message it answers. `None` when the
/// original carries no `Message-ID` to name it by.
fn conversation_of(original: &engine_api::Message, answers: bool) -> Option<Threading> {
    let parent = original.envelope.message_id.first()?;
    let mut references = original.envelope.references.clone();
    references.push(parent.clone());
    Some(Threading {
        in_reply_to: answers.then(|| parent.clone()),
        references,
    })
}

fn fresh_message_id() -> Result<MessageIdHeader, String> {
    new_message_id().ok_or_else(|| "could not mint a Message-ID".to_owned())
}

/// A composer holding nothing yet, on `account`.
fn blank(account: AccountId, message_id: MessageIdHeader) -> Opening {
    Opening {
        account,
        message_id,
        key: None,
        kind: ComposeKind::New,
        threading: None,
        pictures: Vec::new(),
        to: String::new(),
        cc: String::new(),
        bcc: String::new(),
        subject: String::new(),
        html: None,
        text: String::new(),
        quote: None,
        attachments: Vec::new(),
        attachments_failed: false,
        seeds_signature: true,
    }
}

fn lay_over(opening: &mut Opening, prefill: ComposePrefill) {
    let ComposePrefill {
        to,
        cc,
        bcc,
        subject,
        body_text,
        signed,
        account,
    } = prefill;
    if signed {
        opening.seeds_signature = false;
    }
    if let Some(account) = account {
        opening.account = account;
    }
    for (field, value) in [
        (&mut opening.to, to),
        (&mut opening.cc, cc),
        (&mut opening.bcc, bcc),
        (&mut opening.subject, subject),
        (&mut opening.text, body_text),
    ] {
        if !value.trim().is_empty() {
            *field = value;
        }
    }
}
