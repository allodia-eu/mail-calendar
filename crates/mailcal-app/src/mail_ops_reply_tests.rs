//! Tests for **rich reply and forward**: that the shared rich-draft path derives the
//! recipient/subject/threading from the stored original (exactly as the plain versions
//! did) and carries the composer-rendered HTML body and attachment manifest. A child of
//! [`super`] (the rich-submit tests), reusing its `rich_document` and `SilentObserver`
//! fixtures; split into its own file to keep each test module under the 500-line limit.

use std::sync::{Arc, Mutex};

use engine_api::{
    AccountId, Draft, EmailAddress, Engine, MessageIdHeader, ProviderKey, SubmissionReceipt,
    TimeZoneId,
};
use engine_core::{
    ids::{MailboxId, MessageId},
    mail::{Mailbox, MailboxRole, Message},
    membership::Memberships,
    raw::RawMime,
    sync::{JmapDataType, SyncScope, SyncState, SyncUpdate, SyncWindow},
};
use engine_provider::{
    Capabilities, ConnectionInfo, EmailChunk, EmailStream, Provider, ProviderResult, ScopeSync,
};
use mailcal_composer::{AttachmentId, ComposerDocument, DraftBlobHandle};

use super::{SilentObserver, rich_document};
use crate::{Account, App, ComposerBlob, Intent, MessageRef, SendStatus, Telemetry, TimeZoneInit};

/// A mail provider that both **syncs** a configurable set of original messages (so a
/// reply/forward can resolve the original via `find_message_in`) and **records** the
/// drafts it is asked to submit: so a test can assert the derived recipient/subject/
/// threading and the rich body on the submitted draft.
struct ThreadProvider {
    caps: Capabilities,
    mailboxes: Vec<Mailbox>,
    messages: Vec<Message>,
    submissions: Arc<Mutex<Vec<Draft>>>,
}

impl ThreadProvider {
    fn with(messages: Vec<Message>) -> Self {
        let mut inbox = Mailbox::new(MailboxId::try_from("inbox").unwrap(), "Inbox");
        inbox.role = Some(MailboxRole::Inbox);
        Self {
            caps: Capabilities::none()
                .with_mail()
                .with_submission()
                .with_message_source(),
            mailboxes: vec![inbox],
            messages,
            submissions: Arc::new(Mutex::new(Vec::new())),
        }
    }

    fn submissions(&self) -> Arc<Mutex<Vec<Draft>>> {
        Arc::clone(&self.submissions)
    }
}

#[async_trait::async_trait]
impl Provider for ThreadProvider {
    fn connection_info(&self) -> ConnectionInfo {
        ConnectionInfo::new(self.caps)
    }

    fn mailbox_scope(&self, account: &AccountId) -> SyncScope {
        SyncScope::JmapType {
            account: account.clone(),
            data_type: JmapDataType::Mailbox,
        }
    }

    fn email_scope(&self, account: &AccountId) -> SyncScope {
        SyncScope::JmapType {
            account: account.clone(),
            data_type: JmapDataType::Email,
        }
    }

    async fn sync_mailboxes(
        &self,
        _account: &AccountId,
        cursor: Option<&SyncState>,
    ) -> ProviderResult<ScopeSync<Mailbox>> {
        if cursor.is_some() {
            return Ok(ScopeSync::new(
                SyncUpdate::delta(Vec::new(), Vec::new()),
                SyncState::new("mbox-2"),
            ));
        }
        let present = self.mailboxes.iter().map(|m| m.id.key().clone()).collect();
        Ok(ScopeSync::new(
            SyncUpdate::snapshot(self.mailboxes.clone(), present),
            SyncState::new("mbox-1"),
        ))
    }

