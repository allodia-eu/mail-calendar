//! Which credential a Gmail provider's two halves sign in with, what each request presents, and
//! the client a dead connection replaces.

use std::sync::{
    Arc, Mutex,
    atomic::{AtomicUsize, Ordering},
};

use engine_api::EmailAddress;
use engine_core::{
    error::FailureClass,
    ids::{AccountId, MailboxId, MessageId, MessageIdHeader},
    mail::{Message, MessageMetadata},
    membership::Memberships,
    sync::SyncWindow,
};
use engine_provider::{
    Credential, CredentialSource, CredentialSubject, Draft, FixedCredential, HandOver, MailboxEdit,
    MailboxWrites, Provider, Unrecorded,
};
use engine_tls::{TlsClientConfig, TlsPolicy};
use futures::StreamExt;
use provider_google::GoogleClient;
use provider_imap::{ImapAccount, ImapConfig};

use super::super::{
    Built, ReconnectingGmailProvider, client_connecting,
    imap::{GmailImap, GmailImapSlot},
};
use crate::{
    GmailKeywordNaming, GraphTokenSource,
    graph::test_support::{mock_token_endpoint, refused_token_endpoint, source_at},
    http_fake::{self, Reply},
    tls::tls_with,
};

const LABELS: &str = r#"{"labels":[]}"#;

/// A source over a token endpoint answering `tokens` in turn, and how often it was asked.
fn tokens(tokens: Vec<String>) -> (Arc<GraphTokenSource>, Arc<AtomicUsize>) {
    let (endpoint, hits) = mock_token_endpoint(tokens);
    (source_at(endpoint, None), hits)
}

fn account() -> AccountId {
    AccountId::try_from("alice@gmail.com@mail.google.com").unwrap()
}

/// The engine's Gmail IMAP server, and a reader of it signing in as `subject`.
async fn reader(subject: &CredentialSubject) -> (GmailImap, TlsClientConfig) {
    let (cert, address) = provider_imap::test_server::gmail().await;
    let tls = engine_tls::client_config(&TlsPolicy::pinned(vec![cert])).unwrap();
    let password = FixedCredential::new(subject.clone(), Credential::password("alice", "pw"));
    let config = ImapConfig::from_credential_source(address, "127.0.0.1", Arc::new(password));
    let reader = ImapAccount::connect(&config, tls.connector())
        .await
        .unwrap()
        .gmail()
        .expect("the server says it is Gmail");
    (reader, tls)
}

/// The account `tokens` signs in to, its mail at `address` read over `reader`.
fn reading_over(
    tokens: &Arc<GraphTokenSource>,
    address: &str,
    reader: GmailImap,
    tls: &TlsClientConfig,
) -> ReconnectingGmailProvider {
    let mut provider = ReconnectingGmailProvider::new(
        Arc::clone(tokens),
        None,
        tls.clone(),
        &EmailAddress::new(address),
        GmailKeywordNaming::default(),
    );
    provider.imap = GmailImapSlot::opened(tokens, tls, reader);
    provider
}

/// The account `tokens` signs in to, its API requests sent to the server at `base`.
fn provider_at(
    base: String,
    tokens: &Arc<GraphTokenSource>,
    address: &str,
) -> ReconnectingGmailProvider {
    let tls = tls_with(&[]).unwrap();
    let mut provider = ReconnectingGmailProvider::new(
        Arc::clone(tokens),
        None,
        tls.clone(),
        &EmailAddress::new(address),
        GmailKeywordNaming::default(),
    );
    provider.client =
        client_connecting(Arc::clone(tokens), tls, move |source, tls, retry, agent| {
            GoogleClient::with_base_and_source(source, &base, tls, retry, agent)
        });
    provider
}

/// The client generation the delegate in `cache` was built on.
fn built_on(cache: &Mutex<Built>) -> Option<u64> {
    cache.lock().unwrap().as_ref().map(|built| built.generation)
}

/// The engine binds the IMAP half only when it signs in as the same credential subject as the
/// API half's client, so a client built over a token of its own, under a subject the engine made
/// up, fails every read of an account that reads over IMAP.
#[tokio::test]
async fn the_imap_side_of_the_account_is_bound_to_its_gmail_provider() {
    let (tokens, _) = tokens(vec![http_fake::minted(1)]);
    let (reader, tls) = reader(tokens.subject()).await;
    let provider = reading_over(&tokens, "pairing@example.com", reader, &tls);

    let (_, delegate) = provider
        .reading_delegate()
        .await
        .expect("the account's two halves sign in as the account");

    assert!(
        delegate.connection_info().sources_per_request > 1,
        "the reads go over IMAP"
    );
}

/// Another account's IMAP side is a fault in how the provider was built, and is never answered by
/// reading over the API instead.
#[tokio::test]
async fn an_imap_side_signing_in_as_another_account_fails_the_read() {
    let (tokens, _) = tokens(vec![http_fake::minted(1)]);
    let (reader, tls) = reader(&CredentialSubject::new("someone-else")).await;
    let provider = reading_over(&tokens, "mismatch@example.com", reader, &tls);

    let refused = provider.reading_delegate().await.unwrap_err();

    assert_eq!(refused.class(), FailureClass::InvalidState, "{refused}");
    assert!(
        provider.delegate().await.is_ok(),
        "writes and sends go over the API, which the IMAP side has no say in"
    );
}

