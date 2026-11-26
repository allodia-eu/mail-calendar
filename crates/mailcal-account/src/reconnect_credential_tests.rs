//! An OAuth IMAP account whose token cannot be renewed: the dial that needed one fails before
//! anything is sent, so the session in hand is kept and nothing is dialled again.

use std::sync::{
    Arc, Mutex,
    atomic::{AtomicUsize, Ordering},
};

use engine_api::{DraftReceipt, ProviderKey};
use engine_core::{
    error::FailureClass,
    ids::{AccountId, MailboxId, MessageId, MessageIdHeader},
    mail::{EmailAddress, Mailbox, Message, MessageMetadata},
    membership::Memberships,
    sync::SyncState,
};
use engine_provider::{
    Capabilities, ConnectionInfo, Draft, Provider, ProviderError, ProviderResult, ScopeSync,
};
use futures::StreamExt;

use super::{ReconnectingImapProvider, Redial};
use crate::graph::token_source::test_support::{imap_dial_without_token, refused_token_endpoint};

/// The failure an OAuth IMAP dial meets when the account's token endpoint cannot be reached.
async fn token_not_renewed() -> ProviderError {
    imap_dial_without_token(refused_token_endpoint()).await
}

/// A session whose first call fails with `failure`; a second call means the wrapper sent the
/// request again.
struct NeedsADial {
    failure: Mutex<Option<ProviderError>>,
    calls: Arc<AtomicUsize>,
}

impl NeedsADial {
    fn fail(&self) -> ProviderError {
        self.calls.fetch_add(1, Ordering::SeqCst);
        self.failure
            .lock()
            .unwrap()
            .take()
            .expect("the request was sent again")
    }
}

#[async_trait::async_trait]
impl Provider for NeedsADial {
    fn connection_info(&self) -> ConnectionInfo {
        ConnectionInfo::new(Capabilities::none().with_mail())
    }

    async fn sync_mailboxes(
        &self,
        _account: &AccountId,
        _cursor: Option<&SyncState>,
    ) -> ProviderResult<ScopeSync<Mailbox>> {
        Err(self.fail())
    }

    async fn put_draft(
        &self,
        _account: &AccountId,
        _draft: &Draft,
        _replacing: Option<&ProviderKey>,
        _metadata: &MessageMetadata,
    ) -> ProviderResult<DraftReceipt> {
        Err(self.fail())
    }
}

impl engine_api::CalendarSync for NeedsADial {}
impl engine_api::CalendarWrites for NeedsADial {}
impl engine_api::MailboxWrites for NeedsADial {}

/// What a test adopts and counts: the wrapper, its session's calls and its redials.
type Adopted = (ReconnectingImapProvider, Arc<AtomicUsize>, Arc<AtomicUsize>);

/// A wrapper adopting a session that fails with `failure`.
fn adopting(failure: ProviderError) -> Adopted {
    let calls = Arc::new(AtomicUsize::new(0));
    let redials = Arc::new(AtomicUsize::new(0));
    let counted = Arc::clone(&redials);
    let redial: Redial = Box::new(move || {
        counted.fetch_add(1, Ordering::SeqCst);
        Box::pin(async { Err(ProviderError::retryable("a redial")) })
    });
    let session = Arc::new(NeedsADial {
        failure: Mutex::new(Some(failure)),
        calls: Arc::clone(&calls),
    });
    let provider =
        ReconnectingImapProvider::adopt(session, MailboxId::try_from("INBOX").unwrap(), redial);
    (provider, calls, redials)
}

fn account() -> AccountId {
    AccountId::try_from("you@example.net@imap.example.net").unwrap()
}

#[tokio::test]
async fn a_read_keeps_its_session_and_dials_nothing_when_the_token_cannot_be_renewed() {
    let failure = token_not_renewed().await;
    assert_eq!(failure.class(), FailureClass::Retryable, "{failure}");
    let (provider, calls, redials) = adopting(failure);

    let refused = provider.sync_mailboxes(&account(), None).await.unwrap_err();

    assert_eq!(refused.class(), FailureClass::Retryable, "{refused}");
    assert_eq!(calls.load(Ordering::SeqCst), 1, "sent once");
    assert_eq!(redials.load(Ordering::SeqCst), 0, "nothing dialled");
    assert!(
        provider.cached.lock().unwrap().is_some(),
        "the session was kept"
    );
}

#[tokio::test]
async fn a_write_keeps_its_session_when_the_token_cannot_be_renewed() {
    let (provider, calls, redials) = adopting(token_not_renewed().await);
    let draft = Draft::new(
        MessageIdHeader::new("draft@example.com").unwrap(),
        EmailAddress::new("you@example.net"),
        vec![EmailAddress::new("them@example.org")],
        "Subject",
        "Body",
    );

    provider
        .put_draft(&account(), &draft, None, &MessageMetadata::default())
        .await
        .unwrap_err();

    assert_eq!(calls.load(Ordering::SeqCst), 1);
    assert_eq!(redials.load(Ordering::SeqCst), 0);
    assert!(
        provider.cached.lock().unwrap().is_some(),
        "the session was kept"
    );
}

/// A batch whose session cannot be dialled for want of a token answers each message with that
/// failure, still marked as a credential that could not be had, so whoever reads the batch tells
/// it from a dead connection as the wrapper does.
#[tokio::test]
async fn a_batch_without_a_token_answers_each_message_with_the_credential_failure() {
    let (provider, calls, _) = adopting(ProviderError::retryable("never asked"));
    let redial: Redial = Box::new(|| Box::pin(async { Err(token_not_renewed().await) }));
    let provider = ReconnectingImapProvider { redial, ..provider };
    provider.invalidate();
    let inbox = MailboxId::try_from("INBOX").unwrap();
    let messages: Vec<_> = ["imap:v1:u1@INBOX", "imap:v1:u2@INBOX"]
        .into_iter()
        .map(|id| {
            Message::new(
                MessageId::try_from(id).unwrap(),
                Memberships::of_one(inbox.clone()),
            )
        })
        .collect();

    let answers: Vec<_> = provider
        .fetch_message_sources(&account(), &messages)
        .collect()
        .await;

    assert_eq!(answers.len(), 2);
    for (index, answer) in answers {
        let failed = answer.unwrap_err();
        assert!(failed.is_credential_failure(), "{index}: {failed}");
        assert_eq!(failed.class(), FailureClass::Retryable, "{index}: {failed}");
    }
    assert_eq!(calls.load(Ordering::SeqCst), 0, "no session was asked");
}