    fn stream_email<'a>(
        &'a self,
        _account: &AccountId,
        cursor: Option<&'a SyncState>,
        _window: SyncWindow,
        _fetch_batch: usize,
        _chunk_size: usize,
    ) -> EmailStream<'a> {
        let chunk = if cursor.is_some() {
            EmailChunk::additive(Vec::new(), Vec::new(), None, SyncState::new("email-2"))
        } else {
            let present = self.messages.iter().map(|m| m.id.key().clone()).collect();
            EmailChunk::reconcile_last(
                self.messages.clone(),
                present,
                Some(self.messages.len()),
                SyncState::new("email-1"),
            )
        };
        Box::pin(futures::stream::iter(vec![Ok(chunk)]))
    }

    async fn submit_email(
        &self,
        _account: &AccountId,
        draft: &Draft,
    ) -> ProviderResult<SubmissionReceipt> {
        self.submissions.lock().unwrap().push(draft.clone());
        Ok(SubmissionReceipt::filed(
            ProviderKey::new("sent-1").unwrap(),
            draft.message_id.clone(),
        ))
    }

    /// Serves a `multipart/related` original carrying one inline `image/png` part
    /// (`Content-ID: <part1.demo@allodia.local>`, base64 `aGVsbG8=` = `hello`), so a
    /// reply/forward can re-derive the inline parts and re-attach them as `cid:` on send.
    async fn fetch_message_source(
        &self,
        _account: &AccountId,
        _message: &Message,
    ) -> ProviderResult<RawMime> {
        Ok(RawMime::new(
            concat!(
                "Content-Type: multipart/related; boundary=\"b\"\r\n\r\n",
                "--b\r\nContent-Type: text/html\r\n\r\n",
                "<p>Logo:</p><img src=\"cid:part1.demo@allodia.local\">\r\n",
                "--b\r\nContent-Type: image/png\r\n",
                "Content-ID: <part1.demo@allodia.local>\r\n",
                "Content-Transfer-Encoding: base64\r\n",
                "Content-Disposition: inline\r\n\r\naGVsbG8=\r\n",
                "--b--\r\n",
            )
            .as_bytes()
            .to_vec(),
        ))
    }
}

/// Builds a one-account app over a [`ThreadProvider`] seeded with `messages`, returning
/// the app and the submission log. The caller dispatches `RefreshMail` to load the
/// originals into the store before replying/forwarding.
fn reply_app(messages: Vec<Message>) -> (Arc<App<ThreadProvider>>, Arc<Mutex<Vec<Draft>>>) {
    let provider = ThreadProvider::with(messages);
    let submissions = provider.submissions();
    let app = App::new(
        Engine::open_in_memory().unwrap(),
        vec![Account {
            id: AccountId::try_from("acct-1").unwrap(),
            providers: vec![provider],
            calendar_providers: Vec::new(),
            contact_providers: Vec::new(),
            identity: EmailAddress::new("me@allodia.local"),
        }],
        TimeZoneInit {
            device_zone: TimeZoneId::utc(),
            prefs_path: None,
        },
        None,
        std::sync::Arc::new(SilentObserver),
        Telemetry::off(None),
    );
    (Arc::new(app), submissions)
}

/// Spawns `intent`, drives it under paused virtual time until it reaches `target`, and
/// flushes the post-send `refresh_mail` so the auto-clear sleep parks; the
/// `ThreadProvider` twin of the parent module's `dispatch_until` (which is monomorphized
/// over its own provider).
async fn dispatch_until(
    app: &Arc<App<ThreadProvider>>,
    intent: Intent,
    target: SendStatus,
) -> tokio::task::JoinHandle<()> {
    let task = tokio::spawn({
        let app = Arc::clone(app);
        async move {
            app.dispatch(intent).await;
        }
    });
    while app.send_status() != target {
        tokio::task::yield_now().await;
    }
    for _ in 0..64 {
        tokio::task::yield_now().await;
    }
    task
}

