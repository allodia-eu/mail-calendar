//! The [`DemoProvider`]: an in-memory provider seeding a small sample mailbox so the app
//! shows real rows before an account is configured. Split out of
//! `lib.rs` to keep it under the 500-line limit; a real account-configured provider
//! replaces it ([`crate::MailcalApp::new_accounts`]).

use engine_api::AccountId;
use engine_core::{
    ids::{MailboxId, MessageId, MessageIdHeader},
    mail::{Mailbox, MailboxRole, Message},
    membership::Memberships,
    raw::RawMime,
    sync::{JmapDataType, SyncScope, SyncState, SyncUpdate, SyncWindow},
};
use engine_provider::{
    Capabilities, ConnectionInfo, EmailChunk, EmailStream, Provider, ProviderResult, ScopeSync,
};

/// A demo provider seeding a small sample mailbox, so the app shows real rows before an
/// account is configured. A real account-configured provider replaces it.
pub(crate) struct DemoProvider {
    caps: Capabilities,
    mailboxes: Vec<Mailbox>,
    messages: Vec<Message>,
}

impl DemoProvider {
    pub(crate) fn new() -> Self {
        let mut inbox = Mailbox::new(MailboxId::try_from("inbox").unwrap(), "Inbox");
        inbox.role = Some(MailboxRole::Inbox);
        Self {
            caps: Capabilities::none().with_mail().with_message_source(),
            mailboxes: vec![inbox],
            messages: vec![
                sample_message(
                    "m1",
                    "Welcome to Allodia Mail & Calendar",
                    "Thanks for trying the app; here's how to get started with your mailbox.",
                ),
                sample_message(
                    "m2",
                    "Your Q3 sovereignty report is ready",
                    "The quarterly data-residency summary is attached for your review.",
                ),
                sample_message(
                    "m3",
                    "Lunch on Friday?",
                    "Are you free around noon? Thought we could grab a bite and catch up.",
                ),
                // A reply to m1, so the threaded view shows a real 2-message conversation.
                reply_message(
                    "m4",
                    "Re: Welcome to Allodia Mail & Calendar",
                    "m1",
                    "Great, thanks! One quick question about connecting a second account.",
                ),
            ],
        }
    }
}

fn sample_message(id: &str, subject: &str, preview: &str) -> Message {
    let mut message = Message::new(
        MessageId::try_from(id).unwrap(),
        Memberships::of_one(MailboxId::try_from("inbox").unwrap()),
    );
    message.envelope.subject = Some(subject.to_owned());
    message.envelope.message_id = vec![MessageIdHeader::new(format!("{id}@demo")).unwrap()];
    message.preview = Some(preview.to_owned());
    message
}

/// A reply referencing `in_reply_to`'s message-id, so the engine threads the two
/// together; exercising the threaded view on demo data.
fn reply_message(id: &str, subject: &str, in_reply_to: &str, preview: &str) -> Message {
    let mut message = sample_message(id, subject, preview);
    message.envelope.references =
        vec![MessageIdHeader::new(format!("{in_reply_to}@demo")).unwrap()];
    message
}

#[async_trait::async_trait]
impl Provider for DemoProvider {
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

    async fn fetch_message_source(
        &self,
        _account: &AccountId,
        message: &Message,
    ) -> ProviderResult<RawMime> {
        // A demo HTML body carrying hostile content (a script + a remote tracking image)
        // so the reading view demonstrates HTML rendering and the core's sanitization
        // (both are stripped before the host sees the body).
        let subject = message
            .envelope
            .subject
            .as_deref()
            .unwrap_or("(no subject)");
        let html = format!(
            "Content-Type: text/html; charset=utf-8\r\n\r\n\
             <html><body><h2>{subject}</h2>\
             <p>This is a <b>demo</b> message rendered as <i>sanitized</i> HTML; \
             scripts and remote images are stripped before display.</p>\
             <script>alert('xss')</script>\
             <img src=\"https://tracker.example/pixel.gif\"></body></html>"
        );
        Ok(RawMime::new(html.into_bytes()))
    }
}