/// The token goes on each request as the source answers it then, and one Google refused is
/// replaced once and the request sent again, on the same client.
#[tokio::test]
async fn each_request_presents_the_current_token_and_a_refused_one_is_replaced() {
    let (tokens, minted) = tokens(vec![http_fake::minted(1), http_fake::minted(2)]);
    let (base, received) = http_fake::serve(vec![
        Reply::RefusedBearer,
        Reply::Json(LABELS.to_owned()),
        Reply::Json(LABELS.to_owned()),
    ]);
    let provider = provider_at(base, &tokens, "bearer@example.com");

    provider.sync_mailboxes(&account(), None).await.unwrap();
    provider.sync_mailboxes(&account(), None).await.unwrap();

    assert_eq!(
        http_fake::presented(&received),
        [
            http_fake::bearer(1),
            http_fake::bearer(2),
            http_fake::bearer(2)
        ]
    );
    assert_eq!(minted.load(Ordering::SeqCst), 2);
    assert_eq!(built_on(&provider.cached), Some(0), "the client was kept");
}

#[tokio::test]
async fn a_dead_connection_replaces_the_client_and_the_read_is_made_again() {
    let (tokens, _) = tokens(vec![http_fake::minted(1)]);
    let (base, received) = http_fake::serve(vec![Reply::HangUp, Reply::Json(LABELS.to_owned())]);
    let provider = provider_at(base, &tokens, "redial@example.com");

    provider.sync_mailboxes(&account(), None).await.unwrap();

    assert_eq!(received.lock().unwrap().len(), 2);
    assert_eq!(built_on(&provider.cached), Some(1), "a new client");
}

/// A token endpoint that cannot be reached fails the request before anything is sent, which a
/// new connection cannot help, so the client is kept.
#[tokio::test]
async fn a_token_that_cannot_be_renewed_keeps_the_client() {
    let source = source_at(refused_token_endpoint(), None);
    let (base, received) = http_fake::serve(vec![]);
    let provider = provider_at(base, &source, "unrenewed@example.com");

    let refused = provider.sync_mailboxes(&account(), None).await.unwrap_err();

    assert_eq!(refused.class(), FailureClass::Retryable, "{refused}");
    assert!(received.lock().unwrap().is_empty(), "nothing was sent");
    assert_eq!(
        provider.client.current().unwrap().0,
        0,
        "the client was kept"
    );
}

/// A batch read whose IMAP side cannot sign in for want of a token answers each message with
/// that failure, still marked as a credential that could not be had.
#[tokio::test]
async fn a_batch_without_a_token_answers_each_message_with_the_credential_failure() {
    let source = source_at(refused_token_endpoint(), None);
    let (base, received) = http_fake::serve(vec![]);
    let provider = provider_at(base, &source, "batch-unrenewed@example.com");
    let inbox = MailboxId::try_from("INBOX").unwrap();
    let messages: Vec<_> = ["m1", "m2"]
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
    assert!(received.lock().unwrap().is_empty(), "nothing was sent");
}

/// A send, a saved draft and a new label may each have been applied before the connection died,
/// so each is sent once and never again by the wrapper; the dead client is still replaced for
/// the next request.
#[tokio::test]
async fn a_write_on_a_dead_connection_is_sent_once_and_the_client_replaced() {
    let draft = Draft::new(
        MessageIdHeader::new("draft@example.com").unwrap(),
        EmailAddress::new("you@example.net"),
        vec![EmailAddress::new("them@example.org")],
        "Subject",
        // Larger than a socket buffer, so the request is still being written when a server that
        // stopped reading early would drop it.
        "Body ".repeat(100_000),
    );
    let label = MailboxEdit::create("Receipts".to_owned(), None);
    for write in ["send", "draft", "label"] {
        let (tokens, _) = tokens(vec![http_fake::minted(1)]);
        let (base, received) =
            http_fake::serve(vec![Reply::HangUp, Reply::Json(LABELS.to_owned())]);
        let provider = provider_at(base, &tokens, &format!("{write}@example.com"));

        let failed = match write {
            "send" => provider
                .submit_email(&account(), &draft, &HandOver::new(&Unrecorded))
                .await
                .map(|_| ()),
            "draft" => provider
                .put_draft(&account(), &draft, None, &MessageMetadata::default())
                .await
                .map(|_| ()),
            _ => provider.edit_mailbox(&account(), &label).await.map(|_| ()),
        }
        .unwrap_err();

        assert_eq!(received.lock().unwrap().len(), 1, "{write}");
        eprintln!(
            "{write}: {:?} {}",
            failed.class(),
            failed.requires_confirmation()
        );
        assert_eq!(provider.client.generation(), 1, "{write}: a new client");
    }
}

/// A pass over IMAP reads the labels over the API first, so a dead connection on that read is the
/// API client's: the client is replaced and the pass made again, which then reads over IMAP. The
/// engine's test server shows no All Mail, so the pass ends there, past the labels.
#[tokio::test]
async fn a_dead_connection_on_an_imap_pass_replaces_the_api_client() {
    let (tokens, _) = tokens(vec![http_fake::minted(1)]);
    let (reader, imap_tls) = reader(tokens.subject()).await;
    let (base, received) = http_fake::serve(vec![Reply::HangUp, Reply::Json(LABELS.to_owned())]);
    let mut provider = reading_over(&tokens, "imap-pass@example.com", reader, &imap_tls);
    provider.client = client_connecting(
        Arc::clone(&tokens),
        tls_with(&[]).unwrap(),
        move |source, tls, retry, agent| {
            GoogleClient::with_base_and_source(source, &base, tls, retry, agent)
        },
    );

    let pass = provider
        .stream_email(&account(), None, SyncWindow::full().into(), 200, 50)
        .collect::<Vec<_>>()
        .await;

    let [Err(ended)] = pass.as_slice() else {
        panic!("{pass:?}");
    };
    assert_eq!(ended.class(), FailureClass::InvalidState, "{ended}");
    assert_eq!(
        received.lock().unwrap().len(),
        2,
        "the label read, made again"
    );
    assert_eq!(provider.client.generation(), 1, "a new API client");
}
