//! The mail-serving provider the reply, forward and draft-resume tests share: it **syncs**
//! a configurable set of stored messages (so a reply, a forward or a resume can resolve one
//! through `find_message_in`) and **records** what it is asked to submit, store as a draft
//! and remove.
//!
//! Its own file, as [`super::fake`] is for the submitting one, so each test module stays
//! under the 500-line limit.

use std::sync::{Arc, Mutex};

use engine_api::{
    AccountId, CalendarWrites, Draft, EmailAddress, Engine, MessageIdHeader, ProviderKey,
    SubmissionReceipt, TimeZoneId,
};
use engine_core::{
    ids::{MailboxId, MessageId},
    mail::{Keyword, Mailbox, MailboxRole, Message, SystemKeyword},
    membership::Memberships,
    raw::RawMime,
    sync::{JmapDataType, SyncScope, SyncState, SyncUpdate, SyncWindow},
};
use engine_provider::{
    Capabilities, ConnectionInfo, EmailChunk, EmailStream, Provider, ProviderResult, ScopeSync,
};

use super::SilentObserver;
use crate::{Account, App, Intent, SendStatus, Telemetry, TimeZoneInit};

/// A mail provider that both **syncs** a configurable set of original messages (so a
/// reply/forward can resolve the original via `find_message_in`) and **records** the
/// drafts it is asked to submit: so a test can assert the derived recipient/subject/
/// threading and the rich body on the submitted draft.
pub(super) struct ThreadProvider {
    caps: Capabilities,
    mailboxes: Vec<Mailbox>,
    messages: Vec<Message>,
    submissions: Arc<Mutex<Vec<Draft>>>,
    /// Serves an error instead of the raw source, so a test can drive the path where a
    /// forward cannot read the files it is meant to carry.
    source_fails: bool,
    /// Every draft stored, with the key each save said it was superseding: the pair, because
    /// whether a save names the copy it replaces is the whole draft contract
    /// (`docs/drafts.md`).
    draft_puts: Arc<Mutex<Vec<DraftSave>>>,
    /// Every stored draft removed.
    draft_deletes: Arc<Mutex<Vec<ProviderKey>>>,
    /// When set, `submit_email` fails **permanently**: the one send outcome that leaves the
    /// composer's stored draft where it is.
    send_fails: bool,
    /// When set, `submit_email` fails **retryably**, so the engine parks the message in the
    /// outbox: sent from a train, not lost.
    send_queues: bool,
}

/// One recorded draft save: what was stored, and the key it said it was superseding.
pub(super) type DraftSave = (Draft, Option<ProviderKey>);

/// Everything a [`ThreadProvider`] records, for a test that asserts on more than the sends.
pub(super) struct ProviderLogs {
    pub(super) submissions: Arc<Mutex<Vec<Draft>>>,
    pub(super) draft_puts: Arc<Mutex<Vec<DraftSave>>>,
    pub(super) draft_deletes: Arc<Mutex<Vec<ProviderKey>>>,
}

impl ThreadProvider {
    pub(super) fn with(messages: Vec<Message>) -> Self {
        let mut inbox = Mailbox::new(MailboxId::try_from("inbox").unwrap(), "Inbox");
        inbox.role = Some(MailboxRole::Inbox);
        let mut drafts = Mailbox::new(MailboxId::try_from("drafts").unwrap(), "Drafts");
        drafts.role = Some(MailboxRole::Drafts);
        Self {
            caps: Capabilities::none()
                .with_mail()
                .with_submission()
                .with_message_source(),
            mailboxes: vec![inbox, drafts],
            messages,
            submissions: Arc::new(Mutex::new(Vec::new())),
            source_fails: false,
            draft_puts: Arc::new(Mutex::new(Vec::new())),
            draft_deletes: Arc::new(Mutex::new(Vec::new())),
            send_fails: false,
            send_queues: false,
        }
    }

    /// A provider whose every send fails outright, with nothing queued behind it.
    pub(super) fn failing_send(mut self) -> Self {
        self.send_fails = true;
        self
    }

    /// A provider with no network for sending, so every message goes to the outbox instead.
    pub(super) fn queueing_send(mut self) -> Self {
        self.send_queues = true;
        self
    }

    pub(super) fn logs(&self) -> ProviderLogs {
        ProviderLogs {
            submissions: Arc::clone(&self.submissions),
            draft_puts: Arc::clone(&self.draft_puts),
            draft_deletes: Arc::clone(&self.draft_deletes),
        }
    }

    pub(super) fn failing_source(mut self) -> Self {
        self.source_fails = true;
        self
    }

    pub(super) fn submissions(&self) -> Arc<Mutex<Vec<Draft>>> {
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
        if self.send_fails {
            return Err(engine_provider::ProviderError::permanent("mailbox refused"));
        }
        if self.send_queues {
            return Err(engine_provider::ProviderError::retryable(
                "no route to host",
            ));
        }
        Ok(SubmissionReceipt::filed(
            ProviderKey::new("sent-1").unwrap(),
            draft.message_id.clone(),
        ))
    }

    /// Stores a draft, answering with a key that **moves on every save**, which is what
    /// three of the four real adapters do: a caller that never passes `replacing` therefore
    /// leaves a second copy here too, rather than passing quietly.
    async fn put_draft(
        &self,
        _account: &AccountId,
        draft: &Draft,
        replacing: Option<&ProviderKey>,
    ) -> ProviderResult<ProviderKey> {
        let mut puts = self.draft_puts.lock().unwrap();
        puts.push((draft.clone(), replacing.cloned()));
        Ok(ProviderKey::new(format!("draft-{}", puts.len())).unwrap())
    }