/// An original message `key` in the inbox, carrying a `Reply-To`, a `From`, a `To`/`Cc`
/// (including the account's own identity and the `Reply-To`, so reply-all dedup/self-removal
/// is exercised), a parent `Message-ID`, and a one-entry `References` chain; enough to
/// exercise reply recipient derivation (`Reply-To` wins over `From`), reply-all `Cc`
/// derivation, and `In-Reply-To`/`References` threading.
fn original_message(key: &str) -> Message {
    let mut message = Message::new(
        MessageId::try_from(key).unwrap(),
        Memberships::of_one(MailboxId::try_from("inbox").unwrap()),
    );
    message.envelope.subject = Some("Quarterly report".to_owned());
    message.envelope.from = vec![EmailAddress::new("sender@remote.test")];
    message.envelope.reply_to = vec![EmailAddress::new("reply@remote.test")];
    // To/Cc include the account's own identity (`me@allodia.local`) and the Reply-To
    // address (which a reply-all puts in To), so both must be excluded from the reply-all Cc.
    message.envelope.to = vec![
        EmailAddress::new("me@allodia.local"),
        EmailAddress::new("colleague@remote.test"),
    ];
    message.envelope.cc = vec![
        EmailAddress::new("boss@remote.test"),
        EmailAddress::new("reply@remote.test"),
    ];
    message.envelope.message_id = vec![MessageIdHeader::new("parent@remote").unwrap()];
    message.envelope.references = vec![MessageIdHeader::new("root@remote").unwrap()];
    message
}

/// A small rich document (bold text + an inline image + a file attachment) and the bytes
/// for its blobs; shared by the reply and forward tests so they assert a real composer
/// render (a plain draft would have no HTML body and no attachments).
fn reply_document() -> (ComposerDocument, Vec<ComposerBlob>) {
    let inline_blob = DraftBlobHandle::new("blob-inline").unwrap();
    let file_blob = DraftBlobHandle::new("blob-file").unwrap();
    let document = rich_document(
        AttachmentId::new("inline-chart").unwrap(),
        AttachmentId::new("file-report").unwrap(),
        inline_blob.clone(),
        file_blob.clone(),
    );
    let blobs = vec![
        ComposerBlob::new(inline_blob, vec![1, 2, 3]),
        ComposerBlob::new(file_blob, b"PDF!".to_vec()),
    ];
    (document, blobs)
}

#[tokio::test(start_paused = true)]
async fn rich_reply_threads_and_carries_the_composer_html() {
    let (app, submissions) = reply_app(vec![original_message("m1")]);
    // Sync first so the original is in the store and the reply can derive its headers.
    app.dispatch(Intent::RefreshMail).await;

    let (document, blobs) = reply_document();
    // The host pre-fills the recipients from `reply_recipients` (here a plain reply: To =
    // Reply-To, no Cc) and may add a Bcc; the reply send uses exactly what it is given.
    let intent = Intent::SubmitRichReply {
        message: MessageRef::from_parts("acct-1", "m1".to_owned()).unwrap(),
        from: None,
        to: "reply@remote.test".to_owned(),
        cc: String::new(),
        bcc: "discreet@remote.test".to_owned(),
        subject: None,
        document,
        blobs,
    };
    let _task = dispatch_until(&app, intent, SendStatus::Sent).await;
    assert_eq!(app.send_status(), SendStatus::Sent);

    let submissions = submissions.lock().unwrap();
    assert_eq!(submissions.len(), 1);
    let draft = &submissions[0];
    // Recipients are the ones supplied; Bcc carried through; Re: subject derived in core.
    assert_eq!(draft.to.len(), 1);
    assert_eq!(draft.to[0].email, "reply@remote.test");
    assert!(draft.cc.is_empty());
    assert_eq!(draft.bcc.len(), 1);
    assert_eq!(draft.bcc[0].email, "discreet@remote.test");
    assert_eq!(draft.subject, "Re: Quarterly report");
    // Rich body: the HTML alternative came from the composer render, not a plain draft.
    assert_eq!(draft.text_body, "Hello [Chart]");
    let html = draft
        .html_body
        .as_deref()
        .expect("rich reply has an HTML body");
    assert!(html.starts_with("<!DOCTYPE html><html><head>"));
    assert!(html.contains("<img src=\"cid:chart@test.local\""));
    assert_eq!(draft.attachments.len(), 2);
    // Threading: In-Reply-To the parent, References = original's chain + the parent.
    assert_eq!(
        draft.in_reply_to.as_ref().map(MessageIdHeader::as_str),
        Some("parent@remote")
    );
    let references: Vec<&str> = draft
        .references
        .iter()
        .map(MessageIdHeader::as_str)
        .collect();
    assert_eq!(references, vec!["root@remote", "parent@remote"]);
}

