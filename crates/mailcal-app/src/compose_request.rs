//! What a composer opens with: one record for every way into one.
//!
//! A new message, a reply, a reply to all, a forward, a draft resumed from the Drafts folder and a
//! queued send moved back out of the Outbox all open the host's composer from a
//! [`ComposeOpening`], built in one place (`draft_ops::open`), so a host has one way to open a
//! composer and the flows cannot drift into opening different amounts of a message. The core joins
//! the composition at the same moment, with everything about the message the editor cannot hold:
//! the conversation it answers, and the parts its quoted original's pictures came from. Every save
//! and the send carry them, whichever way the composer was opened (`docs/drafts.md`).
//!
//! The Outbox one is raised rather than returned, shaped like the standing question in
//! [`unfiled_copy`](crate::unfiled_copy): the core puts it here and signals
//! [`Surface::ComposeRequest`](crate::Surface::ComposeRequest); the host pulls it, opens its
//! composer, and dismisses it. It does **not** auto-clear, because a client that was backgrounded
//! when the request was raised must still find it on return.

/// What the composer is doing: its heading, and the signature slot it seeds from.
///
/// A reopened message is told apart by its own standard headers: one that answers a message
/// (`In-Reply-To`) is a reply, one that only continues a conversation (`References`) a forward.
/// Which reply a stored one was is not something any header says, so it reopens as a reply.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
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

/// The message a reply or forward quotes, as the reading view shows it, for the host to build the
/// quote's attribution from in the user's language: the core carries no runtime locale, so the
/// words and the date format are the host's (`docs/timestamps.md`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QuoteSource {
    /// The sender, as the reading header names them.
    pub from: String,
    /// When it arrived, as an RFC 3339 instant in UTC (the form a mail row's date takes); empty
    /// when unknown.
    pub date: String,
    /// The `To` recipients, as the reading header lists them.
    pub to: String,
    /// The `Cc` recipients, as the reading header lists them.
    pub cc: String,
    /// Its subject.
    pub subject: String,
    /// Its HTML, sanitised as the reading view's is, with every picture it carries as a part
    /// turned into the `data:` URI the editor shows it from; empty when it has none.
    pub body_html: String,
    /// Its text: what the quote holds when there is no HTML.
    pub body_plain: String,
}

/// A composer to open, on the composition the core has just joined to it.
///
/// The recipient fields are comma-joined rather than lists, matching the shape the composer
/// intents already take across the FFI: one representation of "a recipient field", not two.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ComposeOpening {
    /// The composition the composer opens on, so its saves and its send are this message's.
    pub composition: String,
    /// What the composer is doing.
    pub kind: ComposeKind,
    /// The account the message goes out from unless the user picks another, and for a stored
    /// message the account whose Drafts folder holds it.
    pub account: String,
    /// The `To` field, comma-joined.
    pub to: String,
    /// The `Cc` field, comma-joined.
    pub cc: String,
    /// The `Bcc` field, comma-joined.
    pub bcc: String,
    /// The subject.
    pub subject: String,
    /// A stored message's HTML, for the editor's `setComposerBody`: sanitised as a reading view's
    /// is, with every picture it carries as a part already turned into the `data:` URI the editor
    /// shows it from, its `Content-ID` beside it. Empty for a message that is not stored yet, and
    /// for a stored one with no HTML.
    ///
    /// The editor reads it back into its own document (`clients/composer/src/read_html.ts`), so a
    /// message this app saved opens exactly as it was saved (`docs/drafts.md`).
    pub body_html: String,
    /// A stored message's text, what the editor opens when there is no HTML; or the text someone
    /// else put in a new message (a link, a share, an assistant).
    pub body_text: String,
    /// The quoted original: a reply's or forward's, and a reopened one's when the message it
    /// quotes is still in the store, so the host can complete the attribution the stored quote
    /// carries only one style of.
    pub quote: Option<QuoteSource>,
    /// The files the composer opens holding, already written into the staging directory the host
    /// named: a forward's originals, or a stored message's own.
    pub attachments: Vec<crate::protocol::StagedAttachment>,
    /// Whether a forward's originals could not be staged, so the composer opens saying so. An
    /// empty list alone would read as a message that had nothing attached.
    pub attachments_failed: bool,
    /// Whether the composer seeds the account's signature. A stored message already carries the
    /// one it was written with, and a second under it would be stored by the next save.
    pub seeds_signature: bool,
}