    async fn delete_draft(&self, _account: &AccountId, draft: &ProviderKey) -> ProviderResult<()> {
        self.draft_deletes.lock().unwrap().push(draft.clone());
        Ok(())
    }

    /// Serves an original carrying both kinds of part: one inline `image/png`
    /// (`Content-ID: <part1.demo@allodia.local>`, base64 `aGVsbG8=` = `hello`), so a
    /// reply/forward can re-derive the inline parts and re-attach them as `cid:` on send,
    /// and one downloadable `invoice.pdf` (base64 `SU5WT0lDRQ==` = `INVOICE`), which is what
    /// a forward has to carry on to its recipient.
    async fn fetch_message_source(
        &self,
        _account: &AccountId,
        message: &Message,
    ) -> ProviderResult<RawMime> {
        if self.source_fails {
            return Err(engine_provider::ProviderError::retryable(
                "message source unavailable",
            ));
        }
        // A stored draft is a half-written message with a file on it: the shape a resume has
        // to carry back into the composer whole (`docs/drafts.md`).
        if message.is_draft() {
            return Ok(RawMime::new(
                concat!(
                    "Content-Type: multipart/mixed; boundary=\"d\"\r\n\r\n",
                    "--d\r\nContent-Type: text/plain\r\n\r\n",
                    "Half a sentence\r\n",
                    "--d\r\nContent-Type: application/pdf\r\n",
                    "Content-Disposition: attachment; filename=\"terms.pdf\"\r\n",
                    "Content-Transfer-Encoding: base64\r\n\r\nVEVSTVM=\r\n",
                    "--d--\r\n",
                )
                .as_bytes()
                .to_vec(),
            ));
        }
        Ok(RawMime::new(
            concat!(
                "Content-Type: multipart/mixed; boundary=\"m\"\r\n\r\n",
                "--m\r\nContent-Type: multipart/related; boundary=\"b\"\r\n\r\n",
                "--b\r\nContent-Type: text/html\r\n\r\n",
                "<p>Logo:</p><img src=\"cid:part1.demo@allodia.local\">\r\n",
                "--b\r\nContent-Type: image/png\r\n",
                "Content-ID: <part1.demo@allodia.local>\r\n",
                "Content-Transfer-Encoding: base64\r\n",
                "Content-Disposition: inline\r\n\r\naGVsbG8=\r\n",
                "--b--\r\n",
                "--m\r\nContent-Type: application/pdf\r\n",
                "Content-Disposition: attachment; filename=\"invoice.pdf\"\r\n",
                "Content-Transfer-Encoding: base64\r\n\r\nSU5WT0lDRQ==\r\n",
                "--m--\r\n",
            )
            .as_bytes()
            .to_vec(),
        ))
    }
}

impl CalendarWrites for ThreadProvider {}

/// Builds a one-account app over a [`ThreadProvider`] seeded with `messages`, returning
/// the app and the submission log. The caller dispatches `RefreshMail` to load the
/// originals into the store before replying/forwarding.
pub(super) fn reply_app(
    messages: Vec<Message>,
) -> (Arc<App<ThreadProvider>>, Arc<Mutex<Vec<Draft>>>) {
    app_over(ThreadProvider::with(messages))
}

/// [`reply_app`] over an already-built provider, for a test that configures one (a source
/// that will not serve).
pub(super) fn app_over(
    provider: ThreadProvider,
) -> (Arc<App<ThreadProvider>>, Arc<Mutex<Vec<Draft>>>) {
    let (app, logs) = app_and_logs(provider);
    (app, logs.submissions)
}

/// [`app_over`], handing back every log the provider keeps: what a draft test asserts on is
/// what was stored and removed, not only what was sent.
pub(super) fn app_and_logs(provider: ThreadProvider) -> (Arc<App<ThreadProvider>>, ProviderLogs) {
    let logs = provider.logs();
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
    (Arc::new(app), logs)
}

/// Spawns `intent`, drives it under paused virtual time until it reaches `target`, and
/// flushes the post-send `refresh_mail` so the auto-clear sleep parks; the
/// `ThreadProvider` twin of the parent module's `dispatch_until` (which is monomorphized
/// over its own provider).
pub(super) async fn dispatch_until(
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
pub(super) fn original_message(key: &str) -> Message {
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

/// A stored draft `key` in the Drafts folder: a half-addressed, half-written message with a
/// file on it, carrying the `$draft` keyword every adapter family sets.
///
/// The keyword rather than the folder is what a resume asks about, so this fixture is what
/// tells a resume that refuses ordinary mail from one that lets a composer adopt it.
pub(super) fn stored_draft(key: &str) -> Message {
    let mut message = Message::new(
        MessageId::try_from(key).unwrap(),
        Memberships::of_one(MailboxId::try_from("drafts").unwrap()),
    );
    message
        .keywords
        .insert(Keyword::system(SystemKeyword::Draft));
    message.envelope.subject = Some("Half a subject".to_owned());
    message.envelope.from = vec![EmailAddress::new("me@allodia.local")];
    message.envelope.to = vec![EmailAddress::new("you@remote.test")];
    message.envelope.cc = vec![EmailAddress::new("cc@remote.test")];
    message.envelope.message_id = vec![MessageIdHeader::new("half@allodia.local").unwrap()];
    message.has_attachment = true;
    message
}
