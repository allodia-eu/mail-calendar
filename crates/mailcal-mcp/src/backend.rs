//! The port this crate reaches the running app through.
//!
//! # Why a trait rather than `App<P>` directly
//!
//! Two reasons, and the second is the load-bearing one.
//!
//! The app is generic over its provider (`App<P: Provider>`), so taking it directly would
//! make `McpServer<P>`, the session, the registry and every tool generic too; viral parameters
//! through a crate that has no opinion about providers at all.
//!
//! More importantly, one capability this crate needs cannot come from the app: `create_draft`
//! opens the **client's own composer**, which is a host UI action reached over the UniFFI
//! callback port in `mailcal-bindings`. Depending on that crate would be a cycle (it depends on
//! this one). So the port is declared here, `mailcal-bindings` implements it over the app *and*
//! its host-UI slot, and this crate stays a pure adapter that can be tested against a fake in a
//! dozen lines.
//!
//! Every method maps 1:1 onto a `query_*` or `act_*` on the core. This trait deliberately adds
//! **no** logic: ordering, search scope, the recipient index and the write semantics all live in
//! `mailcal-app`, so an agent and a person are never shown two different mailboxes.

use async_trait::async_trait;
use mailcal_app::{MailActionError, MessageDetail, MessagePage, SendActionError};
use mailcal_viewmodel::{AccountRow, FolderRow};

/// A draft to open, unsent, in the client's own composer.
///
/// The recipient fields are comma-joined rather than lists, matching the shape the composer
/// intent already takes across the FFI; one representation of "a recipient field", not two.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct AgentDraft {
    /// The account to send from, or `None` to let the composer choose as it always does.
    pub account: Option<String>,
    /// The `To` field, comma-joined.
    pub to: String,
    /// The `Cc` field, comma-joined.
    pub cc: String,
    /// The `Bcc` field, comma-joined.
    pub bcc: String,
    /// The subject.
    pub subject: String,
    /// The body, as plain text.
    pub body_text: String,
    /// The account of the message being replied to, if this is a reply.
    pub reply_to_account: Option<String>,
    /// The provider key of the message being replied to, if this is a reply.
    pub reply_to_key: Option<String>,
}

/// Why opening the composer did not happen.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ComposerError {
    /// No host has registered a composer. A headless build, or a client that has not wired the
    /// port yet; Linux, today. Deliberately an error rather than a `#[cfg]`: a platform without
    /// a composer simply reports that it has none, and no conditional compilation is needed
    /// anywhere.
    NoHostComposer,
}

/// The running app, as this crate needs it.
///
/// Implemented by `mailcal-bindings` over the live `MailcalApp`, and by a fake in this crate's
/// tests. Every method is infallible-or-typed: nothing here returns a rendered message, because
/// user-facing strings belong in a client's catalog, not on a wire an assistant reads.
#[async_trait]
pub trait MailBackend: Send + Sync + 'static {
    /// Every configured account, as `(id, address)` rows.
    async fn accounts(&self) -> Vec<AccountRow>;

    /// One account's folders, in canonical sidebar order.
    async fn folders(&self, account: &str) -> Vec<FolderRow>;

    /// One page of a folder's messages, newest first.
    async fn folder_page(
        &self,
        account: &str,
        folder: Option<&str>,
        unread_only: bool,
        offset: usize,
        limit: usize,
    ) -> MessagePage;

    /// One page of search hits, newest first.
    async fn search(
        &self,
        query: &str,
        account: Option<&str>,
        folder: Option<&str>,
        offset: usize,
        limit: usize,
    ) -> MessagePage;

    /// One message in full, **without marking it read**.
    async fn message(&self, account: &str, key: &str) -> Option<MessageDetail>;

    /// Marks a message read or unread.
    async fn mark_read(&self, account: &str, key: &str, read: bool) -> Result<(), MailActionError>;

    /// Flags or unflags a message.
    async fn set_flagged(
        &self,
        account: &str,
        key: &str,
        flagged: bool,
    ) -> Result<(), MailActionError>;

    /// Moves a message to its account's Archive folder.
    async fn archive(&self, account: &str, key: &str) -> Result<(), MailActionError>;

    /// Moves a message to its account's Trash folder (recoverable).
    async fn trash(&self, account: &str, key: &str) -> Result<(), MailActionError>;

    /// Moves a message to its account's Junk folder.
    async fn spam(&self, account: &str, key: &str) -> Result<(), MailActionError>;

    /// Sends a plain-text message directly.
    async fn send_plain(
        &self,
        account: Option<&str>,
        to: &[String],
        cc: &[String],
        bcc: &[String],
        subject: String,
        body: String,
    ) -> Result<(), SendActionError>;

    /// The addresses the recipient index knows for `query`; people the user has actually
    /// written to (mined from Sent mail) plus any synced contacts. Backs the known-recipient
    /// guard in `policy`; see there for why that guard is the control that matters.
    async fn known_recipients(&self, query: &str) -> Vec<String>;

    /// Opens `draft` in the client's own composer, unsent.
    ///
    /// # Errors
    ///
    /// [`ComposerError::NoHostComposer`] when no client has registered one.
    fn open_composer(&self, draft: AgentDraft) -> Result<(), ComposerError>;
}