/// A forward belongs to the conversation it came from. It carries the original's
/// `References` chain (but no `In-Reply-To`, because it answers nothing) and that chain is
/// what puts the Sent copy on the thread. Without it, every forward the user sends becomes a
/// separate one-message conversation sitting beside the discussion it is part of.
#[tokio::test(start_paused = true)]
async fn rich_forward_sets_fwd_subject_and_threads_on_references() {
    let (app, submissions) = reply_app(vec![original_message("m1")]);
    app.dispatch(Intent::RefreshMail).await;

    let (document, blobs) = reply_document();
    let intent = Intent::SubmitRichForward {
        message: MessageRef::from_parts("acct-1", "m1".to_owned()).unwrap(),
        from: None,
        to: "dest@forward.test".to_owned(),
        cc: "watcher@forward.test".to_owned(),
        bcc: String::new(),
        subject: None,
        document,
        blobs,
    };
    let _task = dispatch_until(&app, intent, SendStatus::Sent).await;
    assert_eq!(app.send_status(), SendStatus::Sent);

    let submissions = submissions.lock().unwrap();
    assert_eq!(submissions.len(), 1);
    let draft = &submissions[0];
    // Recipients are the explicitly given addresses; Fwd: subject.
    assert_eq!(draft.to.len(), 1);
    assert_eq!(draft.to[0].email, "dest@forward.test");
    assert_eq!(draft.cc.len(), 1);
    assert_eq!(draft.cc[0].email, "watcher@forward.test");
    assert_eq!(draft.subject, "Fwd: Quarterly report");
    // Rich body present (a forward through the rich path still renders the composer).
    assert_eq!(draft.text_body, "Hello [Chart]");
    assert!(draft.html_body.is_some());
    assert_eq!(draft.attachments.len(), 2);
    // Threading: the original's chain plus the original itself, and no reply pointer.
    let references: Vec<&str> = draft
        .references
        .iter()
        .map(MessageIdHeader::as_str)
        .collect();
    assert_eq!(references, vec!["root@remote", "parent@remote"]);
    assert!(
        draft.in_reply_to.is_none(),
        "a forward continues a thread; it does not answer a message"
    );
}

#[tokio::test]
async fn reply_recipients_for_a_plain_reply_is_reply_to_with_no_cc() {
    let (app, _submissions) = reply_app(vec![original_message("m1")]);
    app.dispatch(Intent::RefreshMail).await;

    let message = MessageRef::from_parts("acct-1", "m1".to_owned()).unwrap();
    let suggestion = app.reply_recipients(message, false).await;
    // To is the Reply-To (wins over From); a plain reply suggests no Cc.
    assert_eq!(suggestion.to, "reply@remote.test");
    assert_eq!(suggestion.cc, "");
}

#[tokio::test]
async fn reply_recipients_for_reply_all_keeps_to_in_to_and_cc_in_cc() {
    let (app, _submissions) = reply_app(vec![original_message("m1")]);
    app.dispatch(Intent::RefreshMail).await;

    let message = MessageRef::from_parts("acct-1", "m1".to_owned()).unwrap();
    let suggestion = app.reply_recipients(message, true).await;
    // To leads with the Reply-To, then keeps the original's other To recipients (Outlook
    // behaviour): the account's own identity (`me@allodia.local`) removed. Cc keeps the
    // original Cc; `reply@remote.test` is already in To, so it isn't repeated in Cc.
    assert_eq!(suggestion.to, "reply@remote.test, colleague@remote.test");
    assert_eq!(suggestion.cc, "boss@remote.test");
}

