//! A message that already exists, to open in the host's composer.
//!
//! Two paths open one: a draft resumed from the Drafts folder
//! ([`App::resume_draft`](crate::App::resume_draft)), and a queued send the user moves back out of
//! the Outbox to edit. Both answer with this record, built in one place
//! (`draft_ops::reopen`), so a host has one way to open a composer on a message it did not write
//! this session, and the two cannot drift into opening different amounts of it.
//!
//! The Outbox one is raised rather than returned, shaped like the standing question in
//! [`unfiled_copy`](crate::unfiled_copy): the core puts it here and signals
//! [`Surface::ComposeRequest`](crate::Surface::ComposeRequest); the host pulls it, opens its
//! composer, and dismisses it. It does **not** auto-clear, because a client that was backgrounded
//! when the request was raised must still find it on return.

/// A message to open, unsent, in the host's composer, on the composition whose saves replace it.
///
/// The recipient fields are comma-joined rather than lists, matching the shape the composer
/// intents already take across the FFI: one representation of "a recipient field", not two.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ComposeRequest {
    /// The account whose Drafts folder holds the message, and which its saves and its send go
    /// through.
    pub account: String,
    /// The composition the message is joined to. The composer opens on it, so its saves
    /// replace the stored copy and its send takes that copy away (`docs/drafts.md`).
    pub composition: String,
    /// The `To` field, comma-joined.
    pub to: String,
    /// The `Cc` field, comma-joined.
    pub cc: String,
    /// The `Bcc` field, comma-joined. Only what the stored copy carries: most transports do
    /// not hand a `Bcc` back, so a draft saved elsewhere usually opens without one.
    pub bcc: String,
    /// The subject.
    pub subject: String,
    /// The body's HTML, for the editor's `setComposerBody`: sanitised as a reading view's is,
    /// with every picture the message carries as a part already turned into the `data:` URI
    /// the editor shows it from. Empty when the message has no HTML.
    ///
    /// The editor reads it back into its own document (`clients/composer/src/read_html.ts`),
    /// so a message this app wrote opens exactly as it was written: formatting, pictures, the
    /// quoted original and the signature.
    pub body_html: String,
    /// The body as text: what the editor opens when there is no HTML.
    pub body_text: String,
    /// The message's files, already written into the staging directory the host named,
    /// ready to be attached exactly as a picked file is.
    pub attachments: Vec<crate::protocol::StagedAttachment>,
}