#[tokio::test]
async fn reply_all_keeps_a_co_recipient_in_to_not_cc() {
    // The reported case: an inbox message addressed To: me + another person, Cc: a third.
    // Reply-all must KEEP the other To recipient in To (Outlook/Thunderbird behaviour), not
    // demote them to Cc.
    let mut original = Message::new(
        MessageId::try_from("m1").unwrap(),
        Memberships::of_one(MailboxId::try_from("inbox").unwrap()),
    );
    original.envelope.subject = Some("Team update".to_owned());
    original.envelope.from = vec![EmailAddress::new("sender@remote.test")];
    original.envelope.to = vec![
        EmailAddress::new("me@allodia.local"),
        EmailAddress::new("alice@remote.test"),
    ];
    original.envelope.cc = vec![EmailAddress::new("bob@remote.test")];

    let (app, _submissions) = reply_app(vec![original]);
    app.dispatch(Intent::RefreshMail).await;

    let message = MessageRef::from_parts("acct-1", "m1".to_owned()).unwrap();
    let suggestion = app.reply_recipients(message, true).await;
    // To = the sender + the OTHER original To recipient (me removed); Cc = the original Cc.
    assert_eq!(suggestion.to, "sender@remote.test, alice@remote.test");
    assert_eq!(suggestion.cc, "bob@remote.test");
}

#[tokio::test]
async fn reply_recipients_is_empty_when_the_original_is_unknown() {
    let (app, _submissions) = reply_app(vec![original_message("m1")]);
    app.dispatch(Intent::RefreshMail).await;

    // A key not in the account's synced set yields an empty suggestion rather than a guess.
    let message = MessageRef::from_parts("acct-1", "missing".to_owned()).unwrap();
    let suggestion = app.reply_recipients(message, true).await;
    assert_eq!(suggestion.to, "");
    assert_eq!(suggestion.cc, "");
}

#[tokio::test]
async fn reply_to_your_own_message_keeps_the_sender_in_to() {
    // Replying to a message in your own Sent folder: From == the account identity. The To must
    // still carry the sender (you), not be emptied by self-exclusion; otherwise the reply is
    // unsendable.
    let mut sent = Message::new(
        MessageId::try_from("m1").unwrap(),
        Memberships::of_one(MailboxId::try_from("inbox").unwrap()),
    );
    sent.envelope.subject = Some("Notes to self".to_owned());
    sent.envelope.from = vec![EmailAddress::new("me@allodia.local")];
    sent.envelope.to = vec![EmailAddress::new("me@allodia.local")];

    let (app, _submissions) = reply_app(vec![sent]);
    app.dispatch(Intent::RefreshMail).await;
    let message = MessageRef::from_parts("acct-1", "m1".to_owned()).unwrap();
    let suggestion = app.reply_recipients(message, false).await;
    assert_eq!(suggestion.to, "me@allodia.local");
    assert_eq!(suggestion.cc, "");
}

#[tokio::test(start_paused = true)]
async fn rich_reply_with_no_recipients_fails_without_sending() {
    // The command surface guards against a recipient-less send for every caller (not just the
    // clients' Send-button gating): all of To/Cc/Bcc empty (or whitespace-only) → a Failed
    // hint, never a queued empty draft.
    let (app, submissions) = reply_app(vec![original_message("m1")]);
    app.dispatch(Intent::RefreshMail).await;

    let (document, blobs) = reply_document();
    let intent = Intent::SubmitRichReply {
        message: MessageRef::from_parts("acct-1", "m1".to_owned()).unwrap(),
        from: None,
        to: String::new(),
        cc: "   ".to_owned(),
        bcc: String::new(),
        subject: None,
        document,
        blobs,
    };
    let _task = dispatch_until(&app, intent, SendStatus::Failed).await;
    assert_eq!(app.send_status(), SendStatus::Failed);
    assert!(submissions.lock().unwrap().is_empty());
}

// Submit-time quote-body hardening (re-sanitisation + `data:`→`cid:` inline re-attachment)
// lives in its own file (each test module stays under the 500-line limit), as a child module it
// reuses this module's `reply_app`/`original_message`/`dispatch_until` fixtures.
#[path = "mail_ops_reply_quote_tests.rs"]
mod quote;

// The send-from-account tests (the composer's From dropdown + the default-send-account fallback)
// live in their own file, as a child module reusing this module's two-provider fixtures.
#[path = "mail_ops_from_tests.rs"]
mod from_account;

// The editable-subject tests, likewise a child module on this file's fixtures.
#[path = "mail_ops_reply_subject_tests.rs"]
mod subject;
